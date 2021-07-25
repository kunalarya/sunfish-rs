use std::collections::HashMap;

use lazy_static::lazy_static;

pub const MIDI_NOTE_MIN: i32 = 2;
//  192 is the theoretical max, but that's 500Khz
pub const MIDI_NOTE_MAX: i32 = 135;

lazy_static! {
    pub static ref NOTE_TO_FREQ: HashMap<i32, f64> = {
        let mut note_freqs: HashMap<i32, f64> = HashMap::new();
        for note in MIDI_NOTE_MIN..MIDI_NOTE_MAX {
            note_freqs.insert(note, freq_for(note));
        }
        note_freqs
    };
}

fn freq_for(note: i32) -> f64 {
    let base_note = note - 69;
    ((base_note as f64) / 12.0).exp2() * 440.0
}
