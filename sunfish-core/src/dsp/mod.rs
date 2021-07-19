pub mod biquad;
pub mod env;
pub mod filter;
pub mod interpolation;
pub mod resonant_filter;
pub mod smoothing;

pub const TAU: f64 = (std::f64::consts::PI as f64) * 2.0;

type F64AsU = u64;

// Useful for putting floats into hashmaps.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Hashablef64(F64AsU);

impl Hashablef64 {
    fn from_float(f: f64) -> Self {
        Hashablef64(unsafe { std::mem::transmute::<f64, F64AsU>(f) })
    }
    #[allow(dead_code)]
    fn to_float(&self) -> f64 {
        unsafe { std::mem::transmute::<F64AsU, f64>(self.0) }
    }
}

fn normalize(mut signal: Vec<f64>) -> Vec<f64> {
    let max = signal
        .iter()
        .map(|f| {
            let res: f64 = f.abs();
            res
        })
        .fold(0. / 0., f64::max);
    signal.drain(..).map(|f| f / max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_utils;

    #[test]
    fn normalize_negative() {
        let v: Vec<f64> = vec![-2.0, 0.0, 1.0];
        let normalized = normalize(v);
        test_utils::assert_similar(&normalized, &vec![-1.0, 0.0, 0.5]);
    }

    #[test]
    fn normalize_positive() {
        let v: Vec<f64> = vec![-2.0, 0.0, 4.0];
        let normalized = normalize(v);
        test_utils::assert_similar(&normalized, &vec![-0.5, 0.0, 1.0]);
    }
}
