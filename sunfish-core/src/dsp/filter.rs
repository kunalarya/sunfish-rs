use serde::{Deserialize, Serialize};

use crate::dsp::biquad::{biquad_direct_form_apply, BiquadCoefs};
use crate::dsp::smoothing::SlewRateLimiter;
use crate::params::MIN_CUTOFF_FREQ;
use crate::util;
use crate::util::enumerable::Enumerable;

// TODO: Investigate ideal stable slew rate.

/// Slew rate, in Hz, for smoothing out filter parameters and avoiding instability regions.
const SLEW_RATE_HZ: f64 = 1000.0;
/// Slew rate, in seconds, for parameter smoothing.
const SLEW_RATE_S: f64 = 1.0 / SLEW_RATE_HZ;
const SLEW_THRESHOLD_SEMIS: f64 = 0.001;
const SLEW_THRESHOLD_RES: f64 = 0.001;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum FilterMode {
    LowPass,
    HighPass,
    BandPass,
    PassThru,
}

impl Enumerable<FilterMode> for FilterMode {
    fn enumerate() -> Vec<FilterMode> {
        vec![
            FilterMode::LowPass,
            FilterMode::HighPass,
            FilterMode::BandPass,
            FilterMode::PassThru,
        ]
    }
}

impl From<FilterMode> for String {
    fn from(f: FilterMode) -> String {
        match f {
            FilterMode::LowPass => "LowPass".to_string(),
            FilterMode::HighPass => "HighPass".to_string(),
            FilterMode::BandPass => "BandPass".to_string(),
            FilterMode::PassThru => "PassThru".to_string(),
        }
    }
}

impl From<String> for FilterMode {
    fn from(s: String) -> FilterMode {
        match s.as_ref() {
            "LowPass" => FilterMode::LowPass,
            "HighPass" => FilterMode::HighPass,
            "BandPass" => FilterMode::BandPass,
            "PassThru" => FilterMode::PassThru,
            _ => panic!("Invalid filter mode!"),
        }
    }
}

impl FilterMode {
    pub fn from_string(value: String) -> FilterMode {
        match value.as_ref() {
            "LowPass" => FilterMode::LowPass,
            "HighPass" => FilterMode::HighPass,
            "BandPass" => FilterMode::BandPass,
            "PassThru" => FilterMode::PassThru,
            _ => panic!("Invalid filter mode!"),
        }
    }
}

// We will process filters over vectors; we only need to store the last two input and output
// points.
#[derive(Debug)]
pub struct Filter {
    coeffs: BiquadCoefs,
    sample_rate: f64,

    mode: FilterMode,
    cutoff_semi: f64,
    cutoff_semi_srl: SlewRateLimiter,
    resonance: f64,
    resonance_srl: SlewRateLimiter,

    prev_xn1: f64,
    prev_xn2: f64,
    prev_yn1: f64,
    prev_yn2: f64,
}

impl Filter {
    pub fn new(sample_rate: f64, mode: &FilterMode, cutoff_semi: &f64, resonance: &f64) -> Filter {
        let cutoff_semi_srl =
            SlewRateLimiter::new(*cutoff_semi, sample_rate, SLEW_RATE_S, SLEW_THRESHOLD_SEMIS);
        let resonance_srl =
            SlewRateLimiter::new(*resonance, sample_rate, SLEW_RATE_S, SLEW_THRESHOLD_RES);
        let mut inst = Filter {
            coeffs: BiquadCoefs::zeros(),
            sample_rate,
            mode: *mode,
            cutoff_semi: 0.0, // updated with set_cutoff below
            cutoff_semi_srl,
            resonance: 0.0, // likewise
            resonance_srl,
            prev_xn1: 0.0,
            prev_xn2: 0.0,
            prev_yn1: 0.0,
            prev_yn2: 0.0,
        };
        inst.set_cutoff(*cutoff_semi);
        inst.set_resonance(*resonance);
        inst.update_coeff();
        inst
    }

    /// Update the filter mode (i.e. low-pass, high-pass, etc.)
    pub fn set_mode(&mut self, mode: &FilterMode) {
        self.mode = *mode;
        self.update_coeff();
    }

    /// Update the cutoff frequency, specified in semitones.
    pub fn set_cutoff(&mut self, cutoff_semi: f64) {
        self.cutoff_semi = cutoff_semi;
        self.cutoff_semi_srl.update(cutoff_semi);
    }

    /// Update the resonance.
    pub fn set_resonance(&mut self, resonance: f64) {
        self.resonance = resonance;
        self.resonance_srl.update(resonance);
    }

    fn update_coeff(&mut self) {
        // TODO: Do we need to invalidate prev_* values?
        let cutoff_semi = self.cutoff_semi_srl.filtered_value;
        let cutoff_hz = util::semitones_to_frequency(cutoff_semi, MIN_CUTOFF_FREQ);
        let resonance = self.resonance_srl.filtered_value;
        match &self.mode {
            FilterMode::LowPass => {
                self.coeffs = BiquadCoefs::lpf(self.sample_rate, cutoff_hz, resonance);
            }
            FilterMode::HighPass => {
                self.coeffs = BiquadCoefs::hpf(self.sample_rate, cutoff_hz, resonance);
            }
            FilterMode::BandPass => {
                self.coeffs = BiquadCoefs::bpf(self.sample_rate, cutoff_hz, resonance);
            }
            FilterMode::PassThru => {}
        };
    }

    /// Apply the filter to the given input signal.
    pub fn apply(&mut self, input: f64) -> f64 {
        // Determine if we need to update
        let cutoff_changed = self.cutoff_semi_srl.step();
        let res_changed = self.resonance_srl.step();
        if cutoff_changed || res_changed {
            self.update_coeff();
        }

        let output = biquad_direct_form_apply(
            input,
            &self.coeffs,
            self.prev_xn1,
            self.prev_xn2,
            self.prev_yn1,
            self.prev_yn2,
        );
        self.prev_xn2 = self.prev_xn1;
        self.prev_xn1 = input;
        self.prev_yn2 = self.prev_yn1;
        self.prev_yn1 = output;
        output
    }
}
