use crate::{
    kernel,
    look_behind::{LookBehind, SliceLike},
    polyphase_kernel::PolyphaseKernel,
};

/// A variable delay allows us to delay a different amount
/// each sample.
#[derive(Clone)]
pub struct ModulatedDelay {
    kernel: PolyphaseKernel,
    lookaround: u16,
    look_behind: LookBehind,
    max_delay: usize,
}

pub struct Options {
    /// This acts as a quality control for the filter.
    ///
    /// Higher lookarounds will have lower aliasing and better high-frequency fidelity,
    /// but this comes with two downsides: CPU cost is higher with higher lookarounds,
    /// and the delay must never be less than the lookaround (which is measured in samples).
    pub lookaround: u16,

    /// The maximum delay in samples.
    pub max_delay: usize,

    /// The maximum buffer size in samples.
    pub max_samples_per_process_call: usize,
}

fn dot<'a, 'b>(a: impl IntoIterator<Item = &'a f32>, b: impl IntoIterator<Item = &'b f32>) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}

pub struct Buffer<'a, B> {
    kernel: &'a PolyphaseKernel,
    lookaround: u16,
    max_delay: usize,
    view: B,
}

impl<'a, B: SliceLike + 'a> Buffer<'a, B> {
    pub fn process<'b: 'a, IDelay: std::iter::IntoIterator<Item = f32> + 'b>(
        &'a self,
        delay: IDelay,
    ) -> impl Iterator<Item = f32> + 'a {
        delay.into_iter().enumerate().map(|(i, d)| {
            debug_assert!(d >= 0f32);

            #[allow(clippy::cast_possible_truncation)]
            let di = d.round() as usize;
            #[allow(clippy::cast_precision_loss)]
            let frac = d - di as f32;

            #[allow(clippy::cast_possible_truncation)]
            let phase = ((frac + 0.5) * f32::from(self.kernel.phases())).floor() as u16;
            debug_assert!(di <= self.max_delay);
            debug_assert!(di >= self.lookaround as usize);
            debug_assert!(self.kernel.phase(phase).len() == 2 * self.lookaround as usize + 1);
            #[allow(clippy::range_plus_one)]
            let buffer_range = (i + self.max_delay - di)
                ..(i + self.max_delay - di + 2 * self.lookaround as usize + 1);
            dot(self.kernel.phase(phase), self.view.range(buffer_range))
        })
    }
}

impl ModulatedDelay {
    pub fn new(
        Options {
            lookaround,
            max_delay,
            max_samples_per_process_call,
        }: Options,
    ) -> Self {
        // Bandwidth relative to nyquist. This can reduce aliasing at the cost of
        // attenuating high frequencies (which are likely inaudible anyways)
        const BANDWIDTH: f32 = 0.85;

        let length_per_phase = lookaround * 2 + 1;
        let num_phases: u16 = 1025;
        let mut kernel = kernel::lpf(kernel::LpfOptions {
            length: length_per_phase * num_phases,
            increment: BANDWIDTH * 0.5 / f32::from(num_phases),
        });
        for tap in &mut kernel {
            *tap *= BANDWIDTH;
        }
        let kernel = PolyphaseKernel::split(&kernel, num_phases);
        Self {
            kernel,
            lookaround,
            look_behind: LookBehind::new(
                max_delay + lookaround as usize,
                max_samples_per_process_call,
            ),
            max_delay,
        }
    }

    pub fn reset(&mut self) {
        self.look_behind.reset();
    }

    pub fn process<'a, 'b: 'a, IAudio: std::iter::IntoIterator<Item = f32>>(
        &'a mut self,
        input: IAudio,
    ) -> Buffer<'a, impl SliceLike + 'a> {
        let input = input.into_iter();
        let view = self.look_behind.process(input);
        let max_delay = self.max_delay;
        let lookaround = self.lookaround;
        let kernel = &self.kernel;
        Buffer {
            kernel,
            lookaround,
            max_delay,
            view,
        }
    }
}

#[cfg(test)]
mod tests;
