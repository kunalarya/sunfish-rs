#[inline]
pub fn hermite_cubic_baseline(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let d2_ = d / 2.0;
    let a2_ = -a / 2.0;
    let a_ = a2_ + ((3.0 * b) / 2.0) - ((3.0 * c) / 2.0) + d2_;
    let b_ = a - ((5.0 * b) / 2.0) + (2.0 * c) - d2_;
    let c_ = a2_ + c / 2.0;
    let d_ = b;
    let t2 = t * t;
    let t3 = t2 * t;

    a_ * t3 + b_ * t2 + c_ * t + d_
}

#[inline(always)]
fn index_wrapped(length: isize, index: isize) -> usize {
    if index > length - 1 {
        (index - (length - 1)) as usize
    } else if index < 0 {
        ((length - 1) + index) as usize
    } else {
        index as usize
    }
}

/// Interpolate the input signal according to the given
/// sample size.
///
/// # Arguments
/// `input`: The input signal.
/// `input_phase`: Normalized phase (i.e. 0.0-1.0)
/// `target_samples`: Number of samples to interpolate to.
///     This is typically 1/f0.
/// `output_buf`: Output buffer to write to.
/// `output_count`: Number of output samples to write.
///     If the buffer is not large enough, this will panic.
pub fn interpolate_hermite_inplace(
    input: &Vec<f64>,
    input_len_f: f64,
    input_phase: f64,
    target_samples: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> f64 {
    let sig_len = input.len() as isize;
    let sig_len_f = input_len_f;
    let mut phase = input_phase * target_samples;

    if phase > target_samples {
        phase = phase % target_samples;
    }

    for output_index in 0..output_count {
        let percent = phase / target_samples;
        let x = sig_len_f * percent;

        let index = x.floor();
        let t = x - index;

        let index_isize = index as isize;
        let a = input[index_wrapped(sig_len, index_isize - 1)];
        let b = input[index_wrapped(sig_len, index_isize)];
        let c = input[index_wrapped(sig_len, index_isize + 1)];
        let d = input[index_wrapped(sig_len, index_isize + 2)];
        output_buf[output_index] = hermite_cubic_baseline(a, b, c, d, t);
        phase += 1.0;
        if phase > target_samples {
            phase = phase % target_samples;
        }
    }
    phase / target_samples
}

pub fn interpolate_hermite_inplace2(
    input: &Vec<f64>,
    input_len_f: f64,
    input_phase: f64,
    input_phase2: f64,
    target_samples: f64,
    target_samples2: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> (f64, f64) {
    let sig_len = input.len() as isize;
    let sig_len_f = input_len_f;
    let mut phase = input_phase * target_samples;
    let mut phase2 = input_phase2 * target_samples2;

    if phase > target_samples {
        phase = phase % target_samples;
    }
    if phase2 > target_samples2 {
        phase2 = phase2 % target_samples2;
    }

    // This looks silly, but the goal is to exploit
    // locality of reference, because we assume the two
    // target_samples are close-enough in frequency.
    for output_index in 0..output_count {
        let percent = phase / target_samples;
        let percent2 = phase2 / target_samples2;
        let x = sig_len_f * percent;
        let x2 = sig_len_f * percent2;

        let index = x.floor();
        let index2 = x2.floor();
        let t = x - index;
        let t2 = x2 - index2;

        let index_isize = index as isize;
        let index_isize2 = index2 as isize;
        let a = input[index_wrapped(sig_len, index_isize - 1)];
        let b = input[index_wrapped(sig_len, index_isize)];
        let c = input[index_wrapped(sig_len, index_isize + 1)];
        let d = input[index_wrapped(sig_len, index_isize + 2)];
        let value = hermite_cubic_baseline(a, b, c, d, t);
        let a2 = input[index_wrapped(sig_len, index_isize2 - 1)];
        let b2 = input[index_wrapped(sig_len, index_isize2)];
        let c2 = input[index_wrapped(sig_len, index_isize2 + 1)];
        let d2 = input[index_wrapped(sig_len, index_isize2 + 2)];
        let value = value + hermite_cubic_baseline(a2, b2, c2, d2, t2);
        phase += 1.0;
        phase2 += 1.0;
        if phase > target_samples {
            phase = phase % target_samples;
        }
        if phase2 > target_samples2 {
            phase2 = phase2 % target_samples2;
        }
        output_buf[output_index] = value;
    }
    (phase / target_samples, phase2 / target_samples2)
}

pub fn interpolate_linear_inplace(
    reference: &Vec<f64>,
    ref_len_f: f64,
    input_phase: f64,
    desired_samples: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> f64 {
    let ref_len = reference.len() as isize;
    let mut phase = input_phase;

    let phase_dt = 1.0 / desired_samples;

    if phase > 1.0 {
        phase = phase % 1.0;
    }
    // Subtract one because:
    // Say ref len is 4:    x --- x --- x --- x
    // & desired len is 3:  x ------ x ------ x
    //
    // We want to treat the last point in our reference signal as an
    // endpoint, so that it's included in the interpolation.
    //
    // Put another way, this is the first-order implementation of
    // Lagrange interpolation, which interpolates points given N + 1
    // samples.
    let ref_len_n_minus_1 = ref_len_f - 1.0;
    for output_index in 0..output_count {
        // We use ref_len_f - 1 because we treat the last
        // datapoint in the reference signal as an endpoint.
        // We will interpolate between datapoints at (n-2) to (n-1)
        let ref_index = ref_len_n_minus_1 * phase;
        let ref_index_floor = ref_index.floor();

        let eta = ref_index - ref_index_floor;

        let ref_index_floor_i = ref_index_floor as isize;
        // println!(
        //     "   phase={:.3} ref_index={:.3} ref_index_floor={:.3} (i={}) eta={:.3}",
        //     phase, ref_index, ref_index_floor, ref_index_floor_i, eta,
        // );
        let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
        let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];

        output_buf[output_index] = ((1.0 - eta) * a) + (eta * b);
        // phase = (output_index as f64 * phase_dt) % 1.0;
        phase += phase_dt;
        if phase > 1.0 {
            phase = phase % 1.0;
        }
    }
    phase
}

pub fn interpolate_linear_inplace2(
    reference: &Vec<f64>,
    ref_len_f: f64,
    input_phase: f64,
    input_phase2: f64,
    desired_samples: f64,
    desired_samples2: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> f64 {
    let ref_len = reference.len() as isize;
    let mut phase = input_phase;
    let mut phase2 = input_phase2;

    let phase_dt = 1.0 / desired_samples;
    let phase2_dt = 1.0 / desired_samples2;

    if phase > 1.0 {
        phase = phase % 1.0;
    }
    if phase2 > 1.0 {
        phase2 = phase2 % 1.0;
    }
    let ref_len_n_minus_1 = ref_len_f - 1.0;
    for output_index in 0..output_count {
        let ref_index = ref_len_n_minus_1 * phase;
        let ref_index_floor = ref_index.floor();

        let ref_index2 = ref_len_n_minus_1 * phase2;
        let ref_index_floor2 = ref_index2.floor();

        let eta = ref_index - ref_index_floor;
        let eta2 = ref_index2 - ref_index_floor2;

        let ref_index_floor_i = ref_index_floor as isize;
        let ref_index_floor2_i = ref_index_floor2 as isize;
        let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
        let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];
        let value1 = ((1.0 - eta) * a) + (eta * b);

        let a2 = reference[index_wrapped(ref_len, ref_index_floor2_i)];
        let b2 = reference[index_wrapped(ref_len, ref_index_floor2_i + 1)];
        let value2 = ((1.0 - eta2) * a2) + (eta2 * b2);
        output_buf[output_index] = value1 + value2;
        phase += phase_dt;
        phase2 += phase2_dt;
        if phase > 1.0 {
            phase = phase % 1.0;
        }
        if phase2 > 1.0 {
            phase2 = phase2 % 1.0;
        }
    }
    phase
}

#[inline(always)]
fn index_clamped(length: usize, index: usize) -> usize {
    if index > length - 1 {
        length - 1
    } else {
        index
    }
}

// Reference/baseline implementation. Do not use in real-time path.
pub fn interpolate_hermite(input: &Vec<f64>, samples: usize) -> Vec<f64> {
    let sig_len = input.len();
    let mut output: Vec<f64> = Vec::with_capacity(sig_len);
    let num_samp_f = (samples - 1) as f64;
    let sig_len_f = (input.len() - 1) as f64;
    let mut i = 0.0;

    for _ in 0..samples {
        let percent = i / num_samp_f;
        let x = sig_len_f * percent;

        let index = x.floor();
        let t = x - index;

        let index_usize = index as usize;
        let prev_index = if index_usize == 0 { 0 } else { index_usize - 1 };
        let a = input[prev_index];
        let b = input[index_clamped(sig_len, index_usize + 0)];
        let c = input[index_clamped(sig_len, index_usize + 1)];
        let d = input[index_clamped(sig_len, index_usize + 2)];

        output.push(hermite_cubic_baseline(a, b, c, d, t));
        i += 1.0;
    }
    output
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::test_utils::assert_similar;

    #[test]
    fn hermite_cubic_baseline() {
        let points = vec![0.0, 1.6, 2.3, 3.5, 4.3, 5.9, 6.8];
        let num_samples = 15;
        let intp = interpolate_hermite(&points, num_samples);
        let expected = vec![
            0.0, 0.62099135, 1.4046648, 1.8510205, 2.089796, 2.448688, 2.9874635, 3.5, 3.8288631,
            4.1472306, 4.719242, 5.4705544, 6.073178, 6.513994, 6.8,
        ];
        assert_similar(&intp, &expected);
    }

    #[test]
    fn linear_interp() {
        let ref_signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let desired_samples = 4;
        let mut output_buf = vec![0.0; desired_samples];
        let _interpolated = interpolate_linear_inplace(
            &ref_signal,
            ref_signal.len() as f64,
            0.0, // input phase
            desired_samples as f64,
            &mut output_buf,
            desired_samples,
        );
    }
}
