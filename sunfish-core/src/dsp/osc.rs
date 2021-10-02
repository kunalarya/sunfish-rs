use serde::{Deserialize, Serialize};

use crate::util::enumerable::Enumerable;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum WaveShape {
    Sine,
    SoftSaw,
    HardSaw,
}

impl WaveShape {
    pub fn value(self) -> u8 {
        match self {
            WaveShape::Sine => 1,
            WaveShape::SoftSaw => 2,
            WaveShape::HardSaw => 3,
        }
    }

    pub fn as_string(self) -> String {
        match self {
            WaveShape::Sine => "Sine".to_string(),
            WaveShape::SoftSaw => "SoftSaw".to_string(),
            WaveShape::HardSaw => "HardSaw".to_string(),
        }
    }
}

impl Enumerable<WaveShape> for WaveShape {
    fn enumerate() -> Vec<WaveShape> {
        vec![WaveShape::Sine, WaveShape::SoftSaw, WaveShape::HardSaw]
    }
}

impl From<WaveShape> for String {
    fn from(f: WaveShape) -> String {
        f.as_string()
    }
}

impl From<String> for WaveShape {
    fn from(s: String) -> WaveShape {
        match s.as_ref() {
            "Sine" => WaveShape::Sine,
            "SoftSaw" => WaveShape::SoftSaw,
            "HardSaw" => WaveShape::HardSaw,
            _ => WaveShape::Sine,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Unison {
    Off,
    U2,
    // U4,
    // U8,
}

impl Unison {
    pub fn value(self) -> u8 {
        match self {
            Unison::Off => 0,
            Unison::U2 => 1,
            // Unison::U4 => 2,
            // Unison::U8 => 3,
        }
    }

    pub fn as_string(self) -> String {
        match self {
            Unison::Off => "Off".to_string(),
            Unison::U2 => "2 Voices".to_string(),
            // Unison::U4 => "4 Voices".to_string(),
            // Unison::U8 => "8 Voices".to_string(),
        }
    }
}

impl From<Unison> for String {
    fn from(f: Unison) -> String {
        f.as_string()
    }
}

impl From<String> for Unison {
    fn from(s: String) -> Unison {
        match s.as_ref() {
            "Off" => Unison::Off,
            "2 Voices" => Unison::U2,
            // "4 Voices" => Unison::U4,
            // "8 Voices" => Unison::U8,
            _ => Unison::Off,
        }
    }
}

impl Enumerable<Unison> for Unison {
    fn enumerate() -> Vec<Unison> {
        vec![
            Unison::Off,
            Unison::U2,
            // Unison::U4,
            // Unison::U8
        ]
    }
}
