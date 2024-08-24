#[derive(Clone)]
pub struct Lfo {
    point: f32,
    scale: f32,

    // Note that for BBDs, we average the delay over a fixed window (the LFO controls
    // the high speed clock, but the total delay is the delay of each tick * the length
    // of the BBD line).
    //
    // It would be fun to write an application note about this!
    //
    // Here, we build this behavior into the LFO rather than the modulated
    // delay line to make the delay line more general. This is a matter of taste!
    // But, it's important to reproduce this somewhere, as sending a triangle
    // wave directly to the modulated delay line will create instant frequency
    // jumps that don't exist in BBD delays.
    //
    // We approximate this by using an exponential smoothing filter (rather than
    // a moving average which would be a better approximation) just for simplicity.
    //
    // Another thing to mention is that on a real device, when the delay is higher,
    // the smoothing is over a longer time - we don't reproduce this nuance here and
    // always use a fix time-constant for smoothing (jeez, really should write more on this).
    alpha: f32,

    phase: f32,
    output: Option<f32>,
}

pub struct Buffer<F, R> {
    pub forward: F,
    pub reverse: R,
}

#[derive(Clone, Copy)]
pub struct Options {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Copy)]
pub struct Parameters {
    pub incr: f32,

    /// In percent
    pub depth: f32,
}

/// Time-constant in samples
fn alpha_from_time_constant(t: f32) -> f32 {
    1. - (-1. / t).exp()
}

impl Lfo {
    pub fn new(Options { min, max }: Options) -> Self {
        assert!(min < max);
        let point = (max + min) * 0.5;

        // Use double the zero-point as the time-constant for the smoothing
        let alpha = alpha_from_time_constant(2. * point);

        Self {
            point,
            scale: (max - min) / 100. * 2.,
            alpha,
            output: None,
            phase: 0.,
        }
    }

    fn run_single(&mut self, Parameters { incr, depth }: Parameters) -> f32 {
        let instant = depth
            * self.scale
            * (if self.phase > 0.5 {
                1. - self.phase
            } else {
                self.phase
            } - 0.25);
        if incr < 0.5 {
            self.phase += incr;
            if self.phase > 1. {
                self.phase -= 1.;
            }
        }
        self.output = match self.output {
            Some(output) => Some(output + self.alpha * (instant - output)),
            None => Some(instant),
        };
        self.output.unwrap()
    }

    pub fn run<P: IntoIterator<Item = Parameters> + Clone>(
        &mut self,
        params: P,
    ) -> Buffer<impl Iterator<Item = f32>, impl Iterator<Item = f32>> {
        let mut forward_lfo = self.clone();
        let mut reverse_lfo = self.clone();

        // Note that we just separately run the LFO here and also in each
        // returned iterator. Kinda slow, but the alternative would require
        // memory storage to store the outputs!
        for param in params.clone() {
            self.run_single(param);
        }

        let forward = params
            .clone()
            .into_iter()
            .map(move |p| forward_lfo.point + forward_lfo.run_single(p));

        let reverse = params
            .into_iter()
            .map(move |p| reverse_lfo.point - reverse_lfo.run_single(p));

        Buffer { forward, reverse }
    }

    pub fn reset(&mut self) {
        self.phase = 0.;
        self.output = None;
    }
}

#[cfg(test)]
mod tests;
