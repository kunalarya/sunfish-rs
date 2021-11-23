pub mod target;

use std::collections::HashSet;

use crate::lfo;
use crate::modulation::target::ModulationTarget;
use crate::params::{EFiltParams, ELfoParams, EOscParams, EParam};
use crate::params::{Params, ParamsMeta};

const MOD_TICK_HZ: f64 = 200.0; // 5 ms.
const MOD_TICK_S: f64 = 1.0 / MOD_TICK_HZ;

// TODO: Consolidate with other constants.
const MIN_OSC_FREQ: f64 = 20.0;
const MAX_OSC_FREQ: f64 = 20000.0;

// Global modulators.
#[derive(Clone, Debug)]
pub struct ModState {
    // Keep track of which parameters are being actively modulated.
    pub modulated_params: HashSet<EParam>,

    // Stores how much time has passed between modulation evaluation.
    mod_time_elapsed: f64,
    // How often to evaluate modulation.
    mod_tick: f64,

    mod_ranges: Vec<ModRange>,
}

impl ModState {
    pub fn new(sample_rate: f64, ranges: usize) -> Self {
        ModState {
            modulated_params: HashSet::new(),
            mod_time_elapsed: 0.0,
            mod_tick: MOD_TICK_S * (1.0 / sample_rate),
            mod_ranges: vec![ModRange::new(); ranges],
        }
    }
    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        self.mod_tick = MOD_TICK_S * (1.0 / sample_rate);
    }

    /// Tick the modulator; if enough time has passed to trigger an actual modulation tick, return
    /// the time elapsed since the last tick.
    pub fn tick(&mut self, delta: f64) -> Option<f64> {
        self.mod_time_elapsed += delta;
        if self.mod_time_elapsed > self.mod_tick {
            let time_elapsed = self.mod_time_elapsed;
            self.mod_time_elapsed = 0.0;
            Some(time_elapsed)
        } else {
            None
        }
    }
}

// Per-modulator state.
#[derive(Clone, Debug)]
pub struct ModRange {
    min: f64,
    max: f64,
    range: f64,
}

impl ModRange {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        ModRange {
            min: 0.0,
            max: 1.0,
            range: 1.0,
        }
    }
    fn update_range(&mut self) {
        self.range = self.max - self.min;
    }
}

pub struct Modulation {
    // LFOs
    lfo1: lfo::Lfo,
    lfo2: lfo::Lfo,
    pub mod_state: ModState,
}

impl Modulation {
    pub fn new(sample_rate: f64) -> Self {
        // Temporary value; the next process cycle will set the tempo. We could use an Option
        // around the LFOs, but then we pay for a conditional branch on every process call.
        let tempo_bps = 10.0;

        Self {
            lfo1: lfo::Lfo::new(lfo::LfoShape::Triangle, lfo::Rate::Hz(1.0), tempo_bps),
            lfo2: lfo::Lfo::new(lfo::LfoShape::Triangle, lfo::Rate::Hz(1.0), tempo_bps),
            mod_state: ModState::new(sample_rate, 2),
        }
    }

    pub fn tick(
        &mut self,
        delta: f64,
        params: &Params,
        params_modulated: &mut Params,
    ) -> (Option<EParam>, Option<EParam>) {
        if let Some(time_elapsed) = self.mod_state.tick(delta) {
            // Which parameters to update voices on, if any.
            self.tick_lfos(time_elapsed, params, params_modulated)
        } else {
            (None, None)
        }
    }

    /// Deal with modulation target and rate changes. This must happen before the modulated state
    /// is updated.
    pub fn on_param_update_before_mod_update(
        &mut self,
        meta: &ParamsMeta,
        params: &Params,
        params_modulated: &Params,
        param: EParam,
        tempo_bps: f64,
    ) -> Option<EParam> {
        // TODO: Hacky: we should do something more intelligent here.
        // TODO: If the target changed, copy all user parameters to modulated parameters.
        // Alternatively determine which parameter changed.
        match param {
            // Modulators
            EParam::Lfo1(ELfoParams::Target) => {
                let previous_target = params_modulated.lfo1.target;
                let target = params.lfo1.target;
                update_mod_range(&mut self.mod_state, meta, 0, target);
                modulation_target_to_eparam(&previous_target)
            }
            EParam::Lfo1(ELfoParams::Rate) => {
                self.lfo1.update_rate(params.lfo1.rate, tempo_bps);
                None
            }
            EParam::Lfo2(ELfoParams::Target) => {
                let previous_target = params_modulated.lfo2.target;
                let target = params.lfo2.target;
                update_mod_range(&mut self.mod_state, meta, 1, target);
                modulation_target_to_eparam(&previous_target)
            }
            EParam::Lfo2(ELfoParams::Rate) => {
                self.lfo2.update_rate(params.lfo2.rate, tempo_bps);
                None
            }
            _ => None,
        }
    }

    #[inline]
    /// "Tick" the modulation by updating all modulated parameters.
    ///
    /// Returns:
    /// -------
    /// The return type is unusual; for LFO1 and LFO2, it will return an optional EParam.
    /// If specified, the parameter affects all active voices (notes being played).
    pub fn tick_lfos(
        &mut self,
        time_delta: f64,
        params: &Params,
        params_modulated: &mut Params,
    ) -> (Option<EParam>, Option<EParam>) {
        let mod_value = self.lfo1.evaluate(time_delta) * params.lfo1.amt;
        let target = params.lfo1.target;
        let update1 = apply_modulation_to(
            &self.mod_state,
            params,
            params_modulated,
            target,
            mod_value,
            0,
        );

        let mod_value = self.lfo2.evaluate(time_delta) * params.lfo2.amt;
        let target = params.lfo2.target;
        let update2 = apply_modulation_to(
            &self.mod_state,
            params,
            params_modulated,
            target,
            mod_value,
            1,
        );
        (update1, update2)
    }
}

#[inline(always)]
pub fn apply_modulation_to(
    mod_state: &ModState,
    params: &Params,
    params_modulated: &mut Params,
    target: ModulationTarget,
    mod_value: f64,
    mod_index: usize,
) -> Option<EParam> {
    if target == ModulationTarget::Off {
        return None;
    }
    match &target {
        ModulationTarget::Osc1Frequency => {
            params_modulated.osc1.fine_offset =
                modulate(mod_state, mod_index, params.osc1.fine_offset, mod_value);
            Some(EParam::Osc1(EOscParams::FineOffset))
        }
        ModulationTarget::Osc1StereoWidth => {
            params_modulated.osc1.stereo_width =
                modulate(mod_state, mod_index, params.osc1.stereo_width, mod_value);
            Some(EParam::Osc1(EOscParams::StereoWidth))
        }
        ModulationTarget::Osc1UnisonAmt => {
            params_modulated.osc1.unison_amt =
                modulate(mod_state, mod_index, params.osc1.unison_amt, mod_value);
            Some(EParam::Osc1(EOscParams::UnisonAmt))
        }
        ModulationTarget::Filter1Cutoff => {
            params_modulated.filt1.cutoff_semi =
                modulate(mod_state, mod_index, params.filt1.cutoff_semi, mod_value);
            Some(EParam::Filt1(EFiltParams::Cutoff))
        }
        ModulationTarget::Filter1Resonance => {
            params_modulated.filt1.resonance =
                modulate(mod_state, mod_index, params.filt1.resonance, mod_value);
            Some(EParam::Filt1(EFiltParams::Resonance))
        }
        ModulationTarget::Osc2Frequency => {
            params_modulated.osc2.fine_offset =
                modulate(mod_state, mod_index, params.osc2.fine_offset, mod_value);
            Some(EParam::Osc2(EOscParams::FineOffset))
        }
        ModulationTarget::Osc2StereoWidth => {
            params_modulated.osc2.stereo_width =
                modulate(mod_state, mod_index, params.osc2.stereo_width, mod_value);
            Some(EParam::Osc2(EOscParams::StereoWidth))
        }
        ModulationTarget::Osc2UnisonAmt => {
            params_modulated.osc2.unison_amt =
                modulate(mod_state, mod_index, params.osc2.unison_amt, mod_value);
            Some(EParam::Osc2(EOscParams::UnisonAmt))
        }
        ModulationTarget::Filter2Cutoff => {
            params_modulated.filt2.cutoff_semi =
                modulate(mod_state, mod_index, params.filt2.cutoff_semi, mod_value);
            Some(EParam::Filt2(EFiltParams::Cutoff))
        }
        ModulationTarget::Filter2Resonance => {
            params_modulated.filt2.resonance =
                modulate(mod_state, mod_index, params.filt2.resonance, mod_value);
            Some(EParam::Filt2(EFiltParams::Resonance))
        }
        _ => None,
    }
}

#[inline(always)]
pub fn modulate(
    mod_state: &ModState,
    mod_index: usize,
    baseline_value: f64,
    mod_value: f64,
) -> f64 {
    let mod_range = &mod_state.mod_ranges[mod_index];
    let mod_value = mod_value * mod_range.range;
    (baseline_value + mod_value)
        .min(mod_range.max)
        .max(mod_range.min)
}

pub fn update_mod_range(
    mod_state: &mut ModState,
    meta: &ParamsMeta,
    mod_index: usize,
    target: ModulationTarget,
) {
    let mut mod_range = &mut mod_state.mod_ranges[mod_index];
    match &target {
        ModulationTarget::Off => {}
        ModulationTarget::Osc1Frequency | ModulationTarget::Osc2Frequency => {
            mod_range.min = MIN_OSC_FREQ;
            mod_range.max = MAX_OSC_FREQ;
        }
        ModulationTarget::Osc1StereoWidth | ModulationTarget::Osc2StereoWidth => {
            mod_range.min = meta.osc_stereo_width_meta.0.min;
            mod_range.max = meta.osc_stereo_width_meta.0.max;
        }
        ModulationTarget::Osc1UnisonAmt | ModulationTarget::Osc2UnisonAmt => {
            mod_range.min = meta.osc_unison_amt_meta.0.min;
            mod_range.max = meta.osc_unison_amt_meta.0.max;
        }
        ModulationTarget::Filter1Cutoff | ModulationTarget::Filter2Cutoff => {
            mod_range.min = meta.cutoff_meta.0.min;
            mod_range.max = meta.cutoff_meta.0.max;
        }
        ModulationTarget::Filter1Resonance | ModulationTarget::Filter2Resonance => {
            mod_range.min = meta.resonance_meta.0.min;
            mod_range.max = meta.resonance_meta.0.max;
        }
    };
    mod_range.update_range();
}

fn modulation_target_to_eparam(target: &ModulationTarget) -> Option<EParam> {
    match &target {
        ModulationTarget::Osc1Frequency => Some(EParam::Osc1(EOscParams::FineOffset)),
        ModulationTarget::Osc1StereoWidth => Some(EParam::Osc1(EOscParams::StereoWidth)),
        ModulationTarget::Osc1UnisonAmt => Some(EParam::Osc1(EOscParams::UnisonAmt)),
        ModulationTarget::Filter1Cutoff => Some(EParam::Filt1(EFiltParams::Cutoff)),
        ModulationTarget::Filter1Resonance => Some(EParam::Filt1(EFiltParams::Resonance)),
        ModulationTarget::Osc2Frequency => Some(EParam::Osc2(EOscParams::FineOffset)),
        ModulationTarget::Osc2StereoWidth => Some(EParam::Osc2(EOscParams::StereoWidth)),
        ModulationTarget::Osc2UnisonAmt => Some(EParam::Osc2(EOscParams::UnisonAmt)),
        ModulationTarget::Filter2Cutoff => Some(EParam::Filt2(EFiltParams::Cutoff)),
        ModulationTarget::Filter2Resonance => Some(EParam::Filt2(EFiltParams::Resonance)),
        _ => None,
    }
}
