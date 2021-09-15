use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vst::host::Host;
use vst::plugin::HostCallback;

use crate::core::{Sunfish, Tempo};
use crate::modulation;
use crate::params;
use crate::params::deltas;
use crate::params::NormalizedParams;
use crate::swarc;
use crate::ui::editor::SunfishEditor;
use crate::util;

// Glues core signal logic with editor.
pub struct SunfishPlugin {
    pub core: Sunfish,
    pub editor: SunfishEditor,
    pub host: HostCallback,
    pub tempo: Tempo,
    pub last_host_param_update: Instant,
    pub host_param_update_tick: Duration,

    // The GUI has its own cloneable view of parameters.
    pub gui_param_set: swarc::ArcReader<modulation::ParamSet>,
    pub from_gui_delta_tracker: deltas::DeltaChangeTracker,
    pub from_gui_deltas: Arc<Mutex<deltas::Deltas>>,

    pub for_host_deltas: deltas::Deltas,
    pub for_host_delta_tracker: deltas::DeltaChangeTracker,

    pub for_gui_deltas: Arc<Mutex<deltas::Deltas>>,
    pub for_gui_deltas_pending: deltas::Deltas,
    pub for_gui_deltas_pending_tracker: deltas::DeltaChangeTracker,
}

impl SunfishPlugin {
    pub fn new() -> SunfishPlugin {
        // Set up thread-wide undenormalization (for SSE).
        util::setup_undenormalization();

        let sample_rate = 44100.0;

        // Create the parameters themselves.
        let params = params::SunfishParams::new(sample_rate);
        // Create a copy for the GUI.
        let gui_params = params.clone();

        // Param set (baseline, modulated, etc.) for the plugin parameters.
        let param_set = modulation::ParamSet::new("main".to_string(), params);

        // TODO: Update sample rate logic for GUI params.
        let gui_param_set = modulation::ParamSet::new("gui_param_set".to_string(), gui_params);
        let (gui_param_set_writer, gui_param_set) = swarc::new(gui_param_set);

        let baseline_deltas = deltas::Deltas::new(&param_set.meta);
        let from_gui_delta_tracker = baseline_deltas.create_tracker();

        // Deltas from GUI:
        // One copy is for the core audio thread:
        let from_gui_deltas = Arc::new(Mutex::new(baseline_deltas));
        // The second is for the GUI.
        let editor_gui_deltas = Arc::clone(&from_gui_deltas);

        // Deltas back to GUI, from the host/core.
        let for_gui_deltas = Arc::new(Mutex::new(deltas::Deltas::new(&param_set.meta)));
        let for_gui_deltas_pending = deltas::Deltas::new(&param_set.meta);
        let for_gui_deltas_pending_tracker = for_gui_deltas_pending.create_tracker();

        // How often to update host with new param values.
        let host_param_update_tick = Duration::from_micros(500);

        // Request clonable parameter readers for the GUI to read (baseline & modulated)
        let baseline_param_reader = swarc::ArcReader::clone(&param_set.baseline);
        let modulated_param_reader = swarc::ArcReader::clone(&param_set.modulated);

        // For keeping track of updates from the GUI to send to the host.
        let for_host_deltas = deltas::Deltas::new(&param_set.meta);
        let for_host_delta_tracker = for_host_deltas.create_tracker();

        let modulation = modulation::Modulation::new(sample_rate, param_set);
        // Give the core thread read access to GUI's inputs.
        let core = Sunfish::new(sample_rate, modulation);

        SunfishPlugin {
            core,
            editor: SunfishEditor::new(
                baseline_param_reader,
                modulated_param_reader,
                gui_param_set_writer,
                editor_gui_deltas,
                Arc::clone(&for_gui_deltas),
            ),
            host: HostCallback::default(),
            tempo: Tempo::new(1.0),
            last_host_param_update: Instant::now() - host_param_update_tick,
            host_param_update_tick,
            gui_param_set,
            from_gui_delta_tracker,
            from_gui_deltas,
            for_host_deltas,
            for_host_delta_tracker,
            for_gui_deltas: Arc::clone(&for_gui_deltas),
            for_gui_deltas_pending,
            for_gui_deltas_pending_tracker,
        }
    }

    pub fn handle_gui_and_host_parameter_updates(&mut self) {
        // Grab the lock, check for updates.
        let mut any_changed = false;

        if let Ok(ref mut from_gui_deltas) = self.from_gui_deltas.try_lock() {
            let meta = &self.core.modulation.params.meta;
            if from_gui_deltas.any_changed() {
                // Update the cached changes.
                self.from_gui_delta_tracker
                    .refresh_changed(meta, &from_gui_deltas);
                from_gui_deltas.reset();
                any_changed = true;
            }
        }
        if any_changed {
            // Now see which parameters changed and update them.
            for changed in &self.from_gui_delta_tracker.changed_list_cached {
                let meta = &self.core.modulation.params.meta;
                let param_value = self
                    .gui_param_set
                    .baseline
                    .get_param_normalized(meta, *changed)
                    .unwrap();

                self.core
                    .modulation
                    .params
                    .baseline_writer
                    .update_param(meta, *changed, param_value)
                    .unwrap();
                self.for_host_deltas.set_changed(meta, &changed);

                // Notify core.
                self.core
                    .notify_param_update(*changed, param_value, self.tempo.tempo_bps);
            }
        }

        let now = Instant::now();
        if now - self.last_host_param_update > self.host_param_update_tick {
            let meta = &self.core.modulation.params.meta;
            // Check any changed host params.
            if self.for_host_deltas.any_changed() {
                // Update the cached changes.
                self.for_host_delta_tracker
                    .refresh_changed(meta, &self.for_host_deltas);
                self.for_host_deltas.reset();
                // Now see which parameters changed and update them.
                for changed in &self.for_host_delta_tracker.changed_list_cached {
                    let index = meta.param_to_index(&changed).unwrap();
                    let param_value = self
                        .core
                        .modulation
                        .params
                        .baseline
                        .get_param_normalized(meta, *changed)
                        .unwrap_or(0.0);
                    self.host.automate(index as i32, param_value as f32);
                }
            }

            self.last_host_param_update = now;
        }
    }
}

impl Default for SunfishPlugin {
    fn default() -> Self {
        SunfishPlugin::new()
    }
}
