use std::collections::HashMap;

use crate::PARAMETERS;

use super::*;
use assert_approx_eq::assert_approx_eq;
use component::{
    audio::BufferData,
    parameters::test_utils::{override_defaults, ConstantBufferStates, StatesMap},
    ProcessingMode,
};
use snapshots::assert_snapshot;

fn params_map() -> StatesMap {
    StatesMap::from(override_defaults(
        PARAMETERS.iter().cloned(),
        &HashMap::from_iter([]),
    ))
}

fn params() -> ConstantBufferStates<StatesMap> {
    ConstantBufferStates::new(params_map())
}

fn generate_snapshot_with_params(
    effect: &mut Effect,
    input: &[f32],
    params: ConstantBufferStates<StatesMap>,
) -> Vec<f32> {
    let mut input_data = BufferData::new(ChannelLayout::Mono, input.len());
    util::iter::move_into(input.iter().copied(), input_data.channel_mut(0));
    let mut output = BufferData::new(ChannelLayout::Mono, input.len());
    effect.process(params, &input_data, &mut output);
    output.channel(0).iter().cloned().collect()
}

fn generate_snapshot(effect: &mut Effect, input: &[f32]) -> Vec<f32> {
    generate_snapshot_with_params(effect, input, params())
}

#[test]
fn reset() {
    let mut effect = Effect::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 100,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    let test_sig = util::test_utils::sine(25, 440. / 48000.);
    effect.set_processing(true);
    let initial = generate_snapshot(&mut effect, &test_sig);
    effect.set_processing(false);
    effect.set_processing(true);
    let reset = generate_snapshot(&mut effect, &test_sig);
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot_sine() {
    let mut effect = Effect::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    let test_sig: Vec<_> = util::test_utils::sine(48000, 440. / 48000.)
        .iter()
        .map(|x| x * 1. / 3.)
        .collect();
    effect.set_processing(true);
    assert_snapshot!("sine", 48000, generate_snapshot(&mut effect, &test_sig));
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot_sweep() {
    let mut effect = Effect::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    let test_sig: Vec<_> = util::test_utils::linear_sine_sweep(48000, 48000., 10., 20000.)
        .iter()
        .map(|x| x * 1. / 4.)
        .collect();
    effect.set_processing(true);
    assert_snapshot!("sweep", 48000, generate_snapshot(&mut effect, &test_sig));
}
