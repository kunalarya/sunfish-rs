use std::time::{Duration, Instant};

use vst::plugin::HostCallback;

use crate::core::{Sunfish, Tempo};
use crate::modulation;
use crate::params;
use crate::params::deltas;
use crate::util;

// Glues core signal logic with editor.
pub struct SunfishPlugin {
    pub core: Sunfish,
    pub host: HostCallback,
    pub tempo: Tempo,
    pub last_host_param_update: Instant,
    pub host_param_update_tick: Duration,

    pub for_host_deltas: deltas::Deltas,
    pub for_host_delta_tracker: deltas::DeltaChangeTracker,
}

impl SunfishPlugin {
    pub fn new() -> SunfishPlugin {
        // Set up thread-wide undenormalization (for SSE).
        util::setup_undenormalization();

        let sample_rate = 44100.0;

        // Create the parameters themselves.
        let params = params::SunfishParams::new(sample_rate);

        // Param set (baseline, modulated, etc.) for the plugin parameters.
        let param_set = modulation::ParamSet::new("main".to_string(), params);

        // How often to update host with new param values.
        let host_param_update_tick = Duration::from_micros(500);

        // For keeping track of updates from the GUI to send to the host.
        let for_host_deltas = deltas::Deltas::new(&param_set.meta);
        let for_host_delta_tracker = for_host_deltas.create_tracker();

        let modulation = modulation::Modulation::new(sample_rate, param_set);
        // Give the core thread read access to GUI's inputs.
        let core = Sunfish::new(sample_rate, modulation);

        SunfishPlugin {
            core,
            host: HostCallback::default(),
            tempo: Tempo::new(1.0),
            last_host_param_update: Instant::now() - host_param_update_tick,
            host_param_update_tick,
            for_host_deltas,
            for_host_delta_tracker,
        }
    }
}

impl Default for SunfishPlugin {
    fn default() -> Self {
        SunfishPlugin::new()
    }
}
