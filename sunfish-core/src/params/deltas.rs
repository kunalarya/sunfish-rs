use num::integer;

use crate::params::{EParam, SunfishParamsVstMeta};

struct Bitmask {
    value: Vec<u64>,
}

impl Bitmask {
    fn new(size: usize) -> Self {
        let bytes = (size as f32 / 8.0).ceil() as usize;
        Bitmask {
            value: vec![0; bytes],
        }
    }

    #[inline(always)]
    fn set(&mut self, index: usize) {
        let (byte_index, bit_index) = integer::div_rem(index, 8);
        self.value[byte_index] |= 1 << bit_index;
    }
    #[inline(always)]
    fn any_set(&self) -> bool {
        for byte in &self.value {
            if *byte != 0 {
                return true;
            }
        }
        false
    }
    #[inline(always)]
    fn get(&self, index: &usize) -> bool {
        let (byte_index, bit_index) = integer::div_rem(*index, 8);
        (self.value[byte_index] & (1 << bit_index)) != 0
    }

    // TODO: Combine reset and any_set.
    fn reset(&mut self) {
        for byte in self.value.iter_mut() {
            *byte = 0;
        }
    }

    /// Set all bits high up to max. Not designed to
    /// be efficient.
    pub fn set_all(&mut self, max: usize) {
        for index in 0..max {
            self.set(index);
        }
    }
}

/// Stores a bitmask of which parameters have changed.
pub struct Deltas {
    changed: Bitmask,
    size: usize,
}

impl Deltas {
    pub fn new(meta: &SunfishParamsVstMeta) -> Self {
        let size = meta.param_to_index.len();
        let changed = Bitmask::new(size);
        Deltas { changed, size }
    }

    #[inline(always)]
    pub fn set_changed(&mut self, meta: &SunfishParamsVstMeta, eparam: &EParam) {
        self.changed.set(meta.param_to_index(eparam).unwrap());
    }

    pub fn set_all(&mut self) {
        self.changed.set_all(self.size);
    }

    pub fn any_changed(&self) -> bool {
        self.changed.any_set()
    }
    pub fn reset(&mut self) {
        self.changed.reset();
    }

    pub fn create_tracker(&self) -> DeltaChangeTracker {
        DeltaChangeTracker {
            changed_list_cached: Vec::with_capacity(self.size),
        }
    }
}

/// Tracker stores a pre-allocated vector that can be efficiently used with
/// Deltas object to get the exact changed parameters.
pub struct DeltaChangeTracker {
    // Stores lists of changed parameters, cached to avoid
    // allocations in the critical path.
    pub changed_list_cached: Vec<EParam>,
}

impl DeltaChangeTracker {
    // TODO: This is a bottleneck; there should be a faster way to do this,
    // maybe through an iterator?
    pub fn refresh_changed(&mut self, meta: &SunfishParamsVstMeta, deltas: &Deltas) {
        self.changed_list_cached.clear();
        for (index, eparam) in meta.paramlist.iter().enumerate() {
            if deltas.changed.get(&index) {
                self.changed_list_cached.push(*eparam);
            }
        }
    }
}
