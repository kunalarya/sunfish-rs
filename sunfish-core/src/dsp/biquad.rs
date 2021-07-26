use crate::dsp::TAU;

#[derive(Debug)]
pub struct BiquadCoefs {
    c0: f64,
    c1: f64,
    c2: f64,
    c3: f64,
    c4: f64,
}

impl BiquadCoefs {
    pub fn zeros() -> Self {
        BiquadCoefs {
            c0: 0.0,
            c1: 0.0,
            c2: 0.0,
            c3: 0.0,
            c4: 0.0,
        }
    }

    pub fn lpf(sample_rate: f64, f0: f64, q: f64) -> Self {
        let w0 = TAU * (f0 / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b1 = 1.0 - cos_w0;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        BiquadCoefs::from_h_terms(b0, b1, b2, a0, a1, a2)
    }

    pub fn hpf(sample_rate: f64, f0: f64, q: f64) -> Self {
        let w0 = TAU * (f0 / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let term0 = 1.0 + cos_w0;
        let b0 = term0 / 2.0;
        let b1 = -1.0 - cos_w0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        BiquadCoefs::from_h_terms(b0, b1, b2, a0, a1, a2)
    }

    pub fn bpf(sample_rate: f64, f0: f64, q: f64) -> Self {
        let w0 = TAU * (f0 / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = q * alpha;
        let b1 = 0.0;
        let b2 = -q * alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        BiquadCoefs::from_h_terms(b0, b1, b2, a0, a1, a2)
    }

    fn from_h_terms(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        let c0 = b0 / a0;
        let c1 = b1 / a0;
        let c2 = b2 / a0;
        let c3 = -(a1 / a0);
        let c4 = -(a2 / a0);
        BiquadCoefs { c0, c1, c2, c3, c4 }
    }
}

pub fn biquad_direct_form(
    signal: &[f64],
    coefficients: &BiquadCoefs,
    prev_xn1: f64,
    prev_xn2: f64,
    prev_yn1: f64,
    prev_yn2: f64,
) -> Vec<f64> {
    let mut output = Vec::with_capacity(signal.len());
    let BiquadCoefs { c0, c1, c2, c3, c4 } = coefficients;
    #[allow(clippy::needless_range_loop)]
    for i in 0..signal.len() {
        let xn1;
        let xn2;
        let yn1;
        let yn2;

        let xn = signal[i];

        if i == 0 {
            xn1 = prev_xn1;
            xn2 = prev_xn2;
            yn1 = prev_yn1;
            yn2 = prev_yn2;
        } else if i == 1 {
            xn1 = signal[0];
            xn2 = prev_xn1;
            yn1 = output[0];
            yn2 = prev_yn1;
        } else {
            xn1 = signal[i - 1];
            xn2 = signal[i - 2];
            yn1 = output[i - 1];
            yn2 = output[i - 2];
        }

        output.push((c0 * xn) + (c1 * xn1) + (c2 * xn2) + (c3 * yn1) + (c4 * yn2));
    }
    output
}

pub fn biquad_direct_form_in_place(
    signal: &mut Vec<f64>,
    coefficients: &BiquadCoefs,
    prev_xn1: f64,
    prev_xn2: f64,
    prev_yn1: f64,
    prev_yn2: f64,
) {
    let BiquadCoefs { c0, c1, c2, c3, c4 } = coefficients;
    let mut yn1 = prev_yn1;
    let mut yn2 = prev_yn2;
    let mut xn1 = prev_xn1;
    let mut xn2 = prev_xn2;
    #[allow(clippy::needless_range_loop)]
    for i in 0..signal.len() {
        // Shift inputs.
        let xn = signal[i];
        let yn = (c0 * xn) + (c1 * xn1) + (c2 * xn2) + (c3 * yn1) + (c4 * yn2);

        // Shift output
        xn2 = xn1;
        xn1 = xn;
        yn2 = yn1;
        yn1 = yn;
        signal[i] = yn;
    }
}

pub fn biquad_direct_form_apply(
    input: f64,
    coefficients: &BiquadCoefs,
    xn1: f64,
    xn2: f64,
    yn1: f64,
    yn2: f64,
) -> f64 {
    let BiquadCoefs { c0, c1, c2, c3, c4 } = coefficients;
    let xn = input;
    (c0 * xn) + (c1 * xn1) + (c2 * xn2) + (c3 * yn1) + (c4 * yn2)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::test_utils::assert_similar_f64;

    const SAMPLING_RATE: f64 = 44100.0;

    #[test]
    fn biquad_direct_form_values() {
        let f0 = 123.45;
        let q = 1.01;
        let coeffs = BiquadCoefs::lpf(SAMPLING_RATE, f0, q);
        let test_signal = vec![1.0, 2.0, 0.5, 1.5, 2.5, -0.5, -1.0, 1.25];
        let result = biquad_direct_form(&test_signal, &coeffs, 0.0, 0.0, 0.0, 0.0);
        let expected = vec![
            0.00007667, 0.00045868, 0.00125563, 0.00238347, 0.00395112, 0.00595053, 0.00795192,
            0.00982049,
        ];
        println!("{:?}", result);
        assert_similar_f64(&result, &expected, 1e8);
    }

    #[test]
    fn biquad_direct_form_apply_values() {
        // Check that it works when applied directly.
        let f0 = 123.45;
        let q = 1.01;
        let coeffs = BiquadCoefs::lpf(SAMPLING_RATE, f0, q);
        let test_signal = vec![1.0, 2.0, 0.5, 1.5, 2.5, -0.5, -1.0, 1.25];

        let mut xn1 = 0.0;
        let mut xn2 = 0.0;
        let mut yn1 = 0.0;
        let mut yn2 = 0.0;
        let mut result = Vec::with_capacity(test_signal.len());
        for x in test_signal {
            let y = biquad_direct_form_apply(x, &coeffs, xn1, xn2, yn1, yn2);
            result.push(y);
            xn2 = xn1;
            xn1 = x;
            yn2 = yn1;
            yn1 = y;
        }
        let expected = vec![
            0.00007667, 0.00045868, 0.00125563, 0.00238347, 0.00395112, 0.00595053, 0.00795192,
            0.00982049,
        ];
        assert_similar_f64(&result, &expected, 1e8);
    }

    #[test]
    fn biquad_direct_form_in_place_values() {
        let f0 = 123.45;
        let q = 1.01;
        let coeffs = BiquadCoefs::lpf(SAMPLING_RATE, f0, q);
        let mut test_signal = vec![1.0, 2.0, 0.5, 1.5, 2.5, -0.5, -1.0, 1.25];
        biquad_direct_form_in_place(&mut test_signal, &coeffs, 0.0, 0.0, 0.0, 0.0);
        let expected = vec![
            0.00007667, 0.00045868, 0.00125563, 0.00238347, 0.00395112, 0.00595053, 0.00795192,
            0.00982049,
        ];
        assert_similar_f64(&test_signal, &expected, 1e8);
    }
}
