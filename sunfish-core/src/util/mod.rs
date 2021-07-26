pub mod enumerable;
pub mod errors;
pub mod note_freq;
pub mod test_utils;

// From freeverb.c
// #define undenormalize(n) { if (xabs(n) < 1e-37) { (n) = 0; } }
#[inline(always)]
pub fn undenormalize(f: &mut f64) {
    if f.abs() < 1e-37 {
        *f = 0.0;
    }
}

#[cfg(target_arch = "x86_64")]
pub fn setup_undenormalization() {
    // From: https://gist.github.com/GabrielMajeri/545042ee4f956d5b2141105eb6a505a9

    // Potentially improves the performance of SIMD floating-point math
    // by flushing denormals/underflow to zero.
    unsafe {
        use std::arch::x86_64::*;

        let mut mxcsr = _mm_getcsr();

        // Denormals & underflows are flushed to zero
        mxcsr |= (1 << 15) | (1 << 6);

        // All exceptions are masked
        mxcsr |= ((1 << 6) - 1) << 7;

        _mm_setcsr(mxcsr);
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn setup_undenormalization() {
    // Do nothing.
}

pub fn semitones_to_frequency(semitones: f64, min_hz: f64) -> f64 {
    // n = 12 * log2 (freq / freq_base).
    // n/12 = log2 (freq / freq_base).
    // 2^(n/12) = freq / freq_base
    // freq_base * (2^(n/12)) = freq
    min_hz * (2.0f64).powf(semitones / 12.0)
}

pub fn frequency_to_semitones(freq_hz: f64, min_hz: f64) -> f64 {
    // n = 12 * log2 (freq / freq_base).
    (freq_hz / min_hz).log2() * 12.0
}

pub fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.log10()
}
