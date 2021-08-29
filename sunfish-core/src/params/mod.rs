pub mod deltas;
pub mod fmt;
pub mod types;

use std::collections::HashMap;

use serde::Deserialize;

use crate::dsp::env::ADSR;
use crate::dsp::filter::FilterMode;
use crate::dsp::osc::{Unison, WaveShape};
use crate::lfo::{LfoRateSync, LfoShape, Rate};
use crate::modulation::target::ModulationTarget;
use crate::params::fmt::{
    BalanceFormatter, BoolOnOffFormatter, DbFormatter, Formatter, FrequencyFormatter,
    NumberFormatter, PercentFormatter, StringFormatter, TimeFormatter,
};
use crate::params::types::{Boolean, Enum, GradualTime, Linear, LinearDiscrete, ParamType};
use crate::util::enumerable::Enumerable;

// Used for converting semitones to frequency:
pub const MIN_CUTOFF_FREQ: f64 = 100.0;

pub const MIN_CUTOFF_SEMI: f64 = 0.0;
pub const MAX_CUTOFF_SEMI: f64 = 91.0;

const MIN_MOD_RATE_FREQ: f64 = 0.05; // ~20 seconds.
const MAX_MOD_RATE_FREQ: f64 = 10.0; // Cap modulation to 10 Hz.

pub const DEFAULT_FILTER: FilterMode = FilterMode::LowPass;
pub const DEFAULT_CUTOFF_SEMI: f64 = MAX_CUTOFF_SEMI;
pub const DEFAULT_RESONANCE: f64 = 1.0;
pub const DEFAULT_ENV_AMT: f64 = 0.2;

#[derive(Clone, Debug, Deserialize)]
pub struct SunfishParams {
    pub sample_rate: f64,

    // Oscillators
    pub osc1: OscParams,
    pub osc2: OscParams,

    // Filters
    pub filt1: FilterParams,
    pub filt2: FilterParams,

    // Envelopes
    pub amp_env: ADSR,
    pub mod_env: ADSR,

    // Modulation
    pub lfo1: LfoParams,
    pub lfo2: LfoParams,

    pub output_gain: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OscParams {
    pub enabled: bool,
    pub shape: WaveShape,
    pub fine_offset: f64,
    pub semitones_offset: i32,
    pub octave_offset: i32,
    pub stereo_width: f64,
    pub unison: Unison,
    pub unison_amt: f64,
    pub gain: f64,
}

impl OscParams {
    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: EOscParams,
        new_value: f64,
    ) -> Result<(), ()> {
        match eparam {
            EOscParams::Enabled => {
                self.enabled = meta.osc_enabled_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::Shape => {
                self.shape = meta.osc_shape_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::FineOffset => {
                self.fine_offset = meta.osc_fine_offset_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::SemitonesOffset => {
                self.semitones_offset = meta
                    .osc_semitones_offset_meta
                    .0
                    .vst_float_to_value(new_value);
            }
            EOscParams::OctaveOffset => {
                self.octave_offset = meta.osc_octave_offset_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::StereoWidth => {
                self.stereo_width = meta.osc_stereo_width_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::Unison => {
                self.unison = meta.osc_unison_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::UnisonAmt => {
                self.unison_amt = meta.osc_unison_amt_meta.0.vst_float_to_value(new_value);
            }
            EOscParams::Gain => {
                self.gain = meta.osc_gain_meta.0.vst_float_to_value(new_value);
            }
        }
        Ok(())
    }

    fn get_param_normalized(
        &self,
        meta: &SunfishParamsMeta,
        eparam: EOscParams,
    ) -> Result<f64, ()> {
        Ok(match eparam {
            EOscParams::Enabled => meta.osc_enabled_meta.0.value_to_vst_float(self.enabled),
            EOscParams::Shape => meta.osc_shape_meta.0.value_to_vst_float(self.shape),
            EOscParams::FineOffset => meta
                .osc_fine_offset_meta
                .0
                .value_to_vst_float(self.fine_offset),
            EOscParams::SemitonesOffset => meta
                .osc_semitones_offset_meta
                .0
                .value_to_vst_float(self.semitones_offset),
            EOscParams::OctaveOffset => meta
                .osc_octave_offset_meta
                .0
                .value_to_vst_float(self.octave_offset),
            EOscParams::StereoWidth => meta
                .osc_stereo_width_meta
                .0
                .value_to_vst_float(self.stereo_width),
            EOscParams::Unison => meta.osc_unison_meta.0.value_to_vst_float(self.unison),
            EOscParams::UnisonAmt => meta
                .osc_unison_amt_meta
                .0
                .value_to_vst_float(self.unison_amt),
            EOscParams::Gain => meta.osc_gain_meta.0.value_to_vst_float(self.gain),
        })
    }

    fn format_value(&self, meta: &SunfishParamsMeta, eparam: EOscParams) -> Result<String, ()> {
        Ok(match eparam {
            EOscParams::Enabled => meta.osc_enabled_meta.1.format_value(self.enabled),
            EOscParams::Shape => meta.osc_shape_meta.1.format_value(self.shape),
            EOscParams::FineOffset => meta.osc_fine_offset_meta.1.format_value(self.fine_offset),
            EOscParams::SemitonesOffset => meta
                .osc_semitones_offset_meta
                .1
                .format_value(self.semitones_offset),
            EOscParams::OctaveOffset => meta
                .osc_octave_offset_meta
                .1
                .format_value(self.octave_offset),
            EOscParams::StereoWidth => meta.osc_stereo_width_meta.1.format_value(self.stereo_width),
            EOscParams::Unison => meta.osc_unison_meta.1.format_value(self.unison),
            EOscParams::UnisonAmt => meta.osc_unison_amt_meta.1.format_value(self.unison_amt),
            EOscParams::Gain => meta.osc_gain_meta.1.format_value(self.gain),
        })
    }
}

impl Default for OscParams {
    fn default() -> Self {
        Self {
            enabled: true,
            shape: WaveShape::Sine,
            fine_offset: 0.0,
            semitones_offset: 0,
            octave_offset: 0,
            stereo_width: 0.0,
            unison: Unison::Off,
            unison_amt: 1.0,
            gain: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct FilterParams {
    pub enable: bool,
    pub cutoff_semi: f64,
    pub resonance: f64,
    pub mode: FilterMode,
    pub env_amt: f64,
}

impl FilterParams {
    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: EFiltParams,
        new_value: f64,
    ) -> Result<(), ()> {
        match eparam {
            EFiltParams::Enable => {
                self.enable = meta.filter_enable_meta.0.vst_float_to_value(new_value);
            }
            EFiltParams::Cutoff => {
                self.cutoff_semi = meta.cutoff_meta.0.vst_float_to_value(new_value);
            }
            EFiltParams::Resonance => {
                self.resonance = meta.resonance_meta.0.vst_float_to_value(new_value);
            }
            EFiltParams::Mode => {
                self.mode = meta.mode_meta.0.vst_float_to_value(new_value);
            }
            EFiltParams::EnvAmt => {
                self.env_amt = meta.env_amt_meta.0.vst_float_to_value(new_value);
            }
        };
        Ok(())
    }

    fn get_param_normalized(
        &self,
        meta: &SunfishParamsMeta,
        eparam: EFiltParams,
    ) -> Result<f64, ()> {
        Ok(match eparam {
            EFiltParams::Enable => meta.filter_enable_meta.0.value_to_vst_float(self.enable),
            EFiltParams::Cutoff => meta.cutoff_meta.0.value_to_vst_float(self.cutoff_semi),
            EFiltParams::Resonance => meta.resonance_meta.0.value_to_vst_float(self.resonance),
            EFiltParams::Mode => meta.mode_meta.0.value_to_vst_float(self.mode),
            EFiltParams::EnvAmt => meta.env_amt_meta.0.value_to_vst_float(self.env_amt),
        })
    }

    fn format_value(&self, meta: &SunfishParamsMeta, eparam: EFiltParams) -> Result<String, ()> {
        Ok(match eparam {
            EFiltParams::Enable => meta.filter_enable_meta.1.format_value(self.enable),
            EFiltParams::Cutoff => meta.cutoff_meta.1.format_value(self.cutoff_semi),
            EFiltParams::Resonance => meta.resonance_meta.1.format_value(self.resonance),
            EFiltParams::Mode => meta.mode_meta.1.format_value(self.mode),
            EFiltParams::EnvAmt => meta.env_amt_meta.1.format_value(self.env_amt),
        })
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            enable: true,
            cutoff_semi: DEFAULT_CUTOFF_SEMI,
            resonance: DEFAULT_RESONANCE,
            mode: DEFAULT_FILTER,
            env_amt: DEFAULT_ENV_AMT,
        }
    }
}

impl ADSR {
    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: EAdsrParams,
        new_value: f64,
    ) -> Result<(), ()> {
        match eparam {
            EAdsrParams::Attack => {
                self.attack = meta.attack_meta.0.vst_float_to_value(new_value);
            }
            EAdsrParams::Decay => {
                self.decay = meta.decay_meta.0.vst_float_to_value(new_value);
            }
            EAdsrParams::Sustain => {
                self.sustain = meta.sustain_meta.0.vst_float_to_value(new_value);
            }
            EAdsrParams::Release => {
                self.release = meta.release_meta.0.vst_float_to_value(new_value);
            }
        }
        Ok(())
    }
    fn get_param_normalized(
        &self,
        meta: &SunfishParamsMeta,
        eparam: EAdsrParams,
    ) -> Result<f64, ()> {
        Ok(match eparam {
            EAdsrParams::Attack => meta.attack_meta.0.value_to_vst_float(self.attack),
            EAdsrParams::Decay => meta.decay_meta.0.value_to_vst_float(self.decay),
            EAdsrParams::Sustain => meta.sustain_meta.0.value_to_vst_float(self.sustain),
            EAdsrParams::Release => meta.release_meta.0.value_to_vst_float(self.release),
        })
    }
    fn format_value(&self, meta: &SunfishParamsMeta, eparam: EAdsrParams) -> Result<String, ()> {
        Ok(match eparam {
            EAdsrParams::Attack => meta.attack_meta.1.format_value(self.attack),
            EAdsrParams::Decay => meta.decay_meta.1.format_value(self.decay),
            EAdsrParams::Sustain => meta.sustain_meta.1.format_value(self.sustain),
            EAdsrParams::Release => meta.release_meta.1.format_value(self.release),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LfoParams {
    pub target: ModulationTarget,
    pub shape: LfoShape,
    pub sync: bool,
    pub amt: f64,
    pub rate: Rate,
}

impl LfoParams {
    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: ELfoParams,
        new_value: f64,
    ) -> Result<(), ()> {
        match eparam {
            ELfoParams::Target => {
                self.target = meta.mod_target_meta.0.vst_float_to_value(new_value);
            }
            ELfoParams::Shape => {
                self.shape = meta.mod_shape_meta.0.vst_float_to_value(new_value);
            }
            ELfoParams::Synced => {
                self.sync = meta.mod_sync_meta.0.vst_float_to_value(new_value);
            }
            ELfoParams::Rate => {
                if self.sync {
                    self.rate =
                        Rate::Synced(meta.mod_rate_synced_meta.0.vst_float_to_value(new_value));
                } else {
                    self.rate = Rate::Hz(meta.mod_rate_hz_meta.0.vst_float_to_value(new_value));
                }
            }
            ELfoParams::Amt => {
                self.amt = meta.mod_amt_meta.0.vst_float_to_value(new_value);
            }
        };
        Ok(())
    }
    fn get_param_normalized(
        &self,
        meta: &SunfishParamsMeta,
        eparam: ELfoParams,
    ) -> Result<f64, ()> {
        Ok(match eparam {
            ELfoParams::Target => meta.mod_target_meta.0.value_to_vst_float(self.target),
            ELfoParams::Shape => meta.mod_shape_meta.0.value_to_vst_float(self.shape),
            ELfoParams::Synced => meta.mod_sync_meta.0.value_to_vst_float(self.sync),
            ELfoParams::Rate => match self.rate {
                Rate::Hz(rate_hz) => meta.mod_rate_hz_meta.0.value_to_vst_float(rate_hz),
                Rate::Synced(rate_synced) => {
                    meta.mod_rate_synced_meta.0.value_to_vst_float(rate_synced)
                }
            },
            ELfoParams::Amt => meta.mod_amt_meta.0.value_to_vst_float(self.amt),
        })
    }
    fn format_value(&self, meta: &SunfishParamsMeta, eparam: ELfoParams) -> Result<String, ()> {
        Ok(match eparam {
            ELfoParams::Target => meta.mod_target_meta.1.format_value(self.target),
            ELfoParams::Shape => meta.mod_shape_meta.1.format_value(self.shape),
            ELfoParams::Synced => meta.mod_sync_meta.1.format_value(self.sync),
            ELfoParams::Rate => match self.rate {
                Rate::Hz(rate_hz) => meta.mod_rate_hz_meta.1.format_value(rate_hz),
                Rate::Synced(rate_synced) => meta.mod_rate_synced_meta.1.format_value(rate_synced),
            },
            ELfoParams::Amt => meta.mod_amt_meta.1.format_value(self.amt),
        })
    }
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            target: ModulationTarget::Off,
            shape: LfoShape::Triangle,
            sync: true,
            amt: 1.0,
            rate: Rate::Synced(LfoRateSync::R1_4),
        }
    }
}

// Enums (TODO: Maybe figure out how to macroize these?)
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum EParam {
    // Oscillators
    Osc1(EOscParams),
    Osc2(EOscParams),

    // Filters
    Filt1(EFiltParams),
    Filt2(EFiltParams),

    // Envelopes
    AmpEnv(EAdsrParams),
    ModEnv(EAdsrParams),

    // Modulation
    Lfo1(ELfoParams),
    Lfo2(ELfoParams),

    // Global Gain
    OutputGain,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum EOscParams {
    Enabled,
    Shape,
    FineOffset,
    SemitonesOffset,
    OctaveOffset,
    StereoWidth,
    Unison,
    UnisonAmt,
    Gain,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum EFiltParams {
    Enable,
    Cutoff,
    Resonance,
    Mode,
    EnvAmt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum EAdsrParams {
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub enum ELfoParams {
    Target,
    Shape,
    Synced,
    Rate,
    Amt,
}

// Names.
impl EParam {
    pub fn to_string(&self, short: bool) -> String {
        //"todo".to_string()
        let param_name = match self {
            Self::Osc1(e) => e.to_string(short),
            Self::Osc2(e) => e.to_string(short),
            Self::Filt1(e) => e.to_string(short),
            Self::Filt2(e) => e.to_string(short),
            Self::AmpEnv(e) => e.to_string(short),
            Self::ModEnv(e) => e.to_string(short),
            Self::Lfo1(e) => e.to_string(short),
            Self::Lfo2(e) => e.to_string(short),
            Self::OutputGain => "Output Gain".to_string(),
        };
        if short {
            param_name.to_string()
        } else {
            let prefix = match self {
                Self::Osc1(_) => "Osc1",
                Self::Osc2(_) => "Osc2",
                Self::Filt1(_) => "Filt1",
                Self::Filt2(_) => "Filt2",
                Self::AmpEnv(_) => "AmpEnv",
                Self::ModEnv(_) => "ModEnv",
                Self::Lfo1(_) => "Osc1",
                Self::Lfo2(_) => "Osc1",
                Self::OutputGain => "",
            };
            format!("{}:{}", prefix, param_name)
        }
    }
    fn get_names() -> Vec<(EParam, String)> {
        let mut names = vec![];
        // Oscillators.
        for (param, name) in EOscParams::get_names() {
            names.push((EParam::Osc1(param), format!("Osc1:{}", name)));
            names.push((EParam::Osc2(param), format!("Osc2:{}", name)));
        }
        // Filters.
        for (param, name) in EFiltParams::get_names() {
            names.push((EParam::Filt1(param), format!("Filt1:{}", name)));
            names.push((EParam::Filt2(param), format!("Filt2:{}", name)));
        }
        // Envelopes
        for (param, name) in EAdsrParams::get_names() {
            names.push((EParam::AmpEnv(param), format!("Amp Env:{}", name)));
            names.push((EParam::ModEnv(param), format!("Mod Env:{}", name)));
        }
        // Modulation
        for (param, name) in ELfoParams::get_names() {
            names.push((EParam::Lfo1(param), format!("Mod LFO1:{}", name)));
            names.push((EParam::Lfo2(param), format!("Mod LFO2:{}", name)));
        }
        // Output Gain
        names.push((EParam::OutputGain, "Output Gain".to_string()));
        names
    }
}
impl EOscParams {
    fn to_string(&self, _short: bool) -> String {
        let s = match self {
            Self::Enabled => "Enabled",
            Self::Shape => "Shape",
            Self::FineOffset => "Offset (Fine)",
            Self::SemitonesOffset => "Offset (Semitones)",
            Self::OctaveOffset => "Offset (Octave)",
            Self::StereoWidth => "Stereo Width",
            Self::Unison => "Unison",
            Self::UnisonAmt => "Unison Amount",
            Self::Gain => "Gain",
        };
        s.to_string()
    }
    fn get_names() -> Vec<(EOscParams, String)> {
        vec![
            (Self::Enabled, "Enabled".to_owned()),
            (Self::Shape, "Shape".to_owned()),
            (Self::FineOffset, "Offset (Fine)".to_owned()),
            (Self::SemitonesOffset, "Offset (Semitones)".to_owned()),
            (Self::OctaveOffset, "Offset (Octave)".to_string()),
            (Self::StereoWidth, "Stereo Width".to_string()),
            (Self::Unison, "Unison".to_string()),
            (Self::UnisonAmt, "Unison Amount".to_string()),
            (Self::Gain, "Gain".to_string()),
        ]
    }
}
impl EFiltParams {
    fn to_string(&self, _short: bool) -> String {
        let s = match self {
            Self::Enable => "Enable",
            Self::Cutoff => "Cutoff",
            Self::Resonance => "Resonance",
            Self::Mode => "Mode",
            Self::EnvAmt => "Env Amount",
        };
        s.to_string()
    }
    fn get_names() -> Vec<(EFiltParams, String)> {
        vec![
            (Self::Enable, "Enable".to_string()),
            (Self::Cutoff, "Cutoff".to_string()),
            (Self::Resonance, "Resonance".to_string()),
            (Self::Mode, "Mode".to_string()),
            (Self::EnvAmt, "EnvAmt".to_string()),
        ]
    }
}

impl EAdsrParams {
    fn to_string(&self, _short: bool) -> String {
        let s = match self {
            Self::Attack => "Attack",
            Self::Decay => "Decay",
            Self::Sustain => "Sustain",
            Self::Release => "Release",
        };
        s.to_string()
    }
    fn get_names() -> Vec<(EAdsrParams, String)> {
        vec![
            (Self::Attack, "Attack".to_string()),
            (Self::Decay, "Decay".to_string()),
            (Self::Sustain, "Sustain".to_string()),
            (Self::Release, "Release".to_string()),
        ]
    }
}

impl ELfoParams {
    fn to_string(&self, _short: bool) -> String {
        let s = match self {
            Self::Target => "Target",
            Self::Shape => "Shape",
            Self::Synced => "Sync",
            Self::Rate => "Rate",
            Self::Amt => "Amount",
        };
        s.to_string()
    }
    fn get_names() -> Vec<(ELfoParams, String)> {
        vec![
            (Self::Target, "Target".to_string()),
            (Self::Shape, "Shape".to_string()),
            (Self::Synced, "Sync".to_string()),
            (Self::Rate, "Rate".to_string()),
            (Self::Amt, "Amount".to_string()),
        ]
    }
}

// Metadata per parameter.
#[derive(Clone, Debug)]
struct ParamMeta {
    name: String,
}

impl ParamMeta {
    fn new(name: String) -> Self {
        ParamMeta { name }
    }
}

// Stores all of the Metadata associated with the parameters.
#[derive(Clone, Debug)]
pub struct SunfishParamsMeta {
    // These are classes of parameters:

    // Oscillators
    pub osc_enabled_meta: (Boolean, BoolOnOffFormatter),
    pub osc_shape_meta: (Enum<WaveShape>, StringFormatter),
    pub osc_fine_offset_meta: (Linear, FrequencyFormatter),
    pub osc_semitones_offset_meta: (LinearDiscrete, NumberFormatter),
    pub osc_octave_offset_meta: (LinearDiscrete, NumberFormatter),
    pub osc_stereo_width_meta: (Linear, BalanceFormatter),
    pub osc_unison_meta: (Enum<Unison>, StringFormatter),
    pub osc_unison_amt_meta: (Linear, FrequencyFormatter),
    pub osc_gain_meta: (Linear, DbFormatter),

    // Filters
    pub filter_enable_meta: (Boolean, BoolOnOffFormatter),
    pub cutoff_meta: (Linear, NumberFormatter),
    pub resonance_meta: (Linear, NumberFormatter),
    pub mode_meta: (Enum<FilterMode>, StringFormatter),
    pub env_amt_meta: (Linear, PercentFormatter),

    // Envelopes
    pub attack_meta: (GradualTime, TimeFormatter),
    pub decay_meta: (GradualTime, TimeFormatter),
    pub sustain_meta: (Linear, PercentFormatter),
    pub release_meta: (Linear, TimeFormatter),

    // Modulation
    pub mod_target_meta: (Enum<ModulationTarget>, StringFormatter),
    pub mod_shape_meta: (Enum<LfoShape>, StringFormatter),
    // TODO: Use a non-linear rate
    pub mod_sync_meta: (Boolean, BoolOnOffFormatter),
    pub mod_rate_hz_meta: (Linear, NumberFormatter),
    pub mod_rate_synced_meta: (Enum<LfoRateSync>, StringFormatter),
    pub mod_amt_meta: (Linear, NumberFormatter),

    pub output_gain_meta: (Linear, DbFormatter),

    pub paramlist: Vec<EParam>,
    param_to_index: HashMap<EParam, usize>,
    params: HashMap<EParam, ParamMeta>,
}

impl SunfishParamsMeta {
    pub fn new() -> Self {
        /*
         * The VST interface retrieves parameters by index. Additionally, we would like to store
         * metadata per parameter (such as the name). All parameters are internal referenced
         * through the EParam enum.
         *
         * We do two things in this function:
         * 1. Create a mapping between EParam and ParamMeta (the metadata).
         * 2. Create a list of all EParam choices to be able to look them up by index.
         *    In theory, we could do this on initialization by iterating the hashmap's keys
         *    but we would like the order to be consistent across executions and compilations.
         *
         */
        let (paramlist, param_to_index, params) = {
            // This is the authoritative source of per-param metadata (minus the type).
            let param_metas: Vec<(EParam, String)> = EParam::get_names();

            // Allow index to eparam lookup.
            let mut param_to_index: HashMap<EParam, usize> = HashMap::new();

            // Create the lookup between EParam and the associated metadata.
            let mut m: HashMap<EParam, ParamMeta> = HashMap::new();
            for (index, (eparam, name)) in param_metas.iter().enumerate() {
                m.insert(*eparam, ParamMeta::new(name.to_string()));
                param_to_index.insert(*eparam, index);
            }

            // And finally, VST index to EParam.
            let paramlist: Vec<EParam> = param_metas.iter().map(|(eparam, _)| *eparam).collect();
            (paramlist, param_to_index, m)
        };
        SunfishParamsMeta {
            // Oscillators
            osc_enabled_meta: (Boolean::new(), BoolOnOffFormatter()),
            osc_shape_meta: (Enum::new(WaveShape::enumerate()), StringFormatter()),
            osc_fine_offset_meta: (Linear::new(-1.0, 1.0), FrequencyFormatter()),
            osc_semitones_offset_meta: (LinearDiscrete::new(-24, 24), NumberFormatter()),
            osc_octave_offset_meta: (LinearDiscrete::new(-3, 3), NumberFormatter()),
            osc_stereo_width_meta: (Linear::new(-3.0, 3.0), BalanceFormatter()),
            osc_unison_meta: (Enum::new(Unison::enumerate()), StringFormatter()),
            osc_unison_amt_meta: (Linear::new(0.0, 3.0), FrequencyFormatter()),
            osc_gain_meta: (Linear::new(0.0, 1.0), DbFormatter()),

            // Filters
            filter_enable_meta: (Boolean::new(), BoolOnOffFormatter()),
            cutoff_meta: (
                Linear::new(MIN_CUTOFF_SEMI, MAX_CUTOFF_SEMI),
                NumberFormatter(),
            ),
            resonance_meta: (Linear::new(0.5, 2.0), NumberFormatter()),
            mode_meta: (Enum::new(FilterMode::enumerate()), StringFormatter()),
            env_amt_meta: (Linear::new(0.0, 1.0), PercentFormatter()),

            // Envelopes
            attack_meta: (GradualTime::for_attack(), TimeFormatter()),
            decay_meta: (GradualTime::for_decay(), TimeFormatter()),
            sustain_meta: (Linear::new(0.0, 1.0), PercentFormatter()),
            release_meta: (Linear::new(0.0, 5.0), TimeFormatter()),

            // Modulation
            mod_target_meta: (Enum::new(ModulationTarget::enumerate()), StringFormatter()),
            mod_shape_meta: (Enum::new(LfoShape::enumerate()), StringFormatter()),
            mod_sync_meta: ((Boolean::new(), BoolOnOffFormatter())),
            mod_rate_hz_meta: (
                Linear::new(MIN_MOD_RATE_FREQ, MAX_MOD_RATE_FREQ),
                NumberFormatter(),
            ),
            mod_rate_synced_meta: (Enum::new(LfoRateSync::enumerate()), StringFormatter()),
            mod_amt_meta: (Linear::new(0.0, 1.0), NumberFormatter()),

            // Global Gain
            output_gain_meta: (Linear::new(0.0, 2.0), DbFormatter()),

            paramlist,
            param_to_index,
            params,
        }
    }

    pub fn count(&self) -> usize {
        self.paramlist.len()
    }

    pub fn param_to_index(&self, param: &EParam) -> Option<usize> {
        self.param_to_index.get(param).map(|item| *item)
    }
}

impl SunfishParams {
    pub fn new(sample_rate: f64) -> Self {
        SunfishParams {
            sample_rate,
            osc1: OscParams::default(),
            osc2: OscParams::default(),
            filt1: FilterParams::default(),
            filt2: FilterParams::default(),
            amp_env: ADSR::default(),
            mod_env: ADSR::default(),
            lfo1: LfoParams::default(),
            lfo2: LfoParams::default(),
            output_gain: 1.0,
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
    }
}

pub trait NormalizedParams {
    fn update_param_by_index(
        &mut self,
        meta: &SunfishParamsMeta,
        index: usize,
        new_value: f64,
    ) -> Result<EParam, ()>;
    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: EParam,
        new_value: f64,
    ) -> Result<(), ()>;
    fn get_param_by_index(&self, meta: &SunfishParamsMeta, index: usize) -> Result<f64, ()>;
    fn get_param_normalized(&self, meta: &SunfishParamsMeta, eparam: EParam) -> Result<f64, ()>;
    fn index_param(&self, meta: &SunfishParamsMeta, index: usize) -> Result<EParam, ()>;
    fn get_name(&self, meta: &SunfishParamsMeta, index: usize) -> Result<String, ()>;
    fn formatted_value(&self, meta: &SunfishParamsMeta, eparam: EParam) -> Result<String, ()>;
    fn formatted_value_by_index(
        &self,
        meta: &SunfishParamsMeta,
        index: usize,
    ) -> Result<String, ()> {
        self.formatted_value(meta, self.index_param(meta, index)?)
    }
}

impl NormalizedParams for SunfishParams {
    /// Update parameters from VST values, i.e. between 0.0 and 1.0.
    fn update_param_by_index(
        &mut self,
        meta: &SunfishParamsMeta,
        index: usize,
        mut new_value: f64,
    ) -> Result<EParam, ()> {
        let eparam = self.index_param(meta, index)?;

        // Bad or corrupted presets occasionally cause issues; we silently correct them for now.
        if new_value > 1.0 {
            new_value = 1.0;
        }
        if new_value < 0.0 {
            new_value = 0.0;
        }

        self.update_param(meta, eparam, new_value)?;
        Ok(eparam)
    }

    fn get_param_by_index(&self, meta: &SunfishParamsMeta, index: usize) -> Result<f64, ()> {
        let eparam = self.index_param(meta, index)?;
        self.get_param_normalized(meta, eparam)
    }

    fn index_param(&self, meta: &SunfishParamsMeta, index: usize) -> Result<EParam, ()> {
        if index < meta.paramlist.len() {
            Ok(meta.paramlist[index])
        } else {
            Err(())
        }
    }

    fn get_name(&self, meta: &SunfishParamsMeta, index: usize) -> Result<String, ()> {
        let eparam = self.index_param(meta, index)?;
        if let Some(param_meta) = meta.params.get(&eparam) {
            Ok(param_meta.name.clone())
        } else {
            Ok("(error)".to_string())
        }
    }

    fn update_param(
        &mut self,
        meta: &SunfishParamsMeta,
        eparam: EParam,
        new_value: f64,
    ) -> Result<(), ()> {
        // TODO: Update params
        match eparam {
            EParam::Osc1(osc_param) => {
                self.osc1.update_param(&meta, osc_param, new_value)?;
            }
            EParam::Osc2(osc_param) => {
                self.osc2.update_param(&meta, osc_param, new_value)?;
            }
            EParam::Filt1(filt_param) => {
                self.filt1.update_param(&meta, filt_param, new_value)?;
            }
            EParam::Filt2(filt_param) => {
                self.filt2.update_param(&meta, filt_param, new_value)?;
            }
            EParam::AmpEnv(env_param) => {
                self.amp_env.update_param(&meta, env_param, new_value)?;
            }
            EParam::ModEnv(env_param) => {
                self.mod_env.update_param(&meta, env_param, new_value)?;
            }
            EParam::Lfo1(lfo_param) => {
                self.lfo1.update_param(&meta, lfo_param, new_value)?;
            }
            EParam::Lfo2(lfo_param) => {
                self.lfo2.update_param(&meta, lfo_param, new_value)?;
            }
            EParam::OutputGain => {
                self.output_gain = meta.output_gain_meta.0.vst_float_to_value(new_value);
            }
        };
        Ok(())
    }

    fn get_param_normalized(&self, meta: &SunfishParamsMeta, eparam: EParam) -> Result<f64, ()> {
        // Return 1-normalized value.
        Ok(match eparam {
            EParam::Osc1(osc_param) => self.osc1.get_param_normalized(&meta, osc_param)?,
            EParam::Osc2(osc_param) => self.osc2.get_param_normalized(&meta, osc_param)?,
            EParam::Filt1(filt_param) => self.filt1.get_param_normalized(&meta, filt_param)?,
            EParam::Filt2(filt_param) => self.filt2.get_param_normalized(&meta, filt_param)?,
            EParam::AmpEnv(env_param) => self.amp_env.get_param_normalized(&meta, env_param)?,
            EParam::ModEnv(env_param) => self.mod_env.get_param_normalized(&meta, env_param)?,
            EParam::Lfo1(lfo_param) => self.lfo1.get_param_normalized(&meta, lfo_param)?,
            EParam::Lfo2(lfo_param) => self.lfo2.get_param_normalized(&meta, lfo_param)?,
            EParam::OutputGain => meta.output_gain_meta.0.value_to_vst_float(self.output_gain),
        })
    }

    fn formatted_value(&self, meta: &SunfishParamsMeta, eparam: EParam) -> Result<String, ()> {
        let result = match eparam {
            EParam::Osc1(osc_param) => self.osc1.format_value(&meta, osc_param)?,
            EParam::Osc2(osc_param) => self.osc2.format_value(&meta, osc_param)?,
            EParam::Filt1(filt_param) => self.filt1.format_value(&meta, filt_param)?,
            EParam::Filt2(filt_param) => self.filt2.format_value(&meta, filt_param)?,
            EParam::AmpEnv(env_param) => self.amp_env.format_value(&meta, env_param)?,
            EParam::ModEnv(env_param) => self.mod_env.format_value(&meta, env_param)?,
            EParam::Lfo1(lfo_param) => self.lfo1.format_value(&meta, lfo_param)?,
            EParam::Lfo2(lfo_param) => self.lfo2.format_value(&meta, lfo_param)?,
            EParam::OutputGain => meta.output_gain_meta.1.format_value(self.output_gain),
        };
        Ok(result)
    }
}
