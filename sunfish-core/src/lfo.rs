use serde::{Deserialize, Serialize};

use crate::dsp::TAU;
use crate::util::enumerable::Enumerable;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LfoShape {
    Sine,
    Saw,
    Triangle,
}

impl LfoShape {
    pub fn value(self) -> u8 {
        match self {
            LfoShape::Sine => 0,
            LfoShape::Saw => 1,
            LfoShape::Triangle => 2,
        }
    }

    pub fn as_string(self) -> String {
        match self {
            LfoShape::Sine => "Sine".to_string(),
            LfoShape::Saw => "Saw".to_string(),
            LfoShape::Triangle => "Triangle".to_string(),
        }
    }
}

impl Enumerable<LfoShape> for LfoShape {
    fn enumerate() -> Vec<LfoShape> {
        vec![LfoShape::Triangle, LfoShape::Sine, LfoShape::Saw]
    }
}

impl From<LfoShape> for String {
    fn from(f: LfoShape) -> String {
        f.as_string()
    }
}

impl From<String> for LfoShape {
    fn from(s: String) -> LfoShape {
        match s.as_ref() {
            "Sine" => LfoShape::Sine,
            "Saw" => LfoShape::Saw,
            "Triangle" => LfoShape::Triangle,
            _ => LfoShape::Sine,
        }
    }
}

// Discrete, synced LFO rate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LfoRateSync {
    R1_64,
    R1_32,
    R1_16,
    R1_8,
    R1_4,
    R1_2,
    R1,
    R2_1,
    R4_1,
    R8_1,
    R16_1,
}

impl LfoRateSync {
    pub fn value(self) -> u8 {
        match self {
            LfoRateSync::R1_64 => 0,
            LfoRateSync::R1_32 => 1,
            LfoRateSync::R1_16 => 2,
            LfoRateSync::R1_8 => 3,
            LfoRateSync::R1_4 => 4,
            LfoRateSync::R1_2 => 5,
            LfoRateSync::R1 => 6,
            LfoRateSync::R2_1 => 7,
            LfoRateSync::R4_1 => 8,
            LfoRateSync::R8_1 => 9,
            LfoRateSync::R16_1 => 10,
        }
    }

    pub fn as_string(self) -> String {
        match self {
            LfoRateSync::R1_64 => "1/64".to_string(),
            LfoRateSync::R1_32 => "1/32".to_string(),
            LfoRateSync::R1_16 => "1/16".to_string(),
            LfoRateSync::R1_8 => "1/8".to_string(),
            LfoRateSync::R1_4 => "1/4".to_string(),
            LfoRateSync::R1_2 => "1/2".to_string(),
            LfoRateSync::R1 => "1".to_string(),
            LfoRateSync::R2_1 => "2/1".to_string(),
            LfoRateSync::R4_1 => "4/1".to_string(),
            LfoRateSync::R8_1 => "8/1".to_string(),
            LfoRateSync::R16_1 => "16/1".to_string(),
        }
    }
}

impl From<LfoRateSync> for String {
    fn from(f: LfoRateSync) -> String {
        f.as_string()
    }
}

impl From<String> for LfoRateSync {
    fn from(s: String) -> LfoRateSync {
        match s.as_ref() {
            "1/64" => LfoRateSync::R1_64,
            "1/32" => LfoRateSync::R1_32,
            "1/16" => LfoRateSync::R1_16,
            "1/8" => LfoRateSync::R1_8,
            "1/4" => LfoRateSync::R1_4,
            "1/2" => LfoRateSync::R1_2,
            "1" => LfoRateSync::R1,
            "2/1" => LfoRateSync::R2_1,
            "4/1" => LfoRateSync::R4_1,
            "8/1" => LfoRateSync::R8_1,
            "16/1" => LfoRateSync::R16_1,
            _ => LfoRateSync::R1_4,
        }
    }
}
impl Enumerable<LfoRateSync> for LfoRateSync {
    fn enumerate() -> Vec<LfoRateSync> {
        vec![
            LfoRateSync::R1_64,
            LfoRateSync::R1_32,
            LfoRateSync::R1_16,
            LfoRateSync::R1_8,
            LfoRateSync::R1_4,
            LfoRateSync::R1_2,
            LfoRateSync::R1,
            LfoRateSync::R2_1,
            LfoRateSync::R4_1,
            LfoRateSync::R8_1,
            LfoRateSync::R16_1,
        ]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Rate {
    Hz(f64),
    Synced(LfoRateSync),
}

pub struct Lfo {
    rate: Rate,
    period_sec: f64,
    rate_hz: f64,
    shape: LfoShape,
    time_elapsed: f64,
    // TODO: Phase shift.
}

impl Lfo {
    pub fn new(shape: LfoShape, rate: Rate, tempo_bps: f64) -> Self {
        let (period_sec, rate_hz) = Self::compute_period_sec(&rate, tempo_bps);

        Lfo {
            rate,
            period_sec,
            rate_hz,
            shape,
            time_elapsed: 0.0,
        }
    }

    pub fn evaluate(&mut self, time_delta: f64) -> f64 {
        self.time_elapsed = (self.time_elapsed + time_delta) % self.period_sec;
        match self.shape {
            LfoShape::Sine => (TAU * self.rate_hz * self.time_elapsed).sin(),
            LfoShape::Saw => -2.0 * self.rate_hz * self.time_elapsed + 1.0,
            LfoShape::Triangle => {
                let p = 4.0 * self.rate_hz * self.time_elapsed;
                if self.time_elapsed < self.period_sec / 4.0 {
                    p
                } else if self.time_elapsed < 3.0 * self.period_sec / 4.0 {
                    2.0 - p
                } else {
                    -4.0 + p
                }
            }
        }
    }

    pub fn update_rate(&mut self, rate: Rate, tempo_bps: f64) {
        self.rate = rate;
        let (period_sec, rate_hz) = Self::compute_period_sec(&rate, tempo_bps);
        self.period_sec = period_sec;
        self.rate_hz = rate_hz;
    }

    pub fn compute_period_sec(rate: &Rate, tempo_bps: f64) -> (f64, f64) {
        match rate {
            Rate::Hz(rate_hz) => (1.0 / rate_hz, *rate_hz),
            Rate::Synced(rate) => {
                let factor = match rate {
                    LfoRateSync::R1_64 => 16.0,
                    LfoRateSync::R1_32 => 8.0,
                    LfoRateSync::R1_16 => 4.0,
                    LfoRateSync::R1_8 => 2.0,
                    LfoRateSync::R1_4 => 1.0,
                    LfoRateSync::R1_2 => 1.0 / 2.0,
                    LfoRateSync::R1 => 1.0 / 4.0,
                    LfoRateSync::R2_1 => 1.0 / 8.0,
                    LfoRateSync::R4_1 => 1.0 / 16.0,
                    LfoRateSync::R8_1 => 1.0 / 32.0,
                    LfoRateSync::R16_1 => 1.0 / 64.0,
                };
                // We get Hz by taking the beats per second, which put another way
                // is:
                //    (1 quarter note * factor / 1 sec)
                // To get period, it's 1 sec / cycle
                let rate_hz = factor * tempo_bps;
                (1.0 / rate_hz, rate_hz)
            }
        }
    }
}
