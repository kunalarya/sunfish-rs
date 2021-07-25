/// Envelope generator.
use serde::Deserialize;

use crate::util;

#[derive(Clone, Debug, Deserialize)]
pub struct ADSR {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
}

impl ADSR {
    #[cfg(test)]
    pub fn new(attack: f64, decay: f64, sustain: f64, release: f64) -> ADSR {
        ADSR {
            attack,
            decay,
            sustain,
            release,
        }
    }
}

impl Default for ADSR {
    fn default() -> Self {
        ADSR {
            attack: 0.01,
            decay: 0.02,
            sustain: 0.80,
            release: 0.01,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ADSRStage {
    Idle,
    Attack,
    Sustain,
    Decay,
    Release,
}

#[derive(Debug)]
pub struct Env {
    level: f64,
    stage: ADSRStage,
    target_level_opt: Option<f64>,
    coeff: f64,
    sample_rate: f64,
    adsr: ADSR,
}

/*
 * Envelopes are generated using a coefficient that approximates exponential decay. Since the
 * heuristics don't work well with nonzero start/ends, we compute a base level, then apply a scale
 * and offset.
 */
impl Env {
    pub fn new(adsr: ADSR, sample_rate: f64) -> Env {
        Env {
            level: 0.0,
            stage: ADSRStage::Idle,
            target_level_opt: None,
            coeff: 0.0,
            sample_rate,
            adsr,
        }
    }

    pub fn next(&mut self) {
        util::undenormalize(&mut self.level);
        if let Some(target_level) = self.target_level_opt {
            match self.stage {
                ADSRStage::Attack => {
                    if self.level >= target_level {
                        self.enter_stage(ADSRStage::Decay);
                        return;
                    }
                }
                ADSRStage::Decay => {
                    if self.level <= target_level {
                        self.enter_stage(ADSRStage::Sustain);
                        return;
                    }
                }
                ADSRStage::Release => {
                    if self.level <= target_level {
                        self.enter_stage(ADSRStage::Idle);
                        return;
                    }
                }
                _ => {}
            }
        }
        self.level *= self.coeff;
    }

    pub fn get_level(&self) -> f64 {
        self.level
    }

    pub fn start(&mut self) {
        // enter the attack stage
        self.level = 0.0;
        self.enter_stage(ADSRStage::Attack);
    }

    pub fn release(&mut self) {
        // Allow release to be called multiple times.
        if self.stage != ADSRStage::Release {
            self.enter_stage(ADSRStage::Release);
        }
    }

    fn enter_stage(&mut self, stage: ADSRStage) {
        match stage {
            ADSRStage::Idle => {
                self.coeff = 1.0;
                self.target_level_opt = None;
            }
            ADSRStage::Attack => {
                // Ramp up to 1.0
                self.calc_coeff(self.adsr.attack, 1.0);
            }
            ADSRStage::Sustain => {
                // Keep the current level;
                self.coeff = 1.0;
                self.target_level_opt = None;
            }
            ADSRStage::Decay => {
                self.calc_coeff(self.adsr.decay, self.adsr.sustain);
            }
            ADSRStage::Release => {
                self.calc_coeff(self.adsr.release, 0.0);
            }
        }
        self.stage = stage;
    }

    pub fn is_idle(&self) -> bool {
        self.stage == ADSRStage::Idle
    }

    fn calc_coeff(&mut self, mut time: f64, mut target_level: f64) {
        // make sure coeff is never inf or nan
        const ALMOST_ZERO: f64 = 1e-3;
        if target_level == 0.0 {
            target_level = ALMOST_ZERO;
        }
        if self.level == 0.0 {
            self.level = ALMOST_ZERO;
        }
        if time == 0.0 {
            time = ALMOST_ZERO;
        }
        self.coeff = 1.0 + (target_level.ln() - self.level.ln()) / (time * self.sample_rate);
        self.target_level_opt = Some(target_level);
    }

    pub fn update_adsr(&mut self, adsr: &ADSR) {
        self.adsr = adsr.clone();
        // Re-enter the stage; the level stays as is, so we should be okay.
        self.enter_stage(self.stage.clone());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;
    const DT: f64 = 1.0 / SAMPLE_RATE;

    #[test]
    fn initializes_attack() {
        let mut eg = Env::new(default_adsr(), SAMPLE_RATE);
        assert_eq!(eg.stage, ADSRStage::Idle);
        eg.start();
        assert_eq!(eg.stage, ADSRStage::Attack);
    }

    #[test]
    fn attack_to_decay() {
        let mut eg = Env::new(default_adsr(), SAMPLE_RATE);

        // Compensate for overshoot and filtering:
        const MARGIN: usize = 5;

        eg.start();

        // Let some time pass.
        // Cycle through the attack phase. It's set to 1 ms so we anticipate that after 1ms we
        // will be in the decay stage.
        // If we have dt = 1 / sample_rate, then the number of samples for 1 ms is 1 ms / dt.
        let samples = (1e-3 / DT) as usize;
        for _ in 0..samples + MARGIN {
            eg.next();
        }
        assert_eq!(eg.stage, ADSRStage::Decay);
    }

    fn default_adsr() -> ADSR {
        ADSR::new(0.001, 0.002, 0.8, 0.003)
    }
}
