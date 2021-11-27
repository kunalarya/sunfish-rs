use std::collections::VecDeque;

use num_traits::Float;

use crate::dsp::env;
use crate::dsp::filter::Filter;
use crate::dsp::interpolator::{CachedWaveform, Interpolator};
use crate::dsp::osc::{Unison, WaveShape};
use crate::modulation;
use crate::modulation::target::ModulationTarget;
use crate::modulation::{ModState, Modulation};
use crate::params::sync::{MailboxReceiver, Synchronizer};
use crate::params::NormalizedParams;
use crate::params::Params;
use crate::params::ParamsMeta;
use crate::params::{EFiltParams, EOscParams, EParam};
use crate::util::note_freq::NOTE_TO_FREQ;

pub const CHANNEL_COUNT: usize = 2;
pub const VOICES_MAX: usize = 128;

#[derive(Debug)]
pub struct Voice {
    base_note: u8,
    freq_osc1: f64,
    freq_osc2: f64,

    #[allow(dead_code)]
    pitch_bend: f64, // -1.0 - 1.0
    #[allow(dead_code)]
    pitch_bend_range: f64, // +/- this value.

    osc1_fine_offset: f64,
    osc1_semitones_offset: i32,
    osc1_octave_offset: i32,

    osc2_fine_offset: f64,
    osc2_semitones_offset: i32,
    osc2_octave_offset: i32,

    #[allow(dead_code)]
    velocity: i8,
    // Each filter state is per channel (left, right)
    filter1: Vec<Filter>,
    filter2: Vec<Filter>,
    // Amplitude envelope:
    amp_envelope: env::Env,
    // Filter envelope:
    mod_envelope: env::Env,
    mod_state: ModState,

    cached_waveforms_osc1: Vec<CachedWaveform>,
    cached_waveforms_osc2: Vec<CachedWaveform>,

    note_released: bool,
}

struct VoiceInfo<'a> {
    sample_rate: f64,
    note: u8,
    velocity: i8,
    osc1_fine_offset: f64,
    osc1_semitones_offset: i32,
    osc1_octave_offset: i32,
    osc2_fine_offset: f64,
    osc2_semitones_offset: i32,
    osc2_octave_offset: i32,
    amp_adsr: env::ADSR,
    mod_adsr: env::ADSR,
    params: &'a Params,
    meta: &'a ParamsMeta,
}

impl Voice {
    #[allow(clippy::too_many_arguments)]
    fn new(
        info: &VoiceInfo, // sample_rate: f64,
                          // note: u8,
                          // velocity: i8,
                          // osc1_fine_offset: f64,
                          // osc1_semitones_offset: i32,
                          // osc1_octave_offset: i32,
                          // osc2_fine_offset: f64,
                          // osc2_semitones_offset: i32,
                          // osc2_octave_offset: i32,
                          // amp_adsr: env::ADSR,
                          // mod_adsr: env::ADSR,
                          // params: &Params,
                          // meta: &ParamsMeta
    ) -> Voice {
        let mut filter1: Vec<Filter> = Vec::with_capacity(CHANNEL_COUNT);
        let mut filter2: Vec<Filter> = Vec::with_capacity(CHANNEL_COUNT);
        for _channel_idx in 0..CHANNEL_COUNT {
            filter1.push(Filter::new(
                info.sample_rate,
                &info.params.filt1.mode,
                &info.params.filt1.cutoff_semi,
                &info.params.filt1.resonance,
            ));
            filter2.push(Filter::new(
                info.sample_rate,
                &info.params.filt2.mode,
                &info.params.filt2.cutoff_semi,
                &info.params.filt2.resonance,
            ));
        }
        let mut amp_envelope = env::Env::new(info.amp_adsr, info.sample_rate);
        amp_envelope.start();
        let mut mod_envelope = env::Env::new(info.mod_adsr, info.sample_rate);
        mod_envelope.start();

        // TODO: If note isn't valid, set velocity to 0.
        let cached_waveforms_osc1 = vec![CachedWaveform::zero(); CHANNEL_COUNT];
        let cached_waveforms_osc2 = vec![CachedWaveform::zero(); CHANNEL_COUNT];

        let mut mod_state = ModState::new(info.sample_rate, 1);
        modulation::update_mod_range(
            &mut mod_state,
            info.meta,
            0,
            ModulationTarget::Filter1Cutoff,
        );

        let mut inst = Voice {
            base_note: info.note,
            freq_osc1: 0.0,
            freq_osc2: 0.0,

            // TODO: Support pitch bending.
            pitch_bend: 0.0,
            pitch_bend_range: 1.0,

            osc1_fine_offset: info.osc1_fine_offset,
            osc1_semitones_offset: info.osc1_semitones_offset,
            osc1_octave_offset: info.osc1_octave_offset,

            osc2_fine_offset: info.osc2_fine_offset,
            osc2_semitones_offset: info.osc2_semitones_offset,
            osc2_octave_offset: info.osc2_octave_offset,

            velocity: info.velocity,
            filter1,
            filter2,
            amp_envelope,
            mod_envelope,
            mod_state,

            cached_waveforms_osc1,
            cached_waveforms_osc2,

            note_released: false,
        };
        inst.update_osc1_freq();
        inst.update_osc2_freq();
        inst
    }

    pub fn update_osc1_freq(&mut self) {
        for cw in self.cached_waveforms_osc1.iter_mut() {
            cw.reset();
        }
        self.freq_osc1 = self.calculate_freq(
            self.osc1_fine_offset,
            self.osc1_octave_offset,
            self.osc1_semitones_offset,
        );
    }

    pub fn update_osc2_freq(&mut self) {
        for cw in self.cached_waveforms_osc2.iter_mut() {
            cw.reset();
        }
        self.freq_osc2 = self.calculate_freq(
            self.osc2_fine_offset,
            self.osc2_octave_offset,
            self.osc2_semitones_offset,
        );
    }

    fn calculate_freq(
        &mut self,
        fine_offset: f64,
        octave_offset: i32,
        semitones_offset: i32,
    ) -> f64 {
        let note = self.base_note as i32;
        // Add octaves.
        let note = note + (octave_offset * 12);
        // Add semitones.
        let note = note + semitones_offset;

        let freq = *NOTE_TO_FREQ.get(&note).unwrap_or(&0.0);
        // TODO: Pitch bending.
        freq + fine_offset
    }

    fn release(&mut self) {
        if self.note_released {
            return;
        }
        self.note_released = true;
        self.amp_envelope.release();
    }

    fn idle(&self) -> bool {
        // TODO: Do we need to factor in note_released?
        self.amp_envelope.is_idle()
    }
}

pub struct Tempo {
    // The following is a silly hack to minimize the number of
    // time we have to downcast or upcast floats.
    pub tempo_bpm_f32: f32,
    pub tempo_bpm_f64: f64,
    pub tempo_bps: f64,
}

impl Tempo {
    pub fn new(tempo_bpm: f64) -> Self {
        Self {
            tempo_bpm_f32: tempo_bpm as f32,
            tempo_bpm_f64: tempo_bpm as f64,
            tempo_bps: tempo_bpm / 60.0,
        }
    }

    #[inline(always)]
    pub fn update(&mut self, tempo_bpm_f64: f64) {
        #[allow(clippy::float_cmp)]
        if self.tempo_bpm_f64 != tempo_bpm_f64 {
            self.tempo_bpm_f64 = tempo_bpm_f64;
            self.tempo_bpm_f32 = tempo_bpm_f64 as f32;
            self.tempo_bps = (tempo_bpm_f64 / 60.0) as f64;
        }
    }
}

pub type Voices = VecDeque<Voice>;

pub struct Sunfish {
    pub voices: Voices,
    /// Number of active voices producing sound. Note that this number is likely to be greater than
    /// the length of the Voices VecDeque, as we allow notes to complete their release envelopes.
    pub active_voices: usize,
    pub max_active_voices: usize,

    pub dt: f64,
    pub interpolator: Interpolator,

    pub tempo: Tempo,
    // Parameters and modulation.

    // The core logic will have its own copy of parameters
    pub meta: ParamsMeta,
    pub params: Params,
    pub params_modulated: Params,
    pub modulation: Modulation,
    pub params_sync: Synchronizer,
    param_reader: MailboxReceiver,
    last_epoch_recorded: u32,

    // Common buffer when processing audio.
    buf: Vec<f64>,
    // Preallocated amp & filter envelope.
    amp_filt_env_buf: Vec<(f64, f64)>,
}

impl Sunfish {
    pub fn new(
        meta: ParamsMeta,
        sample_rate: f64,
        param_reader: MailboxReceiver,
        params_sync: Synchronizer,
        modulation: Modulation,
        tempo: Tempo,
    ) -> Sunfish {
        let dt = 1.0 / sample_rate;

        // Create a core loop copy of the parameters. Failed clone indicates an error acquiring a
        // mutex; and we'll likely fail later; we proceed with blank parameters.
        let params = params_sync
            .clone_inner()
            .unwrap_or_else(|| Params::new(sample_rate));
        let params_modulated = params.clone();

        Sunfish {
            voices: VecDeque::with_capacity(VOICES_MAX),
            active_voices: 0,
            max_active_voices: 64,

            dt,
            interpolator: Interpolator::new(sample_rate),

            tempo,

            meta,
            params,
            params_modulated,
            params_sync,
            param_reader,
            last_epoch_recorded: 0,

            // Modulation
            modulation,
            buf: Vec::with_capacity(1024),
            amp_filt_env_buf: Vec::with_capacity(1024),
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        self.voices.clear();

        // TODO update GUI sample rate

        // Regenerate all waves.
        self.interpolator = Interpolator::new(sample_rate);
        self.buf.clear();
    }

    pub fn note_on(&mut self, note: u8, velocity: i8) {
        if self.active_voices > self.max_active_voices {
            return;
        }

        // If there's an active, unreleased note, release it now.
        for voice in self.voices.iter_mut().filter(|v| !v.note_released) {
            if voice.base_note == note {
                voice.release();
            }
        }

        let voice = Voice::new(&VoiceInfo {
            sample_rate: self.params.sample_rate,
            note,
            velocity,
            osc1_fine_offset: self.params_modulated.osc1.fine_offset,
            osc1_semitones_offset: self.params_modulated.osc1.semitones_offset,
            osc1_octave_offset: self.params_modulated.osc1.octave_offset,
            osc2_fine_offset: self.params_modulated.osc2.fine_offset,
            osc2_semitones_offset: self.params_modulated.osc2.semitones_offset,
            osc2_octave_offset: self.params_modulated.osc2.octave_offset,
            amp_adsr: self.params_modulated.amp_env,
            mod_adsr: self.params_modulated.mod_env,
            params: &self.params_modulated,
            meta: &self.meta,
        });

        self.voices.push_back(voice);
        self.active_voices += 1;
    }

    pub fn note_off(&mut self, note: u8) {
        for voice in self.voices.iter_mut().filter(|v| !v.note_released) {
            if voice.base_note == note {
                voice.release();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_param_update(
        meta: &ParamsMeta,
        params: &mut Params,
        params_modulated: &mut Params,
        tempo: &Tempo,
        voices: &mut Voices,
        modulation: &mut Modulation,
        param: EParam,
        param_value: f64,
    ) {
        // TODO: Hacky: we should do something more intelligent here.
        let previous_modulated_param = modulation.on_param_update_before_mod_update(
            meta,
            params,
            params_modulated,
            param,
            tempo.tempo_bps,
        );
        // Whatever the previously modulated parameter was, reset it to the user
        // value (to undo modulation).
        if let Some(previous_modulated_param) = previous_modulated_param {
            let user_value = params.read_parameter(meta, previous_modulated_param);
            params_modulated.write_parameter(meta, previous_modulated_param, user_value);
        }
        Self::update_voices(voices, params_modulated, param);
        // If this parameter isn't being modulated, reflect the change to
        // mod parameters. If it is being modulated, the modulation tick
        // will handle it.
        if !modulation.mod_state.modulated_params.contains(&param) {
            params_modulated.write_parameter(meta, param, param_value);
        }
    }

    //fn update_voices(&mut self, param: EParam) {
    fn update_voices(voices: &mut Voices, params_modulated: &mut Params, param: EParam) {
        match param {
            // Oscillators
            // TODO: May need shape here.
            EParam::Osc1(EOscParams::SemitonesOffset)
            | EParam::Osc1(EOscParams::OctaveOffset)
            | EParam::Osc1(EOscParams::FineOffset) => {
                for voice in voices.iter_mut() {
                    voice.osc1_semitones_offset = params_modulated.osc1.semitones_offset;
                    voice.osc1_octave_offset = params_modulated.osc1.octave_offset;
                    voice.osc1_fine_offset = params_modulated.osc1.fine_offset;
                    voice.update_osc1_freq();
                }
            }
            EParam::Osc2(EOscParams::SemitonesOffset)
            | EParam::Osc2(EOscParams::OctaveOffset)
            | EParam::Osc2(EOscParams::FineOffset) => {
                for voice in voices.iter_mut() {
                    voice.osc2_semitones_offset = params_modulated.osc2.semitones_offset;
                    voice.osc2_octave_offset = params_modulated.osc2.octave_offset;
                    voice.osc2_fine_offset = params_modulated.osc2.fine_offset;
                    voice.update_osc2_freq();
                }
            }
            EParam::Filt1(EFiltParams::Mode) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter1.iter_mut() {
                        filter.set_mode(&params_modulated.filt1.mode);
                    }
                }
            }
            EParam::Filt1(EFiltParams::Cutoff) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter1.iter_mut() {
                        filter.set_cutoff(params_modulated.filt1.cutoff_semi);
                    }
                }
            }
            EParam::Filt1(EFiltParams::Resonance) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter1.iter_mut() {
                        filter.set_resonance(params_modulated.filt1.resonance);
                    }
                }
            }
            EParam::Filt2(EFiltParams::Mode) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter2.iter_mut() {
                        filter.set_mode(&params_modulated.filt2.mode);
                    }
                }
            }
            EParam::Filt2(EFiltParams::Cutoff) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter2.iter_mut() {
                        filter.set_cutoff(params_modulated.filt2.cutoff_semi);
                    }
                }
            }
            EParam::Filt2(EFiltParams::Resonance) => {
                for voice in voices.iter_mut() {
                    for filter in voice.filter2.iter_mut() {
                        filter.set_resonance(params_modulated.filt2.resonance);
                    }
                }
            }
            EParam::AmpEnv(_amp_env_param) => {
                for voice in voices.iter_mut() {
                    voice.amp_envelope.update_adsr(&params_modulated.amp_env);
                }
            }
            EParam::ModEnv(_mod_env_param) => {
                for voice in voices.iter_mut() {
                    voice.mod_envelope.update_adsr(&params_modulated.mod_env);
                }
            }
            _ => {}
        }
    }

    pub fn render<F: Float>(&mut self, outputs: &mut [&mut [F]]) {
        // TODO: Throttle this update to something more reasonable (~10khz?)
        self.param_reader
            .check_and_update(&mut self.last_epoch_recorded, |params, changes| {
                self.params = params;

                // if the existing epoch is newer than the last one we saw, apply changes to the
                // mirror to ensure nothing is lost.
                for (eparam, value) in changes {
                    Self::on_param_update(
                        &self.meta,
                        &mut self.params,
                        &mut self.params_modulated,
                        &self.tempo,
                        &mut self.voices,
                        &mut self.modulation,
                        *eparam,
                        *value,
                    );
                }
            });

        let buf_len = outputs[0].len();
        let buf_len_float = buf_len as f64;
        self.buf.resize(buf_len, 0.0);

        let delta_time = buf_len_float * self.dt;
        let (update_eparam_lfo1, update_eparam_lfo2) =
            self.modulation
                .tick(delta_time, &self.params, &mut self.params_modulated);
        if let Some(eparam_lfo1) = update_eparam_lfo1 {
            Self::update_voices(&mut self.voices, &mut self.params_modulated, eparam_lfo1);
        }
        if let Some(eparam_lfo2) = update_eparam_lfo2 {
            Self::update_voices(&mut self.voices, &mut self.params_modulated, eparam_lfo2);
        }

        let osc1_enabled = self.params_modulated.osc1.enabled;
        let osc2_enabled = self.params_modulated.osc2.enabled;

        let filter1_enabled = self.params_modulated.filt1.enable;
        let filter2_enabled = self.params_modulated.filt2.enable;

        for voice in self.voices.iter_mut() {
            let freq_osc1 = voice.freq_osc1;
            let freq_osc2 = voice.freq_osc2;
            if freq_osc1 == 0.0 || freq_osc2 == 0.0 {
                continue;
            }
            // First get the envelope, independent of channel.
            self.amp_filt_env_buf.clear();
            let output_len = outputs[0].len();
            if output_len > self.amp_filt_env_buf.len() {
                self.amp_filt_env_buf.resize(output_len, (0.0, 0.0));
            }
            for env_i in 0..output_len {
                voice.amp_envelope.next();
                voice.mod_envelope.next();
                self.amp_filt_env_buf[env_i] = (
                    voice.amp_envelope.get_level(),
                    voice.mod_envelope.get_level(),
                );
            }

            // Check if we should drop the note.
            if voice.idle() {
                continue;
            }

            let mut channel_idx_float = 0.0;
            for (channel_idx, output_channel) in outputs.iter_mut().enumerate() {
                let stereo_width = channel_idx_float * self.params_modulated.osc1.stereo_width;
                if osc1_enabled {
                    // Oscillator 1
                    let filt = if filter1_enabled {
                        Some(&mut voice.filter1[channel_idx])
                    } else {
                        None
                    };
                    Self::render_chain(
                        &mut self.buf,
                        self.dt,
                        &mut self.interpolator,
                        &mut voice.cached_waveforms_osc1[channel_idx],
                        filt,
                        freq_osc1,
                        &self.amp_filt_env_buf,
                        &mut voice.mod_state,
                        self.params_modulated.filt1.cutoff_semi,
                        self.params_modulated.filt1.env_amt,
                        output_channel,
                        stereo_width,
                        &self.params_modulated.osc1.shape,
                        &self.params_modulated.osc1.unison,
                        self.params_modulated.osc1.unison_amt,
                        self.params_modulated.osc1.gain,
                    );
                }

                if osc2_enabled {
                    // Oscillator 2
                    let filt = if filter2_enabled {
                        Some(&mut voice.filter2[channel_idx])
                    } else {
                        None
                    };
                    Self::render_chain(
                        &mut self.buf,
                        self.dt,
                        &mut self.interpolator,
                        &mut voice.cached_waveforms_osc2[channel_idx],
                        filt,
                        freq_osc2,
                        &self.amp_filt_env_buf,
                        &mut voice.mod_state,
                        self.params_modulated.filt2.cutoff_semi,
                        self.params_modulated.filt2.env_amt,
                        output_channel,
                        stereo_width,
                        &self.params_modulated.osc2.shape,
                        &self.params_modulated.osc2.unison,
                        self.params_modulated.osc2.unison_amt,
                        self.params_modulated.osc2.gain,
                    );
                }
                channel_idx_float += 1.0;
            }
        }

        // // Drop all voices that have done playing.
        while let Some(voice) = self.voices.front() {
            if voice.idle() {
                self.active_voices -= 1;
                self.voices.pop_front();
            } else {
                break;
            }
        }

        for (_channel_idx, output_channel) in outputs.iter_mut().enumerate() {
            for output_sample in output_channel.iter_mut() {
                // Apply global gain.
                *output_sample =
                    *output_sample * num::cast(self.params_modulated.output_gain).unwrap();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    fn render_chain<F: Float>(
        buf: &mut [f64],
        dt: f64, // Delta time per element of buf
        interpolator: &mut Interpolator,
        cached_waveform: &mut CachedWaveform,
        mut voice_filter: Option<&mut Filter>,
        f: f64,
        amp_and_mod_env_levels: &[(f64, f64)],
        voice_mod: &mut ModState,
        cutoff_semi: f64,
        filt_env_amount: f64,
        output_channel: &mut [F],
        stereo_width: f64,
        shape: &WaveShape,
        unison: &Unison,
        unison_amt: f64,
        osc_gain: f64,
    ) {
        // output_channel has type &mut [f64]
        interpolator.populate(
            *shape,               // shape
            f + stereo_width,     // freq
            buf,                  // output_buf
            output_channel.len(), // output_count
            cached_waveform,      // cached_waveform
            *unison,              // unison
            unison_amt,           // unison_amt
        );

        // Iterate over each sample in this channel, zipping with both
        // the amplitude and mod envelopes.
        let mut i = 0.0;
        for (value, amp_and_filt_env) in buf.iter_mut().zip(amp_and_mod_env_levels) {
            let (amp_env, mod_env) = amp_and_filt_env;

            let filtered = if let Some(ref mut filter) = voice_filter {
                // Avoid cast in tight loop: let delta_time = (index as f64) * dt;
                let delta_time = i * dt;
                // Step the voice mod.
                let did_modulate = voice_mod.tick(delta_time).is_some();
                if did_modulate {
                    // Apply the modulation. Filters only for now. Eventually,
                    // we can make these per-voice envelopes customizable.

                    // Since we've ticked, we need to compute the effective
                    // cutoff.
                    let mod_env = mod_env * filt_env_amount;
                    let modulated_cutoff = modulation::modulate(voice_mod, 0, cutoff_semi, mod_env);
                    filter.set_cutoff(modulated_cutoff);
                }
                filter.apply(*value)
            } else {
                *value
            };

            *value = filtered * amp_env * osc_gain;
            i += 1.0;
        }
        for (output_sample, value) in output_channel.iter_mut().zip(buf) {
            *output_sample = *output_sample + num::cast(*value).unwrap();
        }
    }
}
