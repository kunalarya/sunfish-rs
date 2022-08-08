use std::collections::HashMap;

use sunfish::dsp::interpolator as v1_interp;
use sunfish::dsp::interpolator2::{self, Populate, WaveformKey, Waveforms, MAX_UNISON};
use sunfish::dsp::osc::{Unison, WaveShape};

use std::time::{Duration, Instant};

fn compare_v1_v2() {
    let perf_iters = 100;
    fn summarize(times: &[Duration]) {
        let avg_ns = times.iter().map(|x| x.as_nanos() as f64).sum::<f64>() / (times.len() as f64);
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
    let mut interpolator = interpolator2::Interpolator;
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
    let mut state = interpolator2::State {
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
fn main() {
    compare_v1_v2();
}
