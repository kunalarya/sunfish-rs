use std::collections::HashMap;

pub trait ParamType<T> {
    fn vst_float_to_value(&self, value_unit: f64) -> T;
    fn value_to_vst_float(&self, value: T) -> f64;
}

#[derive(Clone, Debug)]
pub struct Boolean;

impl Boolean {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Boolean {}
    }
}

impl ParamType<bool> for Boolean {
    fn vst_float_to_value(&self, value_unit: f64) -> bool {
        value_unit >= 0.5
    }
    fn value_to_vst_float(&self, value: bool) -> f64 {
        if value {
            1.0
        } else {
            0.0
        }
    }
}

/// Support gradual attack times.
#[derive(Clone, Debug)]
pub struct GradualTime {
    piece0_point: f64,
    piece0_minmax: (f64, f64),
    piece1_point: f64,
    piece1_minmax: (f64, f64),
    piece2_point: f64,
    piece2_minmax: (f64, f64),
}

impl GradualTime {
    pub fn for_attack() -> Self {
        // Piecewise attack time:
        // norm:       actual:
        // 0.0-0.3     1ms-100ms
        // 0.3-0.6     100ms-500ms
        // 0.6-1.0     500ms-6seconds
        Self {
            piece0_point: 0.3,
            piece0_minmax: (0.001, 0.100),
            piece1_point: 0.6,
            piece1_minmax: (0.100, 0.500),
            piece2_point: 1.0,
            piece2_minmax: (0.500, 6.000),
        }
    }
    pub fn for_decay() -> Self {
        // Piecewise attack time:
        // norm:       actual:
        // 0.0-0.3     0.1ms-300ms
        // 0.3-0.6     300ms-750ms
        // 0.6-1.0     750ms-10seconds
        Self {
            piece0_point: 0.3,
            piece0_minmax: (0.0001, 0.300),
            piece1_point: 0.6,
            piece1_minmax: (0.300, 0.750),
            piece2_point: 1.0,
            piece2_minmax: (0.750, 10.000),
        }
    }
}

impl ParamType<f64> for GradualTime {
    fn vst_float_to_value(&self, value_unit: f64) -> f64 {
        if value_unit <= self.piece0_point {
            let (min, max) = self.piece0_minmax;
            let target_range = max - min;
            min + ((value_unit / self.piece0_point) * target_range)
        } else if value_unit <= self.piece1_point {
            let (min, max) = self.piece1_minmax;
            let target_range = max - min;
            // offset value
            let value = value_unit - self.piece0_point;
            let source_range = self.piece1_point - self.piece0_point;
            min + ((value / source_range) * target_range)
        } else {
            let (min, max) = self.piece2_minmax;
            let target_range = max - min;
            // offset value
            let value = value_unit - self.piece1_point;
            let source_range = self.piece2_point - self.piece1_point;
            min + ((value / source_range) * target_range)
        }
    }

    fn value_to_vst_float(&self, value_full: f64) -> f64 {
        let mapped_value = if value_full <= self.piece0_minmax.1 {
            let (min, max) = self.piece0_minmax;
            // subtract the target min
            let value = value_full - min;
            // normalize to 0.0-1.0
            let value_norm = value / (max - min);
            let source_range = self.piece0_point;
            // map to source range
            value_norm * source_range
        } else if value_full <= self.piece1_minmax.1 {
            let (min, max) = self.piece1_minmax;
            // subtract the target min
            let value = value_full - min;
            // normalize to 0.0-1.0
            let value_norm = value / (max - min);
            let source_range = self.piece1_point - self.piece0_point;
            // map to source range
            self.piece1_point + (value_norm * source_range)
        } else {
            let (min, max) = self.piece2_minmax;
            // subtract the target min
            let value = value_full - min;
            // normalize to 0.0-1.0
            let value_norm = value / (max - min);
            let source_range = self.piece2_point - self.piece1_point;
            // map to source range
            self.piece2_point + (value_norm * source_range)
        };

        mapped_value.max(0.0).min(1.0)
    }
}

#[derive(Clone, Debug)]
pub struct LinearDiscrete {
    _min_float: f64,
    _max_float: f64,
    _range_float: f64,
}

impl LinearDiscrete {
    pub fn new(min: i32, max: i32) -> Self {
        let _min_float = min as f64;
        let _max_float = max as f64;
        LinearDiscrete {
            _min_float,
            _max_float,
            _range_float: _max_float - _min_float,
        }
    }
}

impl ParamType<i32> for LinearDiscrete {
    fn vst_float_to_value(&self, value_unit: f64) -> i32 {
        // value_full will be from 0.0 to 1.0
        let result = (value_unit * self._range_float) + self._min_float;
        // Clamp to min and max.
        result.max(self._min_float).min(self._max_float) as i32
    }

    fn value_to_vst_float(&self, value_full: i32) -> f64 {
        // value_unit will be from min to max; scale back to 0 to 1
        let result = (value_full as f64 - self._min_float) / self._range_float;
        result.max(0.0).min(1.0)
    }
}

#[derive(Clone, Debug)]
pub struct Enum<T>
where
    T: Clone + std::hash::Hash + Eq,
{
    // For each thres, set the value.
    value_to_thresh: HashMap<T, f64>,
    // These are thresholds
    thresh_to_value: Vec<(f64, T)>,
}

impl<T> Enum<T>
where
    T: Clone + std::hash::Hash + Eq,
{
    pub fn new(options: Vec<T>) -> Self {
        let thresh_step = 1.0 / options.len() as f64;
        let (value_to_thresh, thresh_to_value) = {
            let mut m = HashMap::<T, f64>::new();
            let mut v = Vec::<(f64, T)>::new();
            for (idx, value) in options.iter().enumerate() {
                let step = (idx) as f64 * thresh_step;
                m.insert(value.clone(), step);
                v.push((step, value.clone()));
            }
            (m, v)
        };
        Enum {
            value_to_thresh,
            thresh_to_value,
        }
    }
}

impl<T> ParamType<T> for Enum<T>
where
    T: Clone + std::hash::Hash + Eq,
{
    fn vst_float_to_value(&self, value: f64) -> T {
        let mut ret: T = self.thresh_to_value[0].1.clone();
        for (thresh, item) in self.thresh_to_value.iter() {
            if value >= *thresh {
                ret = item.clone();
            }
        }
        // Default to the first element (TODO: Allow user to specify "fallback" value)
        ret
    }
    fn value_to_vst_float(&self, value: T) -> f64 {
        *self.value_to_thresh.get(&value).unwrap_or(&0.0)
    }
}

#[derive(Clone, Debug)]
pub struct Linear {
    pub min: f64,
    pub max: f64,
}

impl Linear {
    pub fn new(min: f64, max: f64) -> Self {
        Linear { min, max }
    }
}

impl ParamType<f64> for Linear {
    fn vst_float_to_value(&self, value_unit: f64) -> f64 {
        // value_full will be from 0.0 to 1.0
        let range = self.max - self.min;
        let result = (value_unit * range) + self.min;
        // Clamp to min and max.
        result.max(self.min).min(self.max)
    }

    fn value_to_vst_float(&self, value_full: f64) -> f64 {
        // value_unit will be from min to max; scale back to 0 to 1
        let range = self.max - self.min;
        let result = (value_full - self.min) / range;
        result.max(0.0).min(1.0)
    }
}
