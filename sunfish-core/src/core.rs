use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::collections::HashSet;

use crate::dsp::env;
use crate::dsp::filter::Filter;
use crate::dsp::interpolator::{CachedWaveform, Interpolator};
use crate::dsp::osc::{Unison, WaveShape};
use crate::modulation;
use crate::modulation::target::ModulationTarget;
use crate::modulation::{ModState, Modulation};
use crate::params::NormalizedParams;
use crate::params::SunfishParams;
use crate::params::SunfishParamsMeta;
use crate::params::{EFiltParams, EOscParams, EParam};
use crate::util::note_freq::NOTE_TO_FREQ;

pub const CHANNEL_COUNT: usize = 2;
pub const VOICES_MAX: usize = 128;

#[derive(Debug)]
pub struct Voice {
    base_note: u8,
    freq_osc1: f64,
    freq_osc2: f64,

    pitch_bend: f64,       // -1.0 - 1.0
    pitch_bend_range: f64, // +/- this value.

    osc1_fine_offset: f64,
    osc1_semitones_offset: i32,
    osc1_octave_offset: i32,

    osc2_fine_offset: f64,
    osc2_semitones_offset: i32,
    osc2_octave_offset: i32,

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
}

impl Voice {
    fn new(
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
        params: &SunfishParams,
        meta: &SunfishParamsMeta,
    ) -> Voice {
        let mut filter1: Vec<Filter> = Vec::with_capacity(CHANNEL_COUNT);
        let mut filter2: Vec<Filter> = Vec::with_capacity(CHANNEL_COUNT);
        for _channel_idx in 0..CHANNEL_COUNT {
            filter1.push(Filter::new(
                sample_rate,
                &params.filt1.mode,
                &params.filt1.cutoff_semi,
                &params.filt1.resonance,
            ));
            filter2.push(Filter::new(
                sample_rate,
                &params.filt2.mode,
                &params.filt2.cutoff_semi,
                &params.filt2.resonance,
            ));
        }
        let mut amp_envelope = env::Env::new(amp_adsr, sample_rate);
        amp_envelope.start();
        let mut mod_envelope = env::Env::new(mod_adsr, sample_rate);
        mod_envelope.start();

        // TODO: If note isn't valid, set velocity to 0.
        let cached_waveforms_osc1 = vec![CachedWaveform::zero(); CHANNEL_COUNT];
        let cached_waveforms_osc2 = vec![CachedWaveform::zero(); CHANNEL_COUNT];

        let mut mod_state = ModState::new(sample_rate, 1);
        modulation::update_mod_range(&mut mod_state, meta, 0, ModulationTarget::Filter1Cutoff);

        let mut inst = Voice {
            base_note: note,
            freq_osc1: 0.0,
            freq_osc2: 0.0,

            // TODO: Support pitch bending.
            pitch_bend: 0.0,
            pitch_bend_range: 1.0,

            osc1_fine_offset,
            osc1_semitones_offset,
            osc1_octave_offset,

            osc2_fine_offset,
            osc2_semitones_offset,
            osc2_octave_offset,

            velocity,
            filter1,
            filter2,
            amp_envelope,
            mod_envelope,
            mod_state,

            cached_waveforms_osc1,
            cached_waveforms_osc2,
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
        let freq = freq + fine_offset;
        freq
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
        if self.tempo_bpm_f64 != tempo_bpm_f64 {
            self.tempo_bpm_f64 = tempo_bpm_f64;
            self.tempo_bpm_f32 = tempo_bpm_f64 as f32;
            self.tempo_bps = (tempo_bpm_f64 / 60.0) as f64;
        }
    }
}

pub struct Sunfish {
    // Added a counter in our plugin struct.
    // TODO: Replace HashMap with a deque.
    pub voices: HashMap<u8, Vec<Voice>>,
    pub dt: f64,
    pub interpolator: Interpolator,

    // Parameters and modulation.
    pub modulation: Modulation,

    // Common buffer when processing audio.
    buf: Vec<f64>,
    // Preallocated amp & filter envelope.
    amp_filt_env_buf: Vec<(f64, f64)>,

    voice_count: usize,
    ignored_notes: HashSet<u8>,
    voices_to_drop_indices: Vec<usize>,
}

impl Sunfish {
    pub fn new(sample_rate: f64, modulation: Modulation) -> Sunfish {
        let dt = 1.0 / sample_rate;

        Sunfish {
            voices: HashMap::new(),
            ignored_notes: HashSet::with_capacity(VOICES_MAX),

            dt,
            interpolator: Interpolator::new(sample_rate),

            // Modulation
            modulation,
            buf: Vec::with_capacity(1024),
            amp_filt_env_buf: Vec::with_capacity(1024),

            voice_count: 0,
            voices_to_drop_indices: Vec::with_capacity(32),
        }
    }

    pub fn update_sample_rate(&mut self, sample_rate: f64) {
        self.voices.clear();
        self.ignored_notes.clear();

        self.modulation.update_sample_rate(sample_rate);
        // TODO update GUI sample rate

        // Regenerate all waves.
        self.interpolator = Interpolator::new(sample_rate);
        self.buf.clear();
    }

    pub fn note_on(&mut self, note: u8, velocity: i8) {
        if self.voice_count > VOICES_MAX {
            self.ignored_notes.insert(note);
            return;
        }

        let voice = Voice::new(
            self.modulation.params.baseline.sample_rate,
            note,
            velocity,
            self.modulation.params.modulated.osc1.fine_offset,
            self.modulation.params.modulated.osc1.semitones_offset,
            self.modulation.params.modulated.osc1.octave_offset,
            self.modulation.params.modulated.osc2.fine_offset,
            self.modulation.params.modulated.osc2.semitones_offset,
            self.modulation.params.modulated.osc2.octave_offset,
            self.modulation.params.modulated.amp_env.clone(),
            self.modulation.params.modulated.mod_env.clone(),
            &self.modulation.params.modulated,
            &self.modulation.params.meta,
        );
        match self.voices.entry(note) {
            Vacant(entry) => {
                let mut v = Vec::with_capacity(32);
                v.push(voice);
                entry.insert(v);
            }
            Occupied(mut entry) => {
                let v = entry.get_mut();
                v.push(voice);
            }
        };
    }

    pub fn note_off(&mut self, note: u8) {
        if self.ignored_notes.contains(&note) {
            self.ignored_notes.remove(&note);
            return;
        }
        if let Some(voices) = self.voices.get_mut(&note) {
            for voice in voices.iter_mut() {
                voice.amp_envelope.release();
            }
        }
    }

    pub fn notify_param_update(&mut self, param: EParam, param_value: f64, tempo_bps: f64) {
        // TODO: Hacky: we should do something more intelligent here.
        let previous_modulated_param = self
            .modulation
            .on_param_update_before_mod_update(param, tempo_bps);
        // Whatever the previously modulated parameter was, reset it to the user
        // value (to undo modulation).
        if let Some(previous_modulated_param) = previous_modulated_param {
            let user_value = self
                .modulation
                .params
                .baseline
                .get_param_normalized(&self.modulation.params.meta, previous_modulated_param);
            if let Ok(user_value) = user_value {
                let _ = self.modulation.params.modulated_writer.update_param(
                    &self.modulation.params.meta,
                    previous_modulated_param,
                    user_value,
                );
            }
        }
        self.update_voices(param);
        // If this parameter isn't being modulated, reflect the change to
        // mod parameters. If it is being modulated, the modulation tick
        // will handle it.
        if !self.modulation.mod_state.modulated_params.contains(&param) {
            let _ = self.modulation.params.modulated_writer.update_param(
                &self.modulation.params.meta,
                param,
                param_value,
            );
        }
    }

    fn update_voices(&mut self, param: EParam) {
        match param {
            // Oscillators
            EParam::Osc1(EOscParams::SemitonesOffset)
            | EParam::Osc1(EOscParams::OctaveOffset)
            | EParam::Osc1(EOscParams::FineOffset) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        voice.osc1_semitones_offset =
                            self.modulation.params.modulated.osc1.semitones_offset;
                        voice.osc1_octave_offset =
                            self.modulation.params.modulated.osc1.octave_offset;
                        voice.osc1_fine_offset = self.modulation.params.modulated.osc1.fine_offset;
                        voice.update_osc1_freq();
                    }
                }
            }
            EParam::Osc2(EOscParams::SemitonesOffset)
            | EParam::Osc2(EOscParams::OctaveOffset)
            | EParam::Osc2(EOscParams::FineOffset) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        voice.osc2_semitones_offset =
                            self.modulation.params.modulated.osc2.semitones_offset;
                        voice.osc2_octave_offset =
                            self.modulation.params.modulated.osc2.octave_offset;
                        voice.osc2_fine_offset = self.modulation.params.modulated.osc2.fine_offset;
                        voice.update_osc2_freq();
                    }
                }
            }
            EParam::Filt1(EFiltParams::Mode) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter1.iter_mut() {
                            filter.set_mode(&self.modulation.params.modulated.filt1.mode);
                        }
                    }
                }
            }
            EParam::Filt1(EFiltParams::Cutoff) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter1.iter_mut() {
                            filter.set_cutoff(self.modulation.params.modulated.filt1.cutoff_semi);
                        }
                    }
                }
            }
            EParam::Filt1(EFiltParams::Resonance) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter1.iter_mut() {
                            filter.set_resonance(self.modulation.params.modulated.filt1.resonance);
                        }
                    }
                }
            }
            EParam::Filt2(EFiltParams::Mode) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter2.iter_mut() {
                            filter.set_mode(&self.modulation.params.modulated.filt2.mode);
                        }
                    }
                }
            }
            EParam::Filt2(EFiltParams::Cutoff) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter2.iter_mut() {
                            filter.set_cutoff(self.modulation.params.modulated.filt2.cutoff_semi);
                        }
                    }
                }
            }
            EParam::Filt2(EFiltParams::Resonance) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        for filter in voice.filter2.iter_mut() {
                            filter.set_resonance(self.modulation.params.modulated.filt2.resonance);
                        }
                    }
                }
            }
            EParam::AmpEnv(_amp_env_param) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        voice
                            .amp_envelope
                            .update_adsr(&self.modulation.params.modulated.amp_env);
                    }
                }
            }
            EParam::ModEnv(_mod_env_param) => {
                for (_, voices) in self.voices.iter_mut() {
                    for (_, voice) in voices.iter_mut().enumerate() {
                        voice
                            .mod_envelope
                            .update_adsr(&self.modulation.params.modulated.mod_env);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn render(&mut self, outputs: &mut [&mut [f64]]) {
        let buf_len = outputs[0].len();
        let buf_len_float = buf_len as f64;
        self.buf.resize(buf_len, 0.0);

        let delta_time = buf_len_float * self.dt;
        let (update_eparam_lfo1, update_eparam_lfo2) = self.modulation.tick(delta_time);
        if let Some(eparam_lfo1) = update_eparam_lfo1 {
            self.update_voices(eparam_lfo1);
        }
        if let Some(eparam_lfo2) = update_eparam_lfo2 {
            self.update_voices(eparam_lfo2);
        }

        let osc1_enabled = self.modulation.params.modulated.osc1.enabled;
        let osc2_enabled = self.modulation.params.modulated.osc2.enabled;

        let filter1_enabled = self.modulation.params.modulated.filt1.enable;
        let filter2_enabled = self.modulation.params.modulated.filt2.enable;

        for (_, voices) in self.voices.iter_mut() {
            self.voices_to_drop_indices.clear();
            for (voice_index, voice) in voices.iter_mut().enumerate() {
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
                if voice.amp_envelope.is_idle() {
                    // Always insert at the beginning; we have to drop from the back
                    // forward.
                    self.voices_to_drop_indices.insert(0, voice_index);
                    continue;
                }

                let mut channel_idx_float = 0.0;
                for (channel_idx, mut output_channel) in outputs.iter_mut().enumerate() {
                    let stereo_width =
                        channel_idx_float * self.modulation.params.modulated.osc1.stereo_width;
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
                            self.modulation.params.modulated.filt1.cutoff_semi,
                            self.modulation.params.modulated.filt1.env_amt,
                            &mut output_channel,
                            stereo_width,
                            &self.modulation.params.modulated.osc1.shape,
                            &self.modulation.params.modulated.osc1.unison,
                            self.modulation.params.modulated.osc1.unison_amt,
                            self.modulation.params.modulated.osc1.gain,
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
                            self.modulation.params.modulated.filt2.cutoff_semi,
                            self.modulation.params.modulated.filt2.env_amt,
                            &mut output_channel,
                            stereo_width,
                            &self.modulation.params.modulated.osc2.shape,
                            &self.modulation.params.modulated.osc2.unison,
                            self.modulation.params.modulated.osc2.unison_amt,
                            self.modulation.params.modulated.osc2.gain,
                        );
                    }
                    channel_idx_float += 1.0;
                }
            }

            // Drop all voices that have done playing.
            for &voice_index in self.voices_to_drop_indices.iter() {
                voices.remove(voice_index);
            }
        }
        for (_channel_idx, output_channel) in outputs.iter_mut().enumerate() {
            for output_sample in output_channel.iter_mut() {
                // Apply global gain.
                *output_sample *= self.modulation.params.modulated.output_gain;
            }
        }
    }

    #[inline(always)]
    fn render_chain(
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
        output_channel: &mut [f64],
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
                    let modulated_cutoff =
                        modulation::modulate(&voice_mod, 0, cutoff_semi, mod_env);
                    filter.set_cutoff(modulated_cutoff);
                    // log::info!("Mod env set cutoff to {} semis", modulated_cutoff);
                }
                filter.apply(*value)
            } else {
                *value
            };

            *value = filtered * amp_env * osc_gain;
            i += 1.0;
        }
        for (output_sample, value) in output_channel.iter_mut().zip(buf) {
            *output_sample += *value;
        }
    }
}