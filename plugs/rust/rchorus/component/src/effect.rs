use crate::compander::{compress, expand, PeakLevelDetector};
use crate::nonlinearity::nonlinearity;
use crate::{anti_aliasing_filter::AntiAliasingFilter, lfo, modulated_delay};
use component::{
    audio::{Buffer, BufferMut, ChannelLayout},
    effect::Effect as EffectT,
    parameters::{self, BufferStates},
    pzip, ProcessingEnvironment, Processor,
};
use iir::dc_blocker::DcBlocker;
use itertools::izip;
use num_traits::cast;

pub struct Effect {
    lfo: lfo::Lfo,
    delay_l: modulated_delay::ModulatedDelay,
    rate_to_incr_scale: f32,
    pre_filter: AntiAliasingFilter,
    post_filter: AntiAliasingFilter,
    dc_blocker: DcBlocker,
    detector: PeakLevelDetector,
}

impl Processor for Effect {
    fn set_processing(&mut self, processing: bool) {
        if !processing {
            self.lfo.reset();
            self.delay_l.reset();
            self.pre_filter.reset();
            self.post_filter.reset();
            self.dc_blocker.reset();
            self.detector.reset();
        }
    }
}

const PERCENT_SCALE: f32 = 1. / 100.;

impl Effect {
    pub fn new(env: &ProcessingEnvironment) -> Self {
        const LOOKAROUND: u8 = 8;

        let mut min_delay = 0.00166 * env.sampling_rate;
        if min_delay < f32::from(LOOKAROUND) {
            min_delay = f32::from(LOOKAROUND);
        }
        let mut max_delay = 0.00535 * env.sampling_rate;
        if max_delay < min_delay {
            max_delay = min_delay + 1.0;
        }
        let delay_l = modulated_delay::ModulatedDelay::new(modulated_delay::Options {
            lookaround: u16::from(LOOKAROUND),
            max_delay: cast::<f32, usize>(max_delay.ceil()).unwrap(),
            max_samples_per_process_call: env.max_samples_per_process_call,
        });
        Effect {
            lfo: lfo::Lfo::new(lfo::Options {
                min: min_delay,
                max: max_delay,
            }),
            delay_l,
            rate_to_incr_scale: 1. / env.sampling_rate,
            pre_filter: AntiAliasingFilter::new(env.sampling_rate),
            post_filter: AntiAliasingFilter::new(env.sampling_rate),
            dc_blocker: DcBlocker::new(env.sampling_rate),
            detector: PeakLevelDetector::new(env.sampling_rate),
        }
    }

    fn process_mono<
        I: Buffer,
        O: BufferMut,
        F: Iterator<Item = f32>,
        R: Iterator<Item = f32>,
        M: Iterator<Item = f32> + Clone,
    >(
        &mut self,
        input: &I,
        output: &mut O,
        forward: F,
        reverse: R,
        mix: M,
    ) {
        let delay_buffer = self.delay_l.process(
            self.post_filter.process(
                self.pre_filter
                    .process(input.channel(0).iter().copied())
                    .map(|x| {
                        let detected_level = self.detector.detect_level(x);
                        self.dc_blocker.process(expand(
                            nonlinearity(compress(x, detected_level)),
                            detected_level,
                        ))
                    }),
            ),
        );
        util::iter::move_into(
            izip!(
                input.channel(0),
                delay_buffer.process(forward),
                delay_buffer.process(reverse),
                mix
            )
            .map(|(i, l, r, m)| i + (l + r) * m * PERCENT_SCALE),
            output.channel_mut(0),
        );
    }

    fn process_stereo<
        I: Buffer,
        O: BufferMut,
        F: Iterator<Item = f32>,
        R: Iterator<Item = f32>,
        M: Iterator<Item = f32> + Clone,
    >(
        &mut self,
        input: &I,
        output: &mut O,
        forward: F,
        reverse: R,
        mix: M,
    ) {
        // Note that we re-do the mixing for each channel - it might
        // be better to just do this once and store the result in a temp buffer?
        let mixed = izip!(input.channel(0), input.channel(1)).map(|(l, r)| (l + r) * 0.5);
        let delay_buffer =
            self.delay_l
                .process(
                    self.post_filter
                        .process(self.pre_filter.process(mixed).map(|x| {
                            let detected_level = self.detector.detect_level(x);
                            self.dc_blocker.process(expand(
                                nonlinearity(compress(x, detected_level)),
                                detected_level,
                            ))
                        })),
                );

        util::iter::move_into(
            izip!(input.channel(0), delay_buffer.process(forward), mix.clone())
                .map(|(i, l, m)| i + l * m * PERCENT_SCALE),
            output.channel_mut(0),
        );
        util::iter::move_into(
            izip!(input.channel(1), delay_buffer.process(reverse), mix)
                .map(|(i, r, m)| i + r * m * PERCENT_SCALE),
            output.channel_mut(1),
        );
    }
}

impl EffectT for Effect {
    fn handle_parameters<P: parameters::States>(&mut self, _: P) {}

    fn process<P: BufferStates, I: Buffer, O: BufferMut>(
        &mut self,
        parameters: P,
        input: &I,
        output: &mut O,
    ) {
        debug_assert_eq!(input.channel_layout(), output.channel_layout());
        debug_assert_eq!(input.num_frames(), output.num_frames());
        let rate_to_incr_scale = self.rate_to_incr_scale;
        let lfo::Buffer { forward, reverse } = self.lfo.run(
            pzip!(parameters[numeric "rate", numeric "depth"])
                .take(input.num_frames())
                .map(move |(rate, depth)| lfo::Parameters {
                    incr: rate * rate_to_incr_scale,
                    depth,
                }),
        );
        let mix =
            pzip!(parameters[numeric "mix", switch "bypass"]).map(
                |(mix, bypass)| {
                    if bypass {
                        0.0
                    } else {
                        mix
                    }
                },
            );
        match input.channel_layout() {
            ChannelLayout::Mono => self.process_mono(input, output, forward, reverse, mix),
            ChannelLayout::Stereo => self.process_stereo(input, output, forward, reverse, mix),
        }
    }
}

#[cfg(test)]
mod tests;
