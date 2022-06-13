use std::collections::HashMap;

use crate::dsp::osc::{Unison, WaveShape};
use crate::dsp::{HashableF64};
use crate::dsp::util;

type ShapeKey = u8;
pub const MAX_UNISON: usize = 32;

type WaveformKey = HashableF64;

struct Waveforms {
    frequencies: Vec<f64>,
    waves: HashMap<WaveformKey, Vec<f64>>
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
            key: HashableF64::from_float(0.0),
            f_samples: Default::default(),
            ref_waveform_len: 0.0,
            unison: Unison::Off,
            unison_amt: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.freq = 0.0;
        self.key = HashableF64::from_float(0.0);
        self.f_samples = Default::default();
        self.ref_waveform_len = 0.0;
        self.unison = Unison::Off;
        self.unison_amt = 0.0;
    }
}

pub struct Populate<'a> {
    sample_rate: f64,
    voices: usize,
    shape: &'a WaveShape,
    freq: f64,
    output_buf: &'a mut [f64],
    output_count: usize,
    cache: &'a mut State,
    unison: Unison,
    unison_amt: f64,
}

impl<'a> Populate<'a> {
    fn frequency_changed(&self) -> bool {
        self.freq != self.cache.freq
            || self.unison != self.cache.unison
            || self.unison_amt != self.cache.unison_amt
    }
}

struct Interpolator;

impl Interpolator {
    fn populate(&mut self, args: &Populate, waveforms: &Waveforms) {
        if args.freq == 0.0 {
            log::error!("Zero frequency");
            return;
        }
        if args.frequency_changed() {
            let bias_up = true;
                let ref_freq = util::closest_number_in(args.freq, &waveforms.frequencies, bias_up);
                let key =  HashableF64::from_float(ref_freq);
                args.cache.key = key;
                args.cache.freq = args.freq;
                for voice in 0..args.voices {
                    args.cache.f_samples[voice] = args.sample_rate / (args.freq + (voice as f64) * args.unison_amt);
                }
                args.cache.unison = args.unison;
        }

        let ref_waveform = waveforms
            .waves
            .get(&args.cache.key)
            .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", args.cache.key));
        args.cache.ref_waveform_len = ref_waveform.len() as f64;
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
