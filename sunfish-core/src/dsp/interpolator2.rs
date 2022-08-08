use std::collections::HashMap;

use crate::dsp::osc::{Unison, WaveShape};
use crate::dsp::util;
use crate::dsp::HashableF64;

pub const MAX_UNISON: usize = 32;

pub type WaveformKey = HashableF64;

pub struct Waveforms {
    pub frequencies: Vec<f64>,
    pub waves: HashMap<WaveformKey, Vec<f64>>,
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
    pub freq: f64,
    pub phase: [f64; MAX_UNISON],
    pub key: WaveformKey,
    pub f_samples: [f64; MAX_UNISON],
    pub ref_waveform_len: f64,
    pub unison: Unison,
    pub unison_amt: f64,
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
    pub sample_rate: f64,
    pub voice_count: usize,
    pub freq: f64,
    pub output_buf: &'a mut [f64],
    pub output_count: usize,
    pub unison: Unison,
    pub unison_amt: f64,
}

impl<'a> Populate<'a> {
    fn frequency_changed(&self, cache: &State) -> bool {
        self.freq != cache.freq
            || self.unison != cache.unison
            || self.unison_amt != cache.unison_amt
    }
}

pub struct Interpolator;
use std::time::Instant;

impl Interpolator {
    pub fn populate<W>(&mut self, args: &mut Populate, cache: &mut State, waveforms: &W)
    where
        W: HasWaveforms<HashableF64>,
    {
        if args.freq == 0.0 {
            log::error!("Zero frequency");
            return;
        }

        let t0 = Instant::now();
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
        let t1 = Instant::now();
        //println!("t1-t0: {:?}", t1 - t0);

        // let ref_waveform = waveforms
        //     .waves
        //     .get(&cache.key)
        //     .unwrap_or_else(|| panic!("Internal error (bad key: {:?})", cache.key));
        let t2 = Instant::now();
        let ref_waveform = waveforms.get(&cache.key);
        // TODO: remove -- int -> float is ~5 cycles on Intel,
        if frequency_changed {
            cache.ref_waveform_len = ref_waveform.len() as f64;
        }
        let t3 = Instant::now();
        //println!("t3-t2: {:?}", t3 - t2);

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

    // let t0 = Instant::now();
    #[allow(clippy::uninit_assumed_init)]
    let mut phases: [f64; MAX_UNISON] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
    //let t1 = Instant::now();
    //println!("t1-t0: {:?}", t1 - t0);

    //let t2 = Instant::now();
    for index in 0..voice_count {
        phases[index] = input_phases[index] % 1.0;
    }
    //let t3 = Instant::now();
    //println!("t3-t2: {:?}", t3 - t2);

    //let t4 = Instant::now();
    #[allow(clippy::uninit_assumed_init)]
    let mut phase_dts: [f64; MAX_UNISON] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
    //let t5 = Instant::now();
    //println!("t5-t4: {:?}", t5 - t4);

    //let t6 = Instant::now();
    for (index, desired_sample) in desired_samples.iter().enumerate() {
        phase_dts[index] = 1.0 / desired_sample;
    }
    //let t7 = Instant::now();
    //println!("t7-t6: {:?}", t7 - t6);

    #[allow(clippy::needless_range_loop)]
    for output_index in 0..output_count {
        // We will interpolate between datapoints at (n-2) to (n-1)
        output_buf[output_index] = 0.0;
        for i in 0..voice_count {
            //let t8 = Instant::now();
            let ref_index = ref_len_f * phases[i];
            let ref_index_floor = ref_index.floor();
            //let t9 = Instant::now();
            //println!("t9-t8: {:?}", t9 - t8);

            //let t10 = Instant::now();
            let eta = ref_index - ref_index_floor;

            let ref_index_floor_i = ref_index_floor as isize;
            //let t11 = Instant::now();
            //println!("t11-t10: {:?}", t11 - t10);

            //let t12 = Instant::now();
            let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
            let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];
            //let t13 = Instant::now();
            //println!("t13-t12: {:?}", t13 - t12);

            //let t14 = Instant::now();
            let voice = ((1.0 - eta) * a) + (eta * b);
            output_buf[output_index] += voice;

            phases[i] = (phases[i] + phase_dts[i]) % 1.0;
            //let t15 = Instant::now();
            //println!("t15-t14: {:?}", t15 - t14);
        }
    }
    phases
}

/// Wrap the index.
#[inline(always)]
fn index_wrapped(length: isize, index: isize) -> usize {
    //index as usize % length as usize
    (index % length) as usize
}

mod tests {

    use super::*;
    use crate::dsp::interpolator as v1_interp;
    use std::time::{Duration, Instant};

    #[test]
    fn compare_v1_v2() {
        let perf_iters = 100;
        fn summarize(times: &[Duration]) {
            let avg_ns =
                times.iter().map(|x| x.as_nanos() as f64).sum::<f64>() / (times.len() as f64);
            println!("times: {times:?}");
            println!("avg_ns: {avg_ns:.6}");
        }

        let sample_rate = 44100.0;
        let mut interpolator_v1 = v1_interp::Interpolator::new(sample_rate);
        let freq = 3520.0; // A7
        let shape = WaveShape::HardSaw;
        let freq_key = freq.into();

        let v1_key = (shape.value(), freq_key);
        let waveform_data = interpolator_v1
            .temporary_get_ref_cache()
            .get(&v1_key)
            .unwrap()
            .clone();
        let ref_waveform_len = waveform_data.len() as f64;
        const BUF_SIZE: usize = 16; //2048;
        let mut output_buf_v1 = [0.0f64; BUF_SIZE];
        let output_count = output_buf_v1.len();

        let mut cached_waveform = v1_interp::CachedWaveform {
            last_freq: 0.0,
            last_phase: 0.0,
            last_phase2: 0.0,
            last_phase3: 0.0,
            last_phase4: 0.0,
            key: v1_key,
            f_samples: sample_rate / freq,
            f_samples2: 0.0,
            ref_waveform_len,
            last_unison: Unison::Off,
            last_unison_amt: 0.0,
        };

        let mut times = vec![];
        for _ in 0..perf_iters {
            let t0 = Instant::now();
            interpolator_v1.populate(
                shape,
                freq,
                &mut output_buf_v1,
                output_count,
                &mut cached_waveform,
                Unison::Off,
                0.0,
            );
            let t1 = Instant::now();
            times.push(t1 - t0);
        }
        summarize(&times);

        let waveforms = Waveforms {
            frequencies: vec![2500.0],
            waves: {
                let mut waves: HashMap<WaveformKey, Vec<f64>> = HashMap::new();
                waves.insert(freq_key, waveform_data);
                waves
            },
        };
        let mut interpolator = Interpolator;
        let mut output_buf = [0.0f64; BUF_SIZE];
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
        let f_samples: [f64; MAX_UNISON] = {
            let mut f_samples: [f64; MAX_UNISON] = Default::default();
            f_samples[0] = sample_rate / freq;
            f_samples
        };
        let mut state = State {
            freq,
            phase,
            key: freq_key,
            f_samples,
            ref_waveform_len,
            unison: Unison::Off,
            unison_amt: 0.0,
        };
        let mut times = vec![];
        for _ in 0..perf_iters {
            let t0 = Instant::now();
            interpolator.populate(&mut args, &mut state, &waveforms);
            let t1 = Instant::now();
            times.push(t1 - t0);
        }
        summarize(&times);
        let mut errs = 0;
        for (idx, value) in args.output_buf.iter().enumerate() {
            let v1_value = output_buf_v1[idx];
            if *value != v1_value {
                errs += 1;
            }
        }
        println!("errors: {errs}");
    }
}
