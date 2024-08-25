use super::*;

use assert_approx_eq::assert_approx_eq;
use snapshots::assert_snapshot;
use util::test_utils::{estimate_tuning, linear_sine_sweep, sine};

#[test]
#[cfg_attr(miri, ignore)]
pub fn fractional_offset_does_not_change_tuning() {
    let mut variable_delay = ModulatedDelay::new(Options {
        lookaround: 1,
        max_delay: 3,
        max_samples_per_process_call: 4096,
    });
    let incr = 440f32 / 44100f32;
    let input = sine(4096 + 3, incr);
    let delay: Vec<_> = std::iter::repeat(2.5f32).take(3).collect();
    variable_delay
        .process(input[..3].iter().cloned())
        .process(delay.iter().cloned())
        .for_each(drop);
    let delay: Vec<_> = std::iter::repeat(2.5f32).take(4096).collect();
    let estimated_incr = estimate_tuning(
        variable_delay
            .process(input[3..].iter().cloned())
            .process(delay.iter().cloned())
            .collect::<Vec<_>>()
            .as_mut_slice(),
    );
    assert_approx_eq!(incr, estimated_incr, 1e-4);
}

fn process_vibrato(sampling_rate: i32, input: &[f32]) -> Vec<f32> {
    let num_samples = input.len();
    let max_delay_ms = 15f32;
    let min_delay_ms = 5f32;
    let max_delay_samples = (max_delay_ms * sampling_rate as f32 / 1000f32).ceil() as usize;
    let lookaround = 8 as u16;
    let min_delay_samples = (min_delay_ms * sampling_rate as f32 / 1000f32).floor() as usize;
    assert!(min_delay_samples >= lookaround as usize);
    let mut variable_delay = ModulatedDelay::new(Options {
        lookaround,
        max_delay: max_delay_samples,
        max_samples_per_process_call: 512,
    });
    let input = input.iter().map(|x| x * 0.5).collect::<Vec<_>>();
    let vibrato_input = sine(num_samples, 5f32 / sampling_rate as f32)
        .iter()
        .map(|x| {
            let delay_ms =
                x * 0.5 * (max_delay_ms - min_delay_ms) + (max_delay_ms + min_delay_ms) / 2f32;
            delay_ms * sampling_rate as f32 / 1000f32
        })
        .collect::<Vec<_>>();

    let mut output = vec![0f32; num_samples];
    let mut head = 0;
    while head < num_samples {
        let next_head = (head + 512).min(num_samples);
        let block = head..next_head;
        for (src, dest) in variable_delay
            .process(input[block.clone()].iter().cloned())
            .process(vibrato_input[block.clone()].iter().cloned())
            .zip(&mut output[block.clone()])
        {
            *dest = src;
        }
        head = next_head;
    }
    output
}

#[test]
#[cfg_attr(miri, ignore)]
fn vibrato_sine_snapshot() {
    let sampling_rate = 48000;

    assert_snapshot!(
        "vibrato_sine",
        sampling_rate,
        process_vibrato(sampling_rate, &sine(48000, 440f32 / sampling_rate as f32))
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn vibrato_sweep_snapshot() {
    let sampling_rate = 48000;

    assert_snapshot!(
        "vibrato_sweep",
        sampling_rate,
        process_vibrato(
            sampling_rate,
            &linear_sine_sweep(48000, sampling_rate as f32, 10f32, 20000f32)
        )
    );
}
