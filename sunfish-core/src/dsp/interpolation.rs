#[allow(clippy::many_single_char_names)]
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

/// Wrap the index.
#[inline(always)]
fn index_wrapped(length: isize, index: isize) -> usize {
    index as usize % length as usize
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
#[allow(clippy::many_single_char_names)]
pub fn interpolate_hermite_inplace(
    input: &[f64],
    input_len_f: f64,
    input_phase: f64,
    target_samples: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> f64 {
    let sig_len = input.len() as isize;
    let sig_len_f = input_len_f;
    let mut phase = (input_phase * target_samples) % target_samples;

    #[allow(clippy::needless_range_loop)]
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
        phase = (phase + 1.0) % target_samples;
    }
    phase / target_samples
}

pub fn interpolate_linear_inplace(
    reference: &[f64],
    ref_len_f: f64,
    input_phase: f64,
    desired_samples: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> f64 {
    let ref_len = reference.len() as isize;
    let mut phase = input_phase % 1.0;

    let phase_dt = 1.0 / desired_samples;
    // This is the first-order implementation of Lagrange interpolation.
    #[allow(clippy::needless_range_loop)]
    for output_index in 0..output_count {
        // We will interpolate between datapoints at (n-2) to (n-1)
        let ref_index = ref_len_f * phase;
        let ref_index_floor = ref_index.floor();

        let eta = ref_index - ref_index_floor;

        let ref_index_floor_i = ref_index_floor as isize;
        let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
        let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];

        output_buf[output_index] = ((1.0 - eta) * a) + (eta * b);
        phase = (phase + phase_dt) % 1.0;
    }
    phase
}

/// Unison, 2-voice linear interpolation.
/// TODO: Merge with above.
#[allow(clippy::too_many_arguments)]
pub fn interpolate_linear_inplace2(
    reference: &[f64],
    ref_len_f: f64,
    input_phase: f64,
    input_phase2: f64,
    desired_samples: f64,
    desired_samples2: f64,
    output_buf: &mut [f64],
    output_count: usize,
) -> (f64, f64) {
    let ref_len = reference.len() as isize;
    let mut phase = input_phase % 1.0;
    let mut phase2 = input_phase2 % 1.0;

    let phase_dt = 1.0 / desired_samples;
    let phase_dt2 = 1.0 / desired_samples2;
    #[allow(clippy::needless_range_loop)]
    for output_index in 0..output_count {
        // We will interpolate between datapoints at (n-2) to (n-1)
        let ref_index = ref_len_f * phase;
        let ref_index2 = ref_len_f * phase2;
        let ref_index_floor = ref_index.floor();
        let ref_index_floor2 = ref_index2.floor();

        let eta = ref_index - ref_index_floor;
        let eta2 = ref_index2 - ref_index_floor2;

        let ref_index_floor_i = ref_index_floor as isize;
        let ref_index_floor_i2 = ref_index_floor2 as isize;

        let a = reference[index_wrapped(ref_len, ref_index_floor_i)];
        let b = reference[index_wrapped(ref_len, ref_index_floor_i + 1)];
        let voice1 = ((1.0 - eta) * a) + (eta * b);
        let a2 = reference[index_wrapped(ref_len, ref_index_floor_i2)];
        let b2 = reference[index_wrapped(ref_len, ref_index_floor_i2 + 1)];
        let voice2 = ((1.0 - eta2) * a2) + (eta2 * b2);
        output_buf[output_index] = voice1 + voice2;

        phase = (phase + phase_dt) % 1.0;
        phase2 = (phase2 + phase_dt2) % 1.0;
    }
    (phase, phase2)
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
#[allow(clippy::many_single_char_names)]
pub fn interpolate_hermite(input: &[f64], samples: usize) -> Vec<f64> {
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
        let b = input[index_clamped(sig_len, index_usize)];
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
    use crate::util::test_utils::assert_similar_f64;

    #[test]
    fn hermite_cubic_baseline() {
        let points = vec![0.0, 1.6, 2.3, 3.5, 4.3, 5.9, 6.8];
        let num_samples = 15;
        let intp = interpolate_hermite(&points, num_samples);
        let expected = vec![
            0.0, 0.62099125, 1.40466472, 1.85102041, 2.08979592, 2.44868805, 2.98746356, 3.5,
            3.82886297, 4.14723032, 4.71924198, 5.47055394, 6.07317784, 6.51399417, 6.8,
        ];
        assert_similar_f64(&intp, &expected, 1e8);
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
