//! This is a digital "state variable filter". This filter
//! is stable under time-varying parameters.

#[derive(Debug, Clone, Copy, Default)]
pub struct Svf {
    s0: f64,
    s1: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct RawParams {
    pub g: f64,
    pub two_r: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Input {
    pub x: f64,
    pub params: RawParams,
}

#[derive(Debug, Clone, Copy)]
pub struct Output {
    pub low: f64,
    pub band: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputNoHigh {
    pub low: f64,
    pub band: f64,
}

pub fn calc_g(incr: f64) -> f64 {
    (std::f64::consts::TAU / 2. * incr).tan()
}

pub fn calc_two_r(q: f64) -> f64 {
    1. / q
}

impl Svf {
    pub fn process<'a, I: IntoIterator<Item = Input> + 'a>(
        &'a mut self,
        inputs: I,
    ) -> impl Iterator<Item = Output> + '_ {
        inputs.into_iter().map(
            move |Input {
                      x,
                      params: RawParams { g, two_r: damping },
                  }| {
                // following https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf
                let d = 1. / (1. + damping * g + g * g);
                let high = d * (x - (damping + g) * self.s0 - self.s1);
                let v0 = g * high;
                let band = v0 + self.s0;
                self.s0 = band + v0;
                let v1 = g * band;
                let low = v1 + self.s1;
                self.s1 = low + v1;
                Output { low, band, high }
            },
        )
    }

    pub fn process_no_high<'a, I: IntoIterator<Item = Input> + 'a>(
        &'a mut self,
        inputs: I,
    ) -> impl Iterator<Item = OutputNoHigh> + '_ {
        inputs.into_iter().map(
            move |Input {
                      x,
                      params: RawParams { g, two_r: damping },
                  }| {
                // following https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf
                let d = 1. / (1. + damping * g + g * g);
                let band = d * (g * (x - self.s1) + self.s0);
                let v0 = band - self.s0;
                self.s0 = v0 + band;
                let v1 = g * band;
                let low = v1 + self.s1;
                self.s1 = low + v1;

                OutputNoHigh { low, band }
            },
        )
    }

    pub fn process_high<'a, I: IntoIterator<Item = Input> + 'a>(
        &'a mut self,
        inputs: I,
    ) -> impl Iterator<Item = f64> + '_ {
        self.process(inputs).map(|Output { high, .. }| high)
    }

    pub fn process_band<'a, I: IntoIterator<Item = Input> + 'a>(
        &'a mut self,
        inputs: I,
    ) -> impl Iterator<Item = f64> + '_ {
        inputs.into_iter().map(
            move |Input {
                      x,
                      params: RawParams { g, two_r: damping },
                  }| {
                // following https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf
                let d = 1. / (1. + damping * g + g * g);
                let band = d * (g * (x - self.s1) + self.s0);
                let band2 = band + band;
                self.s0 = band2 - self.s0;
                let v22 = g * band2;
                self.s1 += v22;
                band
            },
        )
    }

    pub fn process_low<'a, I: IntoIterator<Item = Input> + 'a>(
        &'a mut self,
        inputs: I,
    ) -> impl Iterator<Item = f64> + '_ {
        self.process_no_high(inputs)
            .map(|OutputNoHigh { low, .. }| low)
    }

    pub fn reset(&mut self) {
        *self = Default::default();
    }
}

#[cfg(test)]
mod tests;
