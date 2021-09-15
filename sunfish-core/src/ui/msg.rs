// Messages used for communicating between the processing
// thread and the GUI thread.
use crate::common::Float;
use crate::params;

#[allow(dead_code)]
pub enum Msg {
    ShowGui,
    HideGui,
    ParamUpdate { eparam: params::EParam, value: Float },
}
