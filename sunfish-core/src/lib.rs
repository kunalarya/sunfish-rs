#![feature(get_mut_unchecked)]
pub mod core;
pub mod dsp;
pub mod lfo;
pub mod modulation;
pub mod params;
pub mod plugin;
pub mod swarc;
pub mod util;

use vst::api::{Events, Supported};
use vst::buffer::AudioBuffer;
use vst::editor::Editor;
use vst::event::Event;
use vst::host::Host;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin};
use vst::plugin_main;

use crate::params::NormalizedParams;
use crate::util::errors;

// We're implementing a trait `Plugin` that does all the VST-y stuff for us.
impl Plugin for plugin::SunfishPlugin {
    fn new(host: HostCallback) -> plugin::SunfishPlugin {
        let mut inst = plugin::SunfishPlugin::default();
        inst.host = host;
        inst
    }

    fn init(&mut self) {
        log::info!("Started Sunfish VST",);
        errors::setup_panic_handling();
    }

    fn get_info(&self) -> Info {
        Info {
            name: "Sunfish".to_string(),

            // Used by hosts to differentiate between plugins.
            unique_id: 0x78_B5_2B_BC,

            // We don't need inputs
            inputs: 0,

            // We do need two outputs though.  This is default, but let's be
            // explicit anyways.
            outputs: core::CHANNEL_COUNT as i32,

            parameters: self.core.modulation.params.meta.count() as i32,

            // Set our category
            category: Category::Synth,

            // We don't care about other stuff, and it can stay default.
            ..Default::default()
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        self.core
            .modulation
            .params
            .baseline
            .get_name(&self.core.modulation.params.meta, index as usize)
            .unwrap_or_else(|_| "(error)".to_string())
    }

    fn get_parameter_text(&self, index: i32) -> String {
        self.core
            .modulation
            .params
            .baseline
            .formatted_value_by_index(&self.core.modulation.params.meta, index as usize)
            .unwrap_or_else(|_| "(error)".to_string())
    }

    fn get_parameter_label(&self, _index: i32) -> String {
        "".to_string()
    }

    fn get_parameter(&self, index: i32) -> f32 {
        self.core
            .modulation
            .params
            .baseline
            .get_param_by_index(&self.core.modulation.params.meta, index as usize)
            .unwrap_or(0.0) as f32
    }

    fn set_parameter(&mut self, index: i32, value: f32) {
        let value = value as f64;
        let notification = self
            .core
            .modulation
            .params
            .baseline_writer
            .update_param_by_index(&self.core.modulation.params.meta, index as usize, value);
        if let Ok(eparam) = notification {
            self.core
                .notify_param_update(eparam, value, self.tempo.tempo_bps);
        }
    }

    fn can_be_automated(&self, _index: i32) -> bool {
        true
    }

    fn set_sample_rate(&mut self, rate: f32) {
        let rate = rate as f64;
        self.core.update_sample_rate(rate);
        self.core.modulation.params.update_sample_rate(rate);
        self.core.dt = 1.0 / rate;
    }

    // Here's the function that allows us to receive events
    fn process_events(&mut self, events: &Events) {
        // Some events aren't MIDI events - so let's do a match
        // to make sure we only get MIDI, since that's all we care about.
        for event in events.events() {
            if let Event::Midi(ev) = event {
                // Check if it's a noteon or noteoff event.
                // This is difficult to explain without knowing how the MIDI standard works.
                // Basically, the first byte of data tells us if this signal is a note on event
                // or a note off event.  You can read more about that here:
                // https://www.midi.org/specifications/item/table-1-summary-of-midi-message
                match ev.data[0] {
                    // if note on, increment our counter
                    144 => {
                        let note = ev.data[1];
                        let velocity = unsafe { std::mem::transmute::<u8, i8>(ev.data[2]) };
                        self.core.note_on(note, velocity);
                    }

                    // if note off, decrement our counter
                    128 => {
                        let note = ev.data[1];
                        self.core.note_off(note);
                    }

                    _ => (),
                }
            }
        }
    }

    /// Return handle to plugin editor if supported.
    fn get_editor(&mut self) -> Option<&mut dyn Editor> {
        None
    }

    fn process(&mut self, _buffer: &mut AudioBuffer<f32>) {
        // TODO: Support 32-bit?
    }

    fn process_f64(&mut self, buffer: &mut AudioBuffer<f64>) {
        // `buffer.split()` gives us a tuple containing the
        // input and output buffers.
        let (_, mut output_buffer) = buffer.split();

        // This is a hack to work around an initialization bug where
        // the host callback isn't set, but process is called (Bitwig does this).
        if self.host.raw_callback().is_some() {
            //let flags = vst::api::flags::TEMPO_VALID;
            let flags = vst::api::TimeInfoFlags::TEMPO_VALID;
            let time_info_opt = self.host.get_time_info(flags.bits());

            if let Some(time_info) = time_info_opt {
                let tempo_bpm_f64 = time_info.tempo;
                self.tempo.update(tempo_bpm_f64);
            }
        }

        // We need to zero out the buffers, since render assumes they
        // are zero. There may be a faster way to do this in the future.
        for output_channel in output_buffer.into_iter() {
            for output_sample in output_channel {
                *output_sample = 0.0;
            }
        }

        // Create a fixed slice of mutable slices (to avoid any heap allocations).
        let mut v: [&mut [f64]; core::CHANNEL_COUNT] = Default::default();
        let ch_count = output_buffer.len().max(core::CHANNEL_COUNT);

        for ch in 0..ch_count {
            v[ch] = output_buffer.get_mut(ch);
        }

        self.core.render(&mut v[..ch_count]);
    }

    // It's good to tell our host what our plugin can do.
    // Some VST hosts might not send any midi events to our plugin
    // if we don't explicitly tell them that the plugin can handle them.
    fn can_do(&self, can_do: CanDo) -> Supported {
        match can_do {
            // Tell our host that the plugin supports receiving MIDI messages
            CanDo::ReceiveMidiEvent => Supported::Yes,
            // Can receive time information (host tempo, etc).
            CanDo::ReceiveTimeInfo => Supported::Yes,
            // Maybe it also supports ather things
            _ => Supported::Maybe,
        }
    }
}

plugin_main!(plugin::SunfishPlugin);
