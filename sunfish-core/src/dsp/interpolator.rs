// Waveform Interpolation Engine.
use std::collections::HashMap;

#[allow(unused_imports)]
use log::{info, trace, warn};

use crate::dsp::interpolation;
use crate::dsp::osc::{Unison, WaveShape};
use crate::dsp::{normalize, HashableF64, TAU};
use crate::util::note_freq;

type ShapeKey = u8;
type RefCache = HashMap<(ShapeKey, HashableF64), Vec<f64>>;

const SOFT_SAW_HARMONICS: usize = 8;
const HARD_SAW_HARMONICS: usize = 64;

// Cache the generated/interpolated waveform.
#[derive(Clone, Debug)]
pub struct CachedWaveform {
    last_freq: f64,
    last_phase: f64,
    last_phase2: f64,
    last_phase3: f64,
    last_phase4: f64,
    key: (ShapeKey, HashableF64),
    f_samples: f64,
    f_samples2: f64,
    ref_waveform_len: f64,
    last_unison: Unison,
    last_unison_amt: f64,
}

impl CachedWaveform {
    pub fn zero() -> Self {
        CachedWaveform {
            last_freq: 0.0,
            last_phase: 0.0,
            last_phase2: 0.0,
            last_phase3: 0.0,
            last_phase4: 0.0,
            key: (0, HashableF64::from_float(0.0)),
            f_samples: 0.0,
            f_samples2: 0.0,
            ref_waveform_len: 0.0,
            last_unison: Unison::Off,
            last_unison_amt: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.last_freq = 0.0;
        self.key = (0, HashableF64::from_float(0.0));
        self.f_samples = 0.0;
        self.f_samples2 = 0.0;
        self.ref_waveform_len = 0.0;
        self.last_unison = Unison::Off;
        self.last_unison_amt = 0.0;
    }
}

pub struct Frequency {
    // Parameters associated with a single frequency.
    f: f64,
    amp: f64,
    divisor: f64,
}

impl Frequency {
    pub fn new(f: f64, amp: f64, divisor: f64) -> Frequency {
        Frequency { f, amp, divisor }
    }
}

pub struct Interpolator {
    sample_rate: f64,
    references: RefCache,
    frequencies: Vec<f64>,
}

impl Interpolator {
    pub fn new(sample_rate: f64) -> Self {
        let dt = 1.0 / sample_rate;
        let (mut frequencies, references) = Self::prerender_waves(sample_rate, dt);
        frequencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Interpolator {
            sample_rate,
            references,
            frequencies,
        }
    }

    fn factors(number: f64) -> Vec<u64> {
        let target = number.ceil() as u64;
        (2..target + 1)
            .into_iter()
            .filter(|&x| target % x == 0)
            .collect()
    }

    fn prerender_waves(sample_rate: f64, dt: f64) -> (Vec<f64>, RefCache) {
        // prerender all shapes.
        let mut cache: RefCache = HashMap::new();

        // How many semitones to step by when creating reference
        let midi_step = 4; // 3 per octave
        let sample_rate_factors: Vec<f64> = Self::factors(sample_rate / 2.0)
            .into_iter()
            .map(|x| x as f64)
            .collect();

        let round_to_sample_rate = |f: f64| {
            // Bias to lower frequency.
            let bias_up = false;
            closest_number_in(f, &sample_rate_factors, bias_up)
        };

        // for each shape, render all fundamental frequencies for the mipmap.
        // Max frequency to render:
        let max_note = note_freq::MIDI_NOTE_MAX;
        let all_freqs: Vec<f64> = (note_freq::MIDI_NOTE_MIN..max_note)
            .step_by(midi_step)
            .map(|note| {
                *note_freq::NOTE_TO_FREQ
                    .get(&note)
                    .expect("NOTE_TO_FREQ missing note")
            })
            .map(round_to_sample_rate)
            .collect();

        // TODO: Change fundamental frequency to nearest perfect non-sampling
        // error.

        Self::prerender_all_pure_sines(sample_rate, dt, &mut cache, &all_freqs);
        Self::prerender_all_soft_saws(sample_rate, dt, &mut cache, &all_freqs);
        Self::prerender_all_hard_saws(sample_rate, dt, &mut cache, &all_freqs);

        (all_freqs, cache)
    }

    fn prerender_all_pure_sines(
        sample_rate: f64,
        dt: f64,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
    ) {
        /*
         * Pre-render pure sine waves containing only the fundamental frequencies.
         */
        let shape_key = WaveShape::Sine.value();

        for freq in fundamental_freqs.iter() {
            let key = (shape_key, HashableF64::from_float(*freq));
            cache.insert(
                key,
                Self::render_waves(sample_rate, dt, &[Frequency::new(*freq, 1.0, 1.0)]),
            );
        }
    }

    fn prerender_all_soft_saws(
        sample_rate: f64,
        dt: f64,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
    ) {
        let shape_key = WaveShape::SoftSaw.value();
        Self::prerender_saws(
            sample_rate,
            dt,
            cache,
            fundamental_freqs,
            shape_key,
            SOFT_SAW_HARMONICS,
        );
    }

    fn prerender_all_hard_saws(
        sample_rate: f64,
        dt: f64,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
    ) {
        let shape_key = WaveShape::HardSaw.value();
        Self::prerender_saws(
            sample_rate,
            dt,
            cache,
            fundamental_freqs,
            shape_key,
            HARD_SAW_HARMONICS,
        );
    }

    fn prerender_saws(
        sample_rate: f64,
        dt: f64,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
        shape_key: u8,
        harmonics: usize,
    ) {
        /*
         * Pre-render sawtooths with a handful of harmonics.
         */

        fn get_amp(harmonic: usize) -> f64 {
            if harmonic & 0x1 == 1 {
                1.0
            } else {
                -1.0
            }
        }

        for freq in fundamental_freqs.iter() {
            let key = (shape_key, HashableF64::from_float(*freq));

            // TODO: Cut off harmonics close to Nyquist.
            let fparams: Vec<Frequency> = (1..=harmonics)
                // Collect tuples of amplitude and frequency.
                .map(|mult| Frequency::new(mult as f64 * freq, get_amp(mult), mult as f64))
                .collect();

            cache.insert(key, Self::render_waves(sample_rate, dt, &fparams));
        }
    }

    pub fn render_waves(sample_rate: f64, dt: f64, fparams: &[Frequency]) -> Vec<f64> {
        // Render waves for the given frequencies, added together. Useful for constructing
        // pure tones, sawtooths, triangles, etc.
        //
        // The fundamental frequency must be the first element.
        let nyquist = sample_rate / 2.0;

        let fundamental_freq = fparams[0].f;
        let samples_float = sample_rate / fundamental_freq;
        let samples_float_rounded = samples_float.round();
        let samples = samples_float_rounded as usize;

        #[allow(clippy::float_cmp)]
        if samples_float != samples_float_rounded {
            println!("Warning: bad reference fundamental frequency; not a multiple of sample rate");
        }

        let mut rendered: Vec<f64> = Vec::with_capacity(samples);
        for i in 0..samples {
            let time = (i as f64) * dt;
            let value = {
                let mut v = 0.0;
                for fparam in fparams.iter() {
                    // Stack up the harmonics
                    if fparam.f < nyquist {
                        v += fparam.amp * ((TAU * fparam.f * time).sin() / fparam.divisor);
                    }
                }
                v
            };

            rendered.push(value);
        }
        normalize(rendered)
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn populate(
        &mut self,
        shape: WaveShape,
        freq: f64,
        mut output_buf: &mut [f64],
        output_count: usize,
        cache: &mut CachedWaveform,
        unison: Unison,
        unison_amt: f64,
    ) {
        if freq == 0.0 {
            log::error!("Zero frequency");
            return;
        }
        let last_freq = cache.last_freq;
        let last_unison = cache.last_unison;
        let last_unison_amt = cache.last_unison_amt;

        #[allow(clippy::float_cmp)]
        let ref_waveform =
            if last_freq != freq || unison != last_unison || unison_amt != last_unison_amt {
                // Grab the next mipmap frequency.
                let bias_up = true;
                let ref_freq = closest_number_in(freq, &self.frequencies, bias_up);
                let key = (shape.value(), HashableF64::from_float(ref_freq));
                cache.key = key;
                cache.last_freq = freq;
                cache.f_samples = self.sample_rate / freq;
                cache.f_samples2 = if unison != Unison::Off {
                    self.sample_rate / (freq + unison_amt)
                } else {
                    0.0
                };
                let ref_waveform = self
                    .references
                    .get(&cache.key)
                    .unwrap_or_else(|| panic!("Internal error"));
                cache.ref_waveform_len = ref_waveform.len() as f64;
                cache.last_unison = unison;
                ref_waveform
            } else {
                self.references
                    .get(&cache.key)
                    .unwrap_or_else(|| panic!("Internal error"))
            };

        // Render a new waveform.
        let (phase, phase2) = if unison == Unison::Off {
            let phase = interpolation::interpolate_linear_inplace(
                ref_waveform,           // input
                cache.ref_waveform_len, // input_len_f
                cache.last_phase,       // input_phase
                cache.f_samples,        // target_samples
                &mut output_buf,        // output_buf
                output_count,           // output_count
            );
            (phase, 0.0)
        } else if unison == Unison::U2 {
            let (phase, phase2) = interpolation::interpolate_linear_inplace2(
                ref_waveform,           // input
                cache.ref_waveform_len, // input_len_f
                cache.last_phase,       // input_phase
                cache.last_phase2,      // input_phase2
                cache.f_samples,        // target_samples
                cache.f_samples2,       // target_samples2
                &mut output_buf,        // output_buf
                output_count,           // output_count
            );
            (phase, phase2)
        } else {
            (0.0, 0.0)
        };
        cache.last_phase = phase;
        cache.last_phase2 = phase2;
    }
}

/// Find the closest frequency, biased either up or down.
fn closest_number_in(search: f64, freqs: &[f64], bias_up: bool) -> f64 {
    // Variation on binary search where we account for items in the range between points. To
    // accommodate this, we vary from traditional binary search by moving the first and last
    // markers to *inclusive* points.
    let n = freqs.len();
    if n == 0 {
        return 0.0;
    }

    let mut first = 0;
    let mut last = n - 1;
    let mut middle = n / 2;
    if search < freqs[first] {
        return freqs[first];
    }
    if search > freqs[last] {
        return freqs[last];
    }

    while last - first > 1 {
        let mid_value = freqs[middle];
        #[allow(clippy::float_cmp)]
        if search == mid_value {
            return mid_value;
        } else if search > mid_value {
            first = middle;
        } else {
            last = middle;
        }

        middle = (first + last) / 2;
    }

    let (i, j) = if bias_up {
        (last, first)
    } else {
        (first, last)
    };
    #[allow(clippy::float_cmp)]
    if freqs[i] == search {
        freqs[i]
    } else {
        freqs[j]
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lookup_freq() {
        let fs = [0.0, 5.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(4.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(5.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(16.0, &fs, true), 15.0);

        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(4.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(5.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(16.0, &fs, false), 15.0);

        let fs = [0.0, 5.0, 10.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(4.0, &fs, true), 0.0);
        assert_eq!(closest_number_in(5.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(6.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(10.0, &fs, true), 10.0);
        assert_eq!(closest_number_in(12.0, &fs, true), 10.0);

        assert_eq!(closest_number_in(0.0, &fs, false), 0.0);
        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(4.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(5.0, &fs, false), 5.0);
        assert_eq!(closest_number_in(6.0, &fs, false), 10.0);
        assert_eq!(closest_number_in(10.0, &fs, false), 10.0);
        assert_eq!(closest_number_in(12.0, &fs, false), 10.0);

        let fs = [5.0, 10.0];
        assert_eq!(closest_number_in(1.0, &fs, true), 5.0);
        assert_eq!(closest_number_in(1.0, &fs, false), 5.0);
    }

    #[test]
    fn factors() {
        fn compute(num: f64) -> Vec<u64> {
            let mut results = Interpolator::factors(num);
            results.sort_by(|a, b| a.partial_cmp(b).unwrap());
            results
        }
        assert_eq!(compute(145.0), vec![5, 29, 145]);
        assert_eq!(
            compute(44100.0),
            vec![
                2, 3, 4, 5, 6, 7, 9, 10, 12, 14, 15, 18, 20, 21, 25, 28, 30, 35, 36, 42, 45, 49,
                50, 60, 63, 70, 75, 84, 90, 98, 100, 105, 126, 140, 147, 150, 175, 180, 196, 210,
                225, 245, 252, 294, 300, 315, 350, 420, 441, 450, 490, 525, 588, 630, 700, 735,
                882, 900, 980, 1050, 1225, 1260, 1470, 1575, 1764, 2100, 2205, 2450, 2940, 3150,
                3675, 4410, 4900, 6300, 7350, 8820, 11025, 14700, 22050, 44100
            ]
        );
    }
}
