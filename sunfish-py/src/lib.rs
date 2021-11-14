use ndarray;
use ndarray::{ArrayD, ArrayViewD, ArrayViewMutD};
use numpy;
use numpy::PyArray1;
use numpy::{IntoPyArray, PyArrayDyn};
use pyo3::exceptions;
use pyo3::prelude::*;

use sunfish::core;
use sunfish::dsp::osc;
use sunfish::lfo;
use sunfish::modulation::target::ModulationTarget;
use sunfish::params::NormalizedParams;
use sunfish::params::MAX_CUTOFF_SEMI;
use sunfish::params::{ELfoParams, EOscParams, EParam};
use sunfish::plugin;

const DEFAULT_TEMPO_BPS: f64 = 120.0;

#[pymodule]
fn pysunfish(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<CoreWrapper>()?;
    Ok(())
}

#[pyclass]
pub struct CoreWrapper {
    inst: core::Sunfish,
    tempo_bps: f64,
}

#[pymethods]
impl CoreWrapper {
    #[new]
    pub fn new(sample_rate: f64) -> Self {
        let mut plugin = plugin::SunfishPlugin::new();
        plugin.core.update_sample_rate(sample_rate);
        CoreWrapper {
            inst: plugin.core,
            tempo_bps: DEFAULT_TEMPO_BPS,
        }
    }

    pub fn update_param(&mut self, param_name: &str, param_value: f64) -> PyResult<()> {
        let eparam: EParam = serde_json::from_str(param_name)
            .map_err(|err| exceptions::PyValueError::new_err(err.to_string()))?;
        core::Sunfish::on_param_update(
            &self.inst.meta,
            &mut self.inst.params,
            &mut self.inst.params_modulated,
            &self.inst.tempo,
            &mut self.inst.voices,
            &mut self.inst.modulation,
            eparam,
            param_value,
        );
        Ok(())
    }

    fn note_on(&mut self, note: u8) -> PyResult<()> {
        self.inst.note_on(note, 100);
        Ok(())
    }

    fn note_off(&mut self, note: u8) -> PyResult<()> {
        self.inst.note_off(note);
        Ok(())
    }

    fn render(
        &mut self,
        py: Python,
        chunk_size: usize,
        buf_len: usize,
        shape: String,
    ) -> PyResult<(Py<PyArray1<f32>>, Py<PyArray1<f32>>)> {
        let mut l_signal = vec![0.0; buf_len];
        let mut r_signal = vec![0.0; buf_len];
        for start_idx in (0..buf_len).step_by(chunk_size) {
            let end_idx = (start_idx + chunk_size).min(buf_len);
            let mut l_chunk = &mut l_signal[start_idx..end_idx];
            let mut r_chunk = &mut r_signal[start_idx..end_idx];
            self.inst.render(&mut [&mut l_chunk, &mut r_chunk]);
        }
        let l_array = l_signal.into_pyarray(py);
        let r_array = r_signal.into_pyarray(py);
        Ok((l_array.to_owned(), r_array.to_owned()))
    }
}

/// Render the waveforms.
///
/// chunk_size: How big a buffer to handle render.
/// buf_len: Total buffer length.
fn render(
    sample_rate: f64,
    chunk_size: usize,
    buf_len: usize,
    shape: osc::WaveShape,
    notes: Vec<u8>,
) -> (ndarray::Array1<f32>, ndarray::Array1<f32>) {
    let mut l_signal = vec![0.0; buf_len];
    let mut r_signal = vec![0.0; buf_len];

    let mut plugin = plugin::SunfishPlugin::new();
    plugin.core.update_sample_rate(sample_rate);
    let synth = &mut plugin.core;
    for &note in notes.iter() {
        l_signal = vec![0.0; buf_len];
        r_signal = vec![0.0; buf_len];

        synth.note_on(note, 100);

        for start_idx in (0..buf_len).step_by(chunk_size) {
            let end_idx = (start_idx + chunk_size).min(buf_len);
            let mut l_chunk = &mut l_signal[start_idx..end_idx];
            let mut r_chunk = &mut r_signal[start_idx..end_idx];
            synth.render(&mut [&mut l_chunk, &mut r_chunk]);
        }
    }
    let l_array = ndarray::Array::from_shape_vec((buf_len,), l_signal.to_vec()).unwrap();
    let r_array = ndarray::Array::from_shape_vec((buf_len,), r_signal.to_vec()).unwrap();
    (l_array, r_array)
}
