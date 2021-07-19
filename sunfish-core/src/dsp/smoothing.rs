// From: https://www.musicdsp.org/en/latest/Filters/257-1-pole-lpf-for-smooth-parameter-changes.html

/// Simple one-pole LPF for parameter smoothing.
#[derive(Clone, Debug)]
struct SmoothingFilter {
    a: f64,
    b: f64,
    z: f64,
}

impl SmoothingFilter {
    fn new(initial_value: f64, sample_rate: f64, smoothing_time_sec: f64) -> Self {
        let a = (-std::f64::consts::TAU / (smoothing_time_sec * sample_rate)).exp();
        let b = 1.0 - a;
        let z = initial_value;
        Self { a, b, z }
    }

    fn process(&mut self, input: f64) -> f64 {
        self.z = (input * self.b) + (self.z * self.a);
        self.z
    }
}

#[derive(Clone, Debug)]
pub struct SlewRateLimiter {
    filter: SmoothingFilter,
    user_value: f64,
    pub filtered_value: f64,
    slew_threshold: f64,
}

impl SlewRateLimiter {
    pub fn new(value: f64, sample_rate: f64, smoothing_time_sec: f64, slew_threshold: f64) -> Self {
        Self {
            filter: SmoothingFilter::new(value, sample_rate, smoothing_time_sec),
            user_value: value,
            filtered_value: value,
            slew_threshold,
        }
    }

    pub fn step(&mut self) -> bool {
        let last_filtered_value = self.filtered_value;
        self.filtered_value = self.filter.process(self.user_value);
        let changed_significantly =
            (last_filtered_value - self.filtered_value).abs() > self.slew_threshold;
        changed_significantly
    }

    pub fn update(&mut self, new_value: f64) {
        self.user_value = new_value;
    }
}
