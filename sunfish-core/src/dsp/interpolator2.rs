use std::collections::HashMap;

use crate::dsp::osc::{Unison, WaveShape};
use crate::dsp::util;
use crate::dsp::HashableF64;

pub const MAX_UNISON: usize = 32;

type WaveformKey = HashableF64;

struct Waveforms {
    frequencies: Vec<f64>,
    waves: HashMap<WaveformKey, Vec<f64>>,
}

pub trait HasWaveforms<K> {
    // TODO: rename
    fn get(&self, key: &K) -> &'_ [f64];
    fn frequencies(&self) -> &'_ [f64];
}

impl HasWaveforms<WaveformKey> for Waveforms {
    fn get(&self, key: &WaveformKey) -> &'_ [f64] {
        self.waves
            .get(key)
            .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", key))
    }
    fn frequencies(&self) -> &'_ [f64] {
        &self.frequencies
    }
}

// Cache the generated/interpolated waveform.
#[derive(Clone, Debug)]
pub struct State {
    freq: f64,
    phase: [f64; MAX_UNISON],
    key: WaveformKey,
    f_samples: [f64; MAX_UNISON],
    ref_waveform_len: f64,
    unison: Unison,
    unison_amt: f64,
}

impl State {
    pub fn zero() -> Self {
        Self {
            freq: 0.0,
            phase: Default::default(),
            key: Default::default(),
            f_samples: Default::default(),
            ref_waveform_len: 0.0,
            unison: Unison::Off,
            unison_amt: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.freq = 0.0;
        self.key = Default::default();
        self.f_samples = Default::default();
        self.ref_waveform_len = 0.0;
        self.unison = Unison::Off;
        self.unison_amt = 0.0;
    }
}

pub struct Populate<'a> {
    sample_rate: f64,
    voice_count: usize,
    freq: f64,
    output_buf: &'a mut [f64],
    output_count: usize,
    unison: Unison,
    unison_amt: f64,
}

impl<'a> Populate<'a> {
    fn frequency_changed(&self, cache: &State) -> bool {
        self.freq != cache.freq
            || self.unison != cache.unison
            || self.unison_amt != cache.unison_amt
    }
}

pub struct Interpolator;

impl Interpolator {
    pub fn populate<W>(&mut self, args: &mut Populate, cache: &mut State, waveforms: &W)
    where
        W: HasWaveforms<HashableF64>,
    {
        if args.freq == 0.0 {
            log::error!("Zero frequency");
            return;
        }

        let mut frequency_changed = false;
        if args.frequency_changed(&cache) {
            frequency_changed = true;
            let bias_up = true;
            let ref_freq = util::closest_number_in(args.freq, &waveforms.frequencies(), bias_up);
            let key = HashableF64::from_float(ref_freq);
            cache.key = key;
            cache.freq = args.freq;
            for voice in 0..args.voice_count {
                cache.f_samples[voice] =
                    args.sample_rate / (args.freq + (voice as f64) * args.unison_amt);
            }
            cache.unison = args.unison;
        }

        // let ref_waveform = waveforms
        //     .waves
        //     .get(&cache.key)
        //     .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", cache.key));
        let ref_waveform = waveforms.get(&cache.key);
        // TODO: remove -- int -> float is ~5 cycles on Intel,
        if frequency_changed {
            cache.ref_waveform_len = ref_waveform.len() as f64;
        }

        cache.phase = interpolate_linear_inplace(
            &ref_waveform,          // reference
            cache.ref_waveform_len, // ref_len_f
            cache.phase,            // input_phases
            cache.f_samples,        // desired_samples
            args.voice_count,       // voice_count
            args.output_buf,        // output_buf
            args.output_count,      // output_count
        );
    }
}

/// Unison, n-voice linear interpolation.
#[allow(clippy::too_many_arguments)]
pub fn interpolate_linear_inplace(
    reference: &[f64],
    ref_len_f: f64,
    input_phases: [f64; MAX_UNISON],
    desired_samples: [f64; MAX_UNISON],
    voice_count: usize,
    output_buf: &mut [f64],
    output_count: usize,
) -> [f64; MAX_UNISON] {
    let ref_len = reference.len() as isize;

    #[allow(clippy::uninit_assumed_init)]
    let mut phases: [f64; MAX_UNISON] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

    for (index, input_phase) in input_phases.iter().enumerate() {
        phases[index] = input_phase % 1.0;
    }

    #[allow(clippy::uninit_assumed_init)]
    let mut phase_dts: [f64; MAX_UNISON] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

    for (index, desired_sample) in desired_samples.iter().enumerate() {
        phase_dts[index] = 1.0 / desired_sample;
    }

    #[allow(clippy::needless_range_loop)]
    for output_index in 0..output_count {
        // We will interpolate between datapoints at (n-2) to (n-1)
        output_buf[output_index] = 0.0;
        for i in 0..voice_count {
            let ref_index = ref_len_f * phases[i];
            let ref_index_floor = ref_index.floor();

            let eta = ref_index - ref_index_floor;

            let ref_index_floor_i = ref_index_floor as isize;

            let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
            let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];
            let voice = ((1.0 - eta) * a) + (eta * b);
            output_buf[output_index] += voice;

            phases[i] = (phases[i] + phase_dts[i]) % 1.0;
        }
    }
    phases
}

/// Wrap the index.
#[inline(always)]
fn index_wrapped(length: isize, index: isize) -> usize {
    index as usize % length as usize
}

mod tests {

    use super::*;
    use crate::dsp::interpolator as v1_interp;

    #[test]
    fn compare_v1_v2() {
        let sample_rate = 44100.0;
        let freq = 2500.0;
        let key = freq.into();
        let waveforms = Waveforms {
            frequencies: vec![2500.0],
            waves: {
                let mut waves: HashMap<WaveformKey, Vec<f64>> = HashMap::new();
                waves.insert(key, vec![1.0, 2.0, 3.0, 4.0]);
                waves
            },
        };
        let mut interpolator = Interpolator;
        let mut output_buf = [0.0f64; 2048];
        let output_count = output_buf.len();
        let mut args = Populate {
            sample_rate,
            voice_count: 1,
            freq,
            output_buf: &mut output_buf,
            output_count,
            unison: Unison::Off,
            unison_amt: 0.0,
        };
        let phase: [f64; MAX_UNISON] = Default::default();
        let f_samples: [f64; MAX_UNISON] = Default::default();
        let mut state = State {
            freq,
            phase,
            key,
            f_samples,
            ref_waveform_len: 4.0,
            unison: Unison::Off,
            unison_amt: 0.0,
        };
        interpolator.populate(&mut args, &mut state, &waveforms);

        let mut v1i = v1_interp::Interpolator::new(sample_rate);
    }
}
