use std::time::{Duration, Instant};

use vst::host::Host;
use vst::plugin::HostCallback;

use crate::core::{Sunfish, Tempo};
use crate::modulation;
use crate::params;
use crate::params::sync::{Subscriber, Synchronizer};
use crate::ui::editor::SunfishEditor;
use crate::util;

// Glues core signal logic with editor.
pub struct SunfishPlugin {
    pub core: Sunfish,
    pub editor: SunfishEditor,
    pub host: HostCallback,
    pub host_subscriber: Subscriber,
    pub last_host_param_update: Instant,
    pub host_param_update_tick: Duration,
}

impl SunfishPlugin {
    pub fn new() -> SunfishPlugin {
        // Set up thread-wide undenormalization (for SSE).
        util::setup_undenormalization();

        let sample_rate = 44100.0;

        // Create the parameters themselves.
        let params = params::Params::new(sample_rate);
        let meta = params::ParamsMeta::new();

        let mut synchronizer = Synchronizer::new(meta.clone(), params);
        let gui_subscriber = synchronizer.subscriber();
        let host_subscriber = synchronizer.subscriber();

        let core_mailbox = synchronizer.mailbox();

        let gui_synchronizer = synchronizer.clone();

        // How often to update host with new param values.
        let host_param_update_tick = Duration::from_micros(500);
        let modulation = modulation::Modulation::new(sample_rate);

        // Give the core thread read access to GUI's inputs.
        let core = Sunfish::new(
            meta,
            sample_rate,
            core_mailbox,
            synchronizer,
            modulation,
            Tempo::new(1.0),
        );

        SunfishPlugin {
            core,
            editor: SunfishEditor::new(gui_synchronizer, gui_subscriber),
            host: HostCallback::default(),

            host_subscriber,
            last_host_param_update: Instant::now() - host_param_update_tick,
            host_param_update_tick,
        }
    }

    pub fn update_host_parameters(&mut self) {
        if let Ok(guard) = self.host_subscriber.changes.lock() {
            let changes = &(*guard);
            for (updated_eparam, updated_value) in changes {
                let index = self.core.meta.param_to_index(updated_eparam).unwrap();
                self.host.automate(index as i32, *updated_value as f32);
            }
        }
    }
}

impl Default for SunfishPlugin {
    fn default() -> Self {
        SunfishPlugin::new()
    }
}
