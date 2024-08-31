use std::collections::HashMap;

use super::{voice, Synth};
use crate::PARAMETERS;
use assert_approx_eq::assert_approx_eq;
use conformal_component::{
    audio::{util::slice_buffer_mut, Buffer as _, BufferData, ChannelLayout},
    events::{Data, Event, Events, NoteData, NoteID},
    parameters::{
        test_utils::{override_synth_defaults, ConstantBufferStates, StatesMap},
        InternalValue,
    },
    synth::Synth as _,
    ProcessingEnvironment, ProcessingMode, Processor as _,
};
use snapshots::assert_snapshot;

fn dummy_params_map() -> StatesMap {
    StatesMap::from(override_synth_defaults(
        PARAMETERS.iter().cloned(),
        &HashMap::from_iter([
            ("dco1_width", InternalValue::Numeric(25.0)),
            (
                "dco2_shape",
                InternalValue::Enum(voice::Dco2Shape::Saw as u32),
            ),
            ("vcf_cutoff", InternalValue::Numeric(0.0)),
            ("vcf_resonance", InternalValue::Numeric(14.2)),
            ("vcf_tracking", InternalValue::Numeric(0.0)),
            ("vcf_env", InternalValue::Numeric(100.0)),
            ("attack", InternalValue::Numeric(0.010)),
            ("decay", InternalValue::Numeric(0.1)),
            ("sustain", InternalValue::Numeric(80.0)),
            ("release", InternalValue::Numeric(0.2)),
            ("vca_level", InternalValue::Numeric(100.0)),
        ]),
    ))
}

fn dummy_params() -> ConstantBufferStates<StatesMap> {
    ConstantBufferStates::new(dummy_params_map())
}

fn generate_snapshot_with_params(
    synth: &mut Synth,
    num_samples: usize,
    params: ConstantBufferStates<StatesMap>,
) -> Vec<f32> {
    let mut output = BufferData::new(ChannelLayout::Mono, num_samples);
    let events = vec![
        Event {
            sample_offset: 0,
            data: Data::NoteOn {
                data: NoteData {
                    channel: 0,
                    id: NoteID::from_pitch(60),
                    pitch: 60,
                    velocity: 1.0,
                    tuning: 0.0,
                },
            },
        },
        Event {
            sample_offset: (num_samples as f32 * 0.8) as usize,
            data: Data::NoteOff {
                data: NoteData {
                    channel: 0,
                    id: NoteID::from_pitch(60),
                    pitch: 60,
                    velocity: 1.0,
                    tuning: 0.0,
                },
            },
        },
    ];

    synth.process(
        Events::new(events.iter().cloned(), num_samples).unwrap(),
        params,
        &mut output,
    );
    output.channel(0).iter().cloned().collect()
}

fn generate_snapshot(synth: &mut Synth, num_samples: usize) -> Vec<f32> {
    generate_snapshot_with_params(synth, num_samples, dummy_params())
}

#[test]
fn reset() {
    let mut synth = Synth::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 100,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    synth.set_processing(true);
    let initial = generate_snapshot(&mut synth, 100);
    synth.set_processing(false);
    synth.set_processing(true);
    let reset = generate_snapshot(&mut synth, 100);
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot() {
    let mut synth = Synth::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    synth.set_processing(true);
    assert_snapshot!("basic", 48000, generate_snapshot(&mut synth, 48000));
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot_pwm() {
    let mut synth = Synth::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    synth.set_processing(true);
    assert_snapshot!(
        "pwm",
        48000,
        generate_snapshot_with_params(
            &mut synth,
            48000,
            ConstantBufferStates::new(StatesMap::from(override_synth_defaults(
                PARAMETERS.iter().cloned(),
                &HashMap::from_iter([
                    (
                        "dco1_shape",
                        InternalValue::Enum(voice::Dco1Shape::Pwm as u32),
                    ),
                    ("dco1_width", InternalValue::Numeric(90.0)),
                    ("vcf_cutoff", InternalValue::Numeric(0.0)),
                    ("vcf_resonance", InternalValue::Numeric(14.2)),
                    ("vcf_tracking", InternalValue::Numeric(0.0)),
                    ("vcf_env", InternalValue::Numeric(100.0)),
                    ("attack", InternalValue::Numeric(0.010)),
                    ("decay", InternalValue::Numeric(0.1)),
                    ("sustain", InternalValue::Numeric(80.0)),
                    ("release", InternalValue::Numeric(0.2)),
                    ("vca_level", InternalValue::Numeric(100.0)),
                    ("mg_rate", InternalValue::Numeric(75.0)),
                    ("mg_delay", InternalValue::Numeric(0.8)),
                ])
            )))
        )
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot_defaults() {
    let mut synth = Synth::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    synth.set_processing(true);
    assert_snapshot!(
        "defaults",
        48000,
        generate_snapshot_with_params(
            &mut synth,
            48000,
            ConstantBufferStates::new(StatesMap::from(override_synth_defaults(
                PARAMETERS.iter().cloned(),
                &HashMap::new()
            )))
        )
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot_separate_events() {
    let mut synth = Synth::new(&ProcessingEnvironment {
        sampling_rate: 48000.0,
        max_samples_per_process_call: 48000,
        channel_layout: ChannelLayout::Mono,
        processing_mode: ProcessingMode::Realtime,
    });
    let num_samples = 48000;
    let note_on_samples = (num_samples as f32 * 0.8) as usize;

    println!("{num_samples} {note_on_samples}");
    let mut output = BufferData::new(ChannelLayout::Mono, num_samples);

    synth.set_processing(true);
    synth.handle_events(
        vec![Data::NoteOn {
            data: NoteData {
                channel: 0,
                id: NoteID::from_pitch(60),
                pitch: 60,
                velocity: 1.0,
                tuning: 0.0,
            },
        }],
        dummy_params_map(),
    );
    synth.process(
        Events::new([], note_on_samples).unwrap(),
        dummy_params(),
        &mut slice_buffer_mut(&mut output, ..note_on_samples),
    );
    synth.handle_events(
        vec![Data::NoteOff {
            data: NoteData {
                channel: 0,
                id: NoteID::from_pitch(60),
                pitch: 60,
                velocity: 1.0,
                tuning: 0.0,
            },
        }],
        dummy_params_map(),
    );

    synth.process(
        Events::new([], num_samples - note_on_samples).unwrap(),
        dummy_params(),
        &mut slice_buffer_mut(&mut output, note_on_samples..),
    );

    assert_snapshot!(
        "separate_events",
        48000,
        output.channel(0).iter().cloned().collect::<Vec<_>>()
    );
}
