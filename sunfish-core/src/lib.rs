#![feature(get_mut_unchecked)]
pub mod core;
pub mod dsp;
pub mod lfo;
pub mod modulation;
pub mod params;
pub mod plugin;
pub mod swarc;
pub mod ui;
pub mod util;

use num_traits::Float;
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
        plugin::SunfishPlugin {
            host,
            ..Default::default()
        }
    }

    fn init(&mut self) {
        errors::setup_panic_handling();

        {
            use std::fs::File;
            use std::path::Path;

            use simplelog::{Config, LevelFilter, WriteLogger};

            let log_file = Path::new("/tmp/").join("sunfish.log");
            let f = File::create(&log_file);
            if let Ok(file) = f {
                // Ignore result.
                let _ = WriteLogger::init(LevelFilter::Info, Config::default(), file);
            }
        }
        log::info!("Started Sunfish VST",);
    }

    fn get_info(&self) -> Info {
        Info {
            name: "Sunfish".to_string(),

            // Version v0.1
            version: 100,

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

            // 64-bit processing.
            f64_precision: true,

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
            let meta = &self.core.modulation.params.meta;

            // Try to update the GUI
            // TODO: Consolidate all of this logic with the GUI -- it's identical
            if let Ok(ref mut for_gui_deltas) = self.for_gui_deltas.try_lock() {
                if self.for_gui_deltas_pending.any_changed() {
                    self.for_gui_deltas_pending_tracker.refresh_changed(
                        &self.core.modulation.params.meta,
                        &self.for_gui_deltas_pending,
                    );
                    for updated_eparam in &self.for_gui_deltas_pending_tracker.changed_list_cached {
                        for_gui_deltas.set_changed(&meta, &updated_eparam);
                    }
                    self.for_gui_deltas_pending.reset();
                }
                for_gui_deltas.set_changed(&meta, &eparam);
            } else {
                // store into pending
                self.for_gui_deltas_pending.set_changed(&meta, &eparam);
            }
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
        if ui::editor_supported() {
            Some(&mut self.editor)
        } else {
            None
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        self._process(buffer);
    }

    fn process_f64(&mut self, buffer: &mut AudioBuffer<f64>) {
        self._process(buffer);
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

impl plugin::SunfishPlugin {
    fn _process<F: Float>(&mut self, buffer: &mut AudioBuffer<F>) {
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
                *output_sample = F::from(0.0).unwrap();
            }
        }

        // Create a fixed slice of mutable slices (to avoid any heap allocations).
        let mut v: [&mut [F]; core::CHANNEL_COUNT] = Default::default();
        let ch_count = output_buffer.len().max(core::CHANNEL_COUNT);

        #[allow(clippy::needless_range_loop)]
        for ch in 0..ch_count {
            v[ch] = output_buffer.get_mut(ch);
        }

        // Resolve parameter updates from the GUI.
        self.handle_gui_and_host_parameter_updates();

        self.core.render(&mut v[..ch_count]);
    }
}

plugin_main!(plugin::SunfishPlugin);
