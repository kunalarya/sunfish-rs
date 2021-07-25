use crate::dsp::filter::FilterMode;

#[derive(Clone, Debug)]
pub struct ResonantFilter {
    pub mode: FilterMode,
    cutoff_freq: f32,
    res: f32,
    buf0: f32,
    buf1: f32,
    feedback_amt: f32,
}

impl ResonantFilter {
    pub fn new(mode: FilterMode, cutoff_freq: f32, res: f32) -> ResonantFilter {
        ResonantFilter {
            mode,
            cutoff_freq,
            res,
            buf0: 0f32,
            buf1: 0f32,
            feedback_amt: 0f32,
        }
    }

    pub fn apply(&mut self, input: f32) -> f32 {
        // By Paul Kellett
        // http://www.musicdsp.org/showone.php?id=29
        // http://www.musicdsp.org/en/latest/Filters/29-resonant-filter.html
        // via:
        // http://www.martin-finke.de/blog/articles/audio-plugins-013-filter/
        //
        // note: f = 2.0*sin(pi*freq/samplerate);

        self.feedback_amt = self.res + self.res / (1.0 - self.cutoff_freq);
        let b0 =
            self.cutoff_freq * (input - self.buf0 + self.feedback_amt * (self.buf0 - self.buf1));
        self.buf0 += b0;
        self.buf1 += self.cutoff_freq * (self.buf0 - self.buf1);
        match self.mode {
            FilterMode::LowPass => self.buf1,
            FilterMode::HighPass => input - self.buf0,
            FilterMode::BandPass => self.buf0 - self.buf1,
            FilterMode::PassThru => input,
        }
    }
}
