use serde::{Deserialize, Serialize};

use crate::util::enumerable::Enumerable;

/// The VST parameter representation of a modulation
/// target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ModulationTarget {
    Off,
    Osc1Frequency,
    Osc1StereoWidth,
    Osc1UnisonAmt,

    Filter1Cutoff,
    Filter1Resonance,

    Osc2Frequency,
    Osc2StereoWidth,
    Osc2UnisonAmt,

    Filter2Cutoff,
    Filter2Resonance,
}

impl ModulationTarget {
    pub fn value(self) -> u8 {
        match self {
            ModulationTarget::Off => 0,
            ModulationTarget::Osc1Frequency => 1,
            ModulationTarget::Osc1StereoWidth => 2,
            ModulationTarget::Osc1UnisonAmt => 3,

            ModulationTarget::Filter1Cutoff => 4,
            ModulationTarget::Filter1Resonance => 5,

            ModulationTarget::Osc2Frequency => 6,
            ModulationTarget::Osc2StereoWidth => 7,
            ModulationTarget::Osc2UnisonAmt => 8,

            ModulationTarget::Filter2Cutoff => 9,
            ModulationTarget::Filter2Resonance => 10,
        }
    }

    pub fn as_string(self) -> String {
        let as_str = match self {
            ModulationTarget::Off => "Off",
            ModulationTarget::Osc1Frequency => "Osc1Frequency",
            ModulationTarget::Osc1StereoWidth => "Osc1StereoWidth",
            ModulationTarget::Osc1UnisonAmt => "Osc1UnisonAmt",

            ModulationTarget::Filter1Cutoff => "Filter1Cutoff",
            ModulationTarget::Filter1Resonance => "Filter1Resonance",

            ModulationTarget::Osc2Frequency => "Osc2Frequency",
            ModulationTarget::Osc2StereoWidth => "Osc2StereoWidth",
            ModulationTarget::Osc2UnisonAmt => "Osc2UnisonAmt",

            ModulationTarget::Filter2Cutoff => "Filter2Cutoff",
            ModulationTarget::Filter2Resonance => "Filter2Resonance",
        };
        as_str.to_string()
    }
}

impl From<ModulationTarget> for String {
    fn from(f: ModulationTarget) -> String {
        f.as_string()
    }
}

impl From<String> for ModulationTarget {
    fn from(s: String) -> ModulationTarget {
        match s.as_ref() {
            "Off" => ModulationTarget::Off,
            "Osc1Frequency" => ModulationTarget::Osc1Frequency,
            "Osc1StereoWidth" => ModulationTarget::Osc1StereoWidth,
            "Osc1UnisonAmt" => ModulationTarget::Osc1UnisonAmt,

            "Filter1Cutoff" => ModulationTarget::Filter1Cutoff,
            "Filter1Resonance" => ModulationTarget::Filter1Resonance,

            "Osc2Frequency" => ModulationTarget::Osc2Frequency,
            "Osc2StereoWidth" => ModulationTarget::Osc2StereoWidth,
            "Osc2UnisonAmt" => ModulationTarget::Osc2UnisonAmt,

            "Filter2Cutoff" => ModulationTarget::Filter2Cutoff,
            "Filter2Resonance" => ModulationTarget::Filter2Resonance,
            _ => ModulationTarget::Off,
        }
    }
}

impl Enumerable<ModulationTarget> for ModulationTarget {
    fn enumerate() -> Vec<ModulationTarget> {
        vec![
            ModulationTarget::Off,
            ModulationTarget::Osc1Frequency,
            ModulationTarget::Osc1StereoWidth,
            ModulationTarget::Osc1UnisonAmt,
            ModulationTarget::Filter1Cutoff,
            ModulationTarget::Filter1Resonance,
            ModulationTarget::Osc2Frequency,
            ModulationTarget::Osc2StereoWidth,
            ModulationTarget::Osc2UnisonAmt,
            ModulationTarget::Filter2Cutoff,
            ModulationTarget::Filter2Resonance,
        ]
    }
}
