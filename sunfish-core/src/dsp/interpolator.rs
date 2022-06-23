// Waveform Interpolation Engine.
use std::collections::HashMap;

#[allow(unused_imports)]
use log::{info, trace, warn};

use crate::dsp::interpolation;
use crate::dsp::osc::{Unison, WaveShape};
use crate::dsp::util;
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
    #[allow(dead_code)]
    last_phase3: f64,
    #[allow(dead_code)]
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

pub const TABLE_SIZE: usize = 4096;

pub struct Interpolator {
    sample_rate: f64,
    references: RefCache,
    frequencies: Vec<f64>,
}

impl Interpolator {
    pub fn new(sample_rate: f64) -> Self {
        let (mut frequencies, references) = Self::prerender_waves(sample_rate, TABLE_SIZE);
        frequencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Interpolator {
            sample_rate,
            references,
            frequencies,
        }
    }

    fn prerender_waves(sample_rate: f64, table_size: usize) -> (Vec<f64>, RefCache) {
        // prerender all shapes.
        let mut cache: RefCache = HashMap::new();

        // How many semitones to step by when creating reference
        let midi_step = 1; // TODO XXX 4; // 3 per octave

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
            .collect();

        Self::prerender_all_pure_sines(sample_rate, table_size, &mut cache, &all_freqs);
        Self::prerender_all_soft_saws(sample_rate, table_size, &mut cache, &all_freqs);
        Self::prerender_all_hard_saws(sample_rate, table_size, &mut cache, &all_freqs);

        (all_freqs, cache)
    }

    fn prerender_all_pure_sines(
        sample_rate: f64,
        table_size: usize,
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
                Self::render_waves(sample_rate, table_size, &[Frequency::new(*freq, 1.0, 1.0)]),
            );
        }
    }

    fn prerender_all_soft_saws(
        sample_rate: f64,
        table_size: usize,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
    ) {
        let shape_key = WaveShape::SoftSaw.value();
        Self::prerender_saws(
            sample_rate,
            table_size,
            cache,
            fundamental_freqs,
            shape_key,
            SOFT_SAW_HARMONICS,
        );
    }

    fn prerender_all_hard_saws(
        sample_rate: f64,
        table_size: usize,
        cache: &mut RefCache,
        fundamental_freqs: &[f64],
    ) {
        let shape_key = WaveShape::HardSaw.value();
        Self::prerender_saws(
            sample_rate,
            table_size,
            cache,
            fundamental_freqs,
            shape_key,
            HARD_SAW_HARMONICS,
        );
    }

    fn prerender_saws(
        sample_rate: f64,
        table_size: usize,
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

            cache.insert(key, Self::render_waves(sample_rate, table_size, &fparams));
        }
    }

    pub fn render_waves(sample_rate: f64, table_size: usize, fparams: &[Frequency]) -> Vec<f64> {
        // Render waves for the given frequencies, added together. Useful for constructing
        // pure tones, sawtooths, triangles, etc.
        //
        let nyquist = sample_rate / 2.0;

        // The fundamental frequency must be the first element.
        let fundamental_freq = fparams[0].f;
        let new_dt = 1.0 / (fundamental_freq * (table_size as f64));

        let mut rendered: Vec<f64> = Vec::with_capacity(table_size);
        for i in 0..table_size {
            let time = (i as f64) * new_dt;
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
        output_buf: &mut [f64],
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
                // Grab the next mipmap frequency; we bias up to ensure we're below nyquist.
                let bias_up = true;

                let ref_freq = util::closest_number_in(freq, &self.frequencies, bias_up);
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
                    .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", cache.key));
                cache.ref_waveform_len = ref_waveform.len() as f64;
                cache.last_unison = unison;
                ref_waveform
            } else {
                self.references
                    .get(&cache.key)
                    .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", cache.key))
            };

        // Render a new waveform.
        let (phase, phase2) = if unison == Unison::Off {
            let phase = interpolation::interpolate_linear_inplace(
                ref_waveform,           // input
                cache.ref_waveform_len, // input_len_f
                cache.last_phase,       // input_phase
                cache.f_samples,        // target_samples
                output_buf,             // output_buf
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
                output_buf,             // output_buf
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
