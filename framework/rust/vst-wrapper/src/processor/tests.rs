use std::cell::RefCell;
use std::collections::HashSet;

use component::effect::Effect;
use vst3::ComWrapper;
use vst3::Steinberg::{
    IBStreamTrait, IPluginBaseTrait,
    Vst::{IAudioProcessorTrait, IComponentTrait, IHostApplication},
};

use super::test_utils::{activate_busses, process_setup, setup_proc, DEFAULT_ENV};
use super::{create_effect, create_synth, PartialProcessingEnvironment};
use crate::fake_ibstream::Stream;
use crate::processor::test_utils::{
    activate_effect_busses, mock_no_audio_process_data, mock_process, mock_process_effect,
    mock_process_mod, setup_proc_effect, ParameterValueQueueImpl, ParameterValueQueuePoint,
    SAMPLE_COUNT,
};
use crate::HostInfo;
use crate::{dummy_host, from_utf16_buffer};
use assert_approx_eq::assert_approx_eq;
use component;
use component::audio::{channels, channels_mut, BufferMut};
use component::events::{Data, Event, Events, NoteData, NoteID};
use component::parameters::utils::{enum_per_sample, numeric_per_sample, switch_per_sample};
use component::parameters::{
    BufferStates, Flags, InfoRef, States, StaticInfoRef, TypeSpecificInfoRef,
};
use component::{synth::Synth, Component, ProcessingEnvironment, ProcessingMode, Processor};

#[derive(Default)]
struct FakeSynthComponent<'a> {
    last_process_env: Option<&'a RefCell<Option<ProcessingEnvironment>>>,
    processing: Option<&'a RefCell<bool>>,
}

struct FakeSynth<'a> {
    processing: Option<&'a RefCell<bool>>,
    notes: HashSet<NoteID>,
}

struct FakeEffect {}

#[derive(Default)]
struct FakeEffectComponent {}

static DEFAULT_NUMERIC: f32 = 1.0;
static MIN_NUMERIC: f32 = 0.5;
static MAX_NUMERIC: f32 = 10.0;
static DEFAULT_ENUM: u32 = 0;
static DEFAULT_SWITCH: bool = true;

static NUMERIC_ID: &str = "mult";
static ENUM_ID: &str = "enum";
static SWITCH_ID: &str = "switch";

static PARAMETERS: [StaticInfoRef; 3] = [
    InfoRef {
        title: "Multiplier",
        short_title: "Mult",
        unique_id: NUMERIC_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=MAX_NUMERIC,
            units: "Hz",
        },
    },
    InfoRef {
        title: "Enum Multiplier",
        short_title: "Enum",
        unique_id: ENUM_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Enum {
            default: DEFAULT_ENUM,
            values: &["1", "2", "3"],
        },
    },
    InfoRef {
        title: "Switch Multipler",
        short_title: "Switch",
        unique_id: SWITCH_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch {
            default: DEFAULT_SWITCH,
        },
    },
];

impl<'a> Processor for FakeSynth<'a> {
    fn set_processing(&mut self, processing: bool) {
        if let Some(processing_) = self.processing {
            processing_.replace(processing);
        }
    }
}

impl<'a> Synth for FakeSynth<'a> {
    fn handle_events<E: IntoIterator<Item = Data>, P: States>(
        &mut self,
        events: E,
        _parameters: P,
    ) {
        for event in events {
            match event {
                Data::NoteOn { data } => {
                    self.notes.insert(data.id);
                }
                Data::NoteOff { data } => {
                    self.notes.remove(&data.id);
                }
            }
        }
    }

    fn process<E: IntoIterator<Item = Event>, P: BufferStates, O: BufferMut>(
        &mut self,
        events: Events<E>,
        parameters: P,
        output: &mut O,
    ) {
        let mut events_iter = events.into_iter();
        let mut next_event = events_iter.next();

        let mult_iter = numeric_per_sample(parameters.get_numeric(NUMERIC_ID).unwrap());
        let enum_iter = enum_per_sample(parameters.get_enum(ENUM_ID).unwrap());
        let switch_iter = switch_per_sample(parameters.get_switch(SWITCH_ID).unwrap());

        for (((frame_index, mult), enum_mult), switch_mult) in (0..output.num_frames())
            .zip(mult_iter)
            .zip(enum_iter)
            .zip(switch_iter)
        {
            while let Some(event) = &next_event {
                if event.sample_offset == frame_index {
                    match event.data {
                        Data::NoteOn { ref data } => {
                            self.notes.insert(data.id);
                        }
                        Data::NoteOff { ref data } => {
                            self.notes.remove(&data.id);
                        }
                    }
                    next_event = events_iter.next();
                } else {
                    break;
                }
            }
            for channel in channels_mut(output) {
                channel[frame_index] = mult
                    * ((enum_mult + 1) as f32)
                    * (if switch_mult { 1.0 } else { 0.0 })
                    * self.notes.len() as f32;
            }
        }
    }
}

impl<'a> Component for FakeSynthComponent<'a> {
    type Processor = FakeSynth<'a>;

    fn create_processor(&self, env: &ProcessingEnvironment) -> Self::Processor {
        if let Some(proc_env) = self.last_process_env {
            proc_env.replace(Some(env.clone()));
        }
        let mut notes = HashSet::new();
        notes.reserve(1024);
        FakeSynth {
            processing: self.processing,
            notes,
        }
    }

    fn parameter_infos(&self) -> Vec<component::parameters::Info> {
        component::parameters::to_infos(&PARAMETERS)
    }
}

fn dummy_synth() -> impl IComponentTrait + IAudioProcessorTrait {
    create_synth(
        |_: &HostInfo| -> FakeSynthComponent<'static> { Default::default() },
        [4; 16],
    )
}

fn dummy_synth_with_processing_environment<'a>(
    env: &'a RefCell<Option<ProcessingEnvironment>>,
) -> impl IAudioProcessorTrait + IComponentTrait + 'a {
    create_synth(
        |_: &HostInfo| FakeSynthComponent {
            last_process_env: Some(env),
            processing: None,
        },
        [4; 16],
    )
}

fn dummy_synth_with_host_info<'a>(
    host_info: &'a RefCell<Option<HostInfo>>,
) -> impl IAudioProcessorTrait + IComponentTrait + 'a {
    create_synth(
        |real_host_info: &HostInfo| {
            host_info.replace(Some((*real_host_info).clone()));
            FakeSynthComponent {
                last_process_env: None,
                processing: None,
            }
        },
        [4; 16],
    )
}

fn dummy_synth_with_processing<'a>(
    env: &'a RefCell<bool>,
) -> impl IAudioProcessorTrait + IComponentTrait + 'a {
    create_synth(
        |_: &HostInfo| FakeSynthComponent {
            last_process_env: None,
            processing: Some(env),
        },
        [4; 16],
    )
}

impl Processor for FakeEffect {
    fn set_processing(&mut self, _processing: bool) {}
}

impl Effect for FakeEffect {
    fn handle_parameters<P: component::parameters::States>(&mut self, _parameters: P) {}

    fn process<
        P: component::parameters::BufferStates,
        I: component::audio::Buffer,
        O: component::audio::BufferMut,
    >(
        &mut self,
        parameters: P,
        input: &I,
        output: &mut O,
    ) {
        let mult_iter = numeric_per_sample(parameters.get_numeric(NUMERIC_ID).unwrap());
        let enum_iter = enum_per_sample(parameters.get_enum(ENUM_ID).unwrap());
        let switch_iter = switch_per_sample(parameters.get_switch(SWITCH_ID).unwrap());

        for (((frame_index, mult), enum_mult), switch_mult) in (0..output.num_frames())
            .zip(mult_iter)
            .zip(enum_iter)
            .zip(switch_iter)
        {
            for (ichannel, ochannel) in channels(input).zip(channels_mut(output)) {
                ochannel[frame_index] = mult
                    * ((enum_mult + 1) as f32)
                    * (if switch_mult { 1.0 } else { 0.0 })
                    * ichannel[frame_index];
            }
        }
    }
}

impl Component for FakeEffectComponent {
    type Processor = FakeEffect;

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        FakeEffect {}
    }

    fn parameter_infos(&self) -> Vec<component::parameters::Info> {
        component::parameters::to_infos(&PARAMETERS)
    }
}

fn dummy_effect() -> impl IComponentTrait + IAudioProcessorTrait {
    create_effect(
        |_: &HostInfo| -> FakeEffectComponent { Default::default() },
        [4; 16],
    )
}

#[test]
fn can_process_f32() {
    let proc = dummy_synth();

    unsafe {
        assert_eq!(
            proc.canProcessSampleSize(vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32),
            vst3::Steinberg::kResultTrue
        );
    }
}

#[test]
fn cannot_process_f64() {
    let proc = dummy_synth();

    unsafe {
        assert_eq!(
            proc.canProcessSampleSize(vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample64 as i32),
            vst3::Steinberg::kResultFalse
        );
    }
}

fn matches(partial: &PartialProcessingEnvironment, full: &ProcessingEnvironment) -> bool {
    partial.sampling_rate == full.sampling_rate
        && partial.max_samples_per_process_call == full.max_samples_per_process_call
        && partial.processing_mode == full.processing_mode
}

#[test]
fn defends_against_setup_processing_before_initialize() {
    let proc = dummy_synth();
    unsafe {
        assert_ne!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn correct_bus_count() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            0
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            1
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            1
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            0
        );
    }
}

#[test]
fn bus_info_invalid_bus() {
    let proc = dummy_synth();

    for &(type_, dir, ind) in &[
        (
            vst3::Steinberg::Vst::MediaTypes_::kAudio,
            vst3::Steinberg::Vst::BusDirections_::kInput,
            0,
        ),
        (
            vst3::Steinberg::Vst::MediaTypes_::kEvent,
            vst3::Steinberg::Vst::BusDirections_::kOutput,
            0,
        ),
        (
            vst3::Steinberg::Vst::MediaTypes_::kEvent,
            vst3::Steinberg::Vst::BusDirections_::kInput,
            1,
        ),
        (
            vst3::Steinberg::Vst::MediaTypes_::kAudio,
            vst3::Steinberg::Vst::BusDirections_::kOutput,
            1,
        ),
    ] {
        unsafe {
            let mut bus = vst3::Steinberg::Vst::BusInfo {
                mediaType: 0,
                direction: 0,
                channelCount: 0,
                name: [0; 128],
                busType: 0,
                flags: 0,
            };
            assert_eq!(
                proc.getBusInfo(type_ as i32, dir as i32, ind, &mut bus),
                vst3::Steinberg::kInvalidArgument
            );
        }
    }
}

#[test]
fn bus_info() {
    let proc = dummy_synth();

    unsafe {
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            1
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            0
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            0
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            1
        );

        let mut bus = vst3::Steinberg::Vst::BusInfo {
            mediaType: 0,
            direction: 0,
            channelCount: 0,
            name: [0; 128],
            busType: 0,
            flags: 0,
        };
        assert_eq!(
            proc.getBusInfo(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut bus
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            bus.mediaType,
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32
        );
        assert_eq!(
            bus.direction,
            vst3::Steinberg::Vst::BusDirections_::kOutput as i32
        );
        // Note we are locked into stereo for now
        assert_eq!(bus.channelCount, 2);
        assert_eq!(
            from_utf16_buffer(&bus.name)
                .as_ref()
                .map(|x: &String| x.as_str()),
            Some("Output")
        );
        assert_eq!(
            bus.flags,
            vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive
        );
        assert_eq!(
            proc.getBusInfo(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32,
                0,
                &mut bus
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            bus.mediaType,
            vst3::Steinberg::Vst::MediaTypes_::kEvent as i32
        );
        assert_eq!(
            bus.direction,
            vst3::Steinberg::Vst::BusDirections_::kInput as i32
        );
        assert_eq!(
            from_utf16_buffer(&bus.name)
                .as_ref()
                .map(|x: &String| x.as_str()),
            Some("Event In")
        );
        assert_eq!(
            bus.flags,
            vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive
        );
    }
}

#[test]
fn bus_info_effect() {
    let proc = dummy_effect();

    unsafe {
        let mut bus = vst3::Steinberg::Vst::BusInfo {
            mediaType: 0,
            direction: 0,
            channelCount: 0,
            name: [0; 128],
            busType: 0,
            flags: 0,
        };
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            1
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32
            ),
            0
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            1
        );
        assert_eq!(
            proc.getBusCount(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32
            ),
            0
        );

        assert_eq!(
            proc.getBusInfo(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut bus
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            bus.mediaType,
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32
        );
        assert_eq!(
            bus.direction,
            vst3::Steinberg::Vst::BusDirections_::kOutput as i32
        );
        // Note we are locked into stereo for now
        assert_eq!(bus.channelCount, 2);
        assert_eq!(
            from_utf16_buffer(&bus.name)
                .as_ref()
                .map(|x: &String| x.as_str()),
            Some("Output")
        );
        assert_eq!(
            bus.flags,
            vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive
        );
        assert_eq!(
            proc.getBusInfo(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32,
                0,
                &mut bus
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            bus.mediaType,
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32
        );
        assert_eq!(
            bus.direction,
            vst3::Steinberg::Vst::BusDirections_::kInput as i32
        );
        assert_eq!(
            from_utf16_buffer(&bus.name)
                .as_ref()
                .map(|x: &String| x.as_str()),
            Some("Input")
        );
        assert_eq!(
            bus.flags,
            vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive
        );
    }
}

#[test]
fn initialize_gets_host_info() {
    let host_info = Default::default();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    let proc = dummy_synth_with_host_info(&host_info);
    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
    assert_eq!(
        host_info.borrow().as_ref(),
        Some(&HostInfo {
            name: "Dummy Host".to_string()
        })
    );
    unsafe { assert_eq!(proc.terminate(), vst3::Steinberg::kResultOk) }
}

#[test]
fn defends_against_termination_before_initialization() {
    let proc = dummy_synth();
    unsafe { assert_ne!(proc.terminate(), vst3::Steinberg::kResultOk) }
}

#[test]
fn defends_against_initialize_twice() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
    unsafe {
        assert_ne!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
}

#[test]
fn allow_initialize_twice_with_terminate() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
    unsafe { assert_eq!(proc.terminate(), vst3::Steinberg::kResultOk) }
    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
}

#[test]
fn defends_against_activating_wrong_bus() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            proc.activateBus(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kInput as i32,
                0,
                1
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_effect_activating_wrong_bus() {
    let proc = dummy_effect();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            proc.activateBus(
                vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                1
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn captures_process_environment() {
    let env = Default::default();
    let proc = dummy_synth_with_processing_environment(&env);

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        activate_busses(&proc);
    }

    for test_env in [
        PartialProcessingEnvironment {
            sampling_rate: 48000.0,
            max_samples_per_process_call: 512,
            processing_mode: ProcessingMode::Realtime,
        },
        PartialProcessingEnvironment {
            sampling_rate: 44100.0,
            max_samples_per_process_call: 8192,
            processing_mode: ProcessingMode::Offline,
        },
        PartialProcessingEnvironment {
            sampling_rate: 96000.0,
            max_samples_per_process_call: 2048,
            processing_mode: ProcessingMode::Prefetch,
        },
    ] {
        unsafe {
            assert_eq!(
                proc.setupProcessing(&mut process_setup(&test_env)),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
            assert!(matches(&test_env, env.borrow().as_ref().unwrap()));
            assert_eq!(proc.setActive(0u8), vst3::Steinberg::kResultOk);
        }
    }
}

#[test]
fn defends_against_activating_without_environment() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_ne!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_activating_without_activating_busses() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_agsinst_effect_activating_without_activating_busses() {
    let proc = dummy_effect();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_changing_processing_environment_while_active() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        assert_ne!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 44100.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_activating_without_initialization() {
    let proc = dummy_synth();

    unsafe {
        assert_ne!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_activating_bus_while_active() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        assert_ne!(
            proc.activateBus(
                vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                0
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_bad_input_count_for_bus_arrangement() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        let mut in_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        assert_ne!(
            proc.setBusArrangements(&mut in_arrangement, 1, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                1,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kInput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_get_set_bus_before_init() {
    let proc = dummy_synth();
    unsafe {
        let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        assert_ne!(
            proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_set_bus_after_active() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);

        assert_ne!(
            proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );
        // getBusArrangement should still work after setActive.
        assert_eq!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn set_get_bus() {
    let proc = dummy_synth();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);

        assert_eq!(
            proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );

        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(out_arrangement, vst3::Steinberg::Vst::SpeakerArr::kMono);

        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );
        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(out_arrangement, vst3::Steinberg::Vst::SpeakerArr::kStereo);
    }
}

#[test]
fn set_get_bus_effect() {
    let proc = dummy_effect();

    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut in_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;
        let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kMono;

        assert_eq!(
            proc.setupProcessing(&mut process_setup(&PartialProcessingEnvironment {
                sampling_rate: 48000.0,
                max_samples_per_process_call: 512,
                processing_mode: ProcessingMode::Realtime,
            })),
            vst3::Steinberg::kResultOk
        );
        activate_effect_busses(&proc);

        assert_ne!(
            proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setBusArrangements(&mut in_arrangement, 1, &mut out_arrangement, 1),
            vst3::Steinberg::kResultTrue
        );

        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(out_arrangement, vst3::Steinberg::Vst::SpeakerArr::kMono);

        in_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_ne!(
            proc.setBusArrangements(&mut in_arrangement, 1, &mut out_arrangement, 1),
            vst3::Steinberg::kResultTrue
        );

        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.setBusArrangements(&mut in_arrangement, 1, &mut out_arrangement, 1),
            vst3::Steinberg::kResultOk
        );
        out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
        assert_eq!(
            proc.getBusArrangement(
                vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
                0,
                &mut out_arrangement
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(out_arrangement, vst3::Steinberg::Vst::SpeakerArr::kStereo);
    }
}

#[test]
fn defends_against_set_processing_before_init() {
    let proc = dummy_synth();
    unsafe {
        assert_ne!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_set_processing_before_setup_processing() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_set_processing_before_activating_busses() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_set_processing_before_activating() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);

        assert_ne!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn can_set_processing() {
    let processing: RefCell<bool> = Default::default();
    let proc = dummy_synth_with_processing(&processing);
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        assert_eq!(*processing.borrow(), false);
        assert_eq!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
        assert_eq!(*processing.borrow(), true);
        assert_eq!(proc.setProcessing(0u8), vst3::Steinberg::kResultOk);
        assert_eq!(*processing.borrow(), false);
    }
}

#[test]
fn defends_against_set_processing_while_inactive() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        assert_eq!(proc.setActive(0u8), vst3::Steinberg::kResultOk);

        assert_ne!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
    }
}

#[test]
fn defends_against_processing_without_set() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            proc.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
            vst3::Steinberg::kResultOk
        );
        activate_busses(&proc);
        assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        assert_ne!(
            proc.process(std::ptr::null_mut()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn can_process() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        let audio = mock_process(
            2,
            vec![
                Event {
                    sample_offset: 0,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
                Event {
                    sample_offset: 100,
                    data: Data::NoteOff {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
                Event {
                    sample_offset: 200,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
                Event {
                    sample_offset: 200,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(1),
                            pitch: 65,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
            ],
            vec![],
            &proc,
        );
        assert!(audio.is_some());
        assert_eq!(audio.as_ref().unwrap()[0][0], 1.0);
        assert_eq!(audio.as_ref().unwrap()[1][100], 0.0);
        assert_eq!(audio.as_ref().unwrap()[1][200], 2.0);
    }
}

#[test]
fn can_process_effect() {
    let proc = dummy_effect();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc_effect(&proc, &host);

        let audio = mock_process_effect(
            vec![vec![1f32; 512]; 2],
            vec![
                ParameterValueQueueImpl {
                    param_id: NUMERIC_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 99,
                            value: ((1.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 100,
                            value: 1.0,
                        },
                    ],
                },
                ParameterValueQueueImpl {
                    param_id: SWITCH_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 500,
                            value: 1.0,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 501,
                            value: 0.0,
                        },
                    ],
                },
                ParameterValueQueueImpl {
                    param_id: ENUM_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 300,
                            value: 0.33,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 301,
                            value: 0.34,
                        },
                    ],
                },
            ],
            &proc,
        );
        assert!(audio.is_some());
        assert_approx_eq!(audio.as_ref().unwrap()[1][100], 10.0);
        assert_approx_eq!(audio.as_ref().unwrap()[1][500], 20.0);
    }
}

#[test]
fn defends_against_events_past_buffer() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        assert!(mock_process(
            2,
            vec![Event {
                sample_offset: SAMPLE_COUNT + 1000,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc
        )
        .is_none());
    }
}

#[test]
fn defends_against_shuffled_events() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        assert!(mock_process(
            2,
            vec![
                Event {
                    sample_offset: 200,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
                Event {
                    sample_offset: 100,
                    data: Data::NoteOff {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
            ],
            vec![],
            &proc
        )
        .is_none());
    }
}

#[test]
fn can_handle_null_events() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        let audio = mock_process_mod(2, vec![], vec![], &proc, |x| {
            x.inputEvents = std::ptr::null_mut();
        });
        assert!(audio.is_some());
        assert_eq!(audio.as_ref().unwrap()[0][0], 0.0);
    }
}

#[test]
fn defends_against_timed_events_without_audio() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        let mut data = mock_no_audio_process_data(
            vec![Event {
                sample_offset: 100,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
        );
        assert_ne!(
            proc.process(&mut data.process_data),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn can_handle_events_without_audio() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        let audio = mock_process(
            2,
            vec![
                Event {
                    sample_offset: 0,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(0),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
                Event {
                    sample_offset: 0,
                    data: Data::NoteOn {
                        data: NoteData {
                            channel: 0,
                            id: NoteID::from_id(1),
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                },
            ],
            vec![],
            &proc,
        );

        assert!(audio.is_some());

        let mut data2 = mock_no_audio_process_data(
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOff {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
        );

        assert_eq!(
            proc.process(&mut data2.process_data),
            vst3::Steinberg::kResultOk
        );

        let audio3 = mock_process(
            2,
            vec![Event {
                sample_offset: 100,
                data: Data::NoteOff {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(1),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc,
        );
        assert!(audio3.is_some());
        assert_eq!(audio3.as_ref().unwrap()[0][0], 1.0);
        assert_eq!(audio3.as_ref().unwrap()[1][100], 0.0);
    }
}

#[test]
fn can_handle_parameter_changes() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![
                ParameterValueQueueImpl {
                    param_id: NUMERIC_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 99,
                            value: ((1.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 100,
                            value: 1.0,
                        },
                    ],
                },
                ParameterValueQueueImpl {
                    param_id: SWITCH_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 500,
                            value: 1.0,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 501,
                            value: 0.0,
                        },
                    ],
                },
                ParameterValueQueueImpl {
                    param_id: ENUM_ID,
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 300,
                            value: 0.33,
                        },
                        ParameterValueQueuePoint {
                            sample_offset: 301,
                            value: 0.34,
                        },
                    ],
                },
            ],
            &proc,
        );

        assert!(audio.is_some());
        let audio = audio.as_ref().unwrap();
        assert_eq!(audio[0][0], 1.0);
        assert_eq!(audio[1][99], 1.0);
        assert_eq!(audio[0][100], MAX_NUMERIC as f32);
        assert_eq!(audio[0][200], MAX_NUMERIC as f32);
        assert_eq!(audio[0][300], MAX_NUMERIC as f32);
        assert_eq!(audio[0][301], (MAX_NUMERIC * 2.0) as f32);
        assert_eq!(audio[0][500], (MAX_NUMERIC * 2.0) as f32);
        assert_eq!(audio[0][501], 0.0);
    }
}

#[test]
fn parameter_changes_at_start_of_buffer() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![
                    ParameterValueQueuePoint {
                        sample_offset: 0,
                        value: ((0.6 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                    },
                    ParameterValueQueuePoint {
                        sample_offset: 100,
                        value: 1.0,
                    },
                ],
            }],
            &proc,
        );

        assert!(audio.is_some());
        let audio = audio.as_ref().unwrap();
        assert_eq!(audio[0][0], 0.6);
        assert_eq!(audio[0][100], MAX_NUMERIC as f32);
        assert_eq!(audio[0][500], MAX_NUMERIC as f32);
    }
}

#[test]
fn defends_against_wild_parameter_ids() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: "some garbage parameter",
                points: vec![ParameterValueQueuePoint {
                    sample_offset: 0,
                    value: MAX_NUMERIC as f64,
                }],
            }],
            &proc,
        );

        assert!(audio.is_none());
    }
}

#[test]
fn empty_queues_are_ignored() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![],
            }],
            &proc,
        );

        assert!(audio.is_some());
        assert_eq!(audio.as_ref().unwrap()[0][100], 1.0);
    }
}

#[test]
fn defends_against_unsorted_param_queues() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![
                    ParameterValueQueuePoint {
                        sample_offset: 100,
                        value: (1.0 - MIN_NUMERIC as f64) / ((MAX_NUMERIC - MIN_NUMERIC) as f64),
                    },
                    ParameterValueQueuePoint {
                        sample_offset: 99,
                        value: 1.0,
                    },
                ],
            }],
            &proc,
        );

        assert!(audio.is_none());
    }
}

#[test]
fn defends_against_doubled_curve_points() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![
                    ParameterValueQueuePoint {
                        sample_offset: 99,
                        value: ((1.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                    },
                    ParameterValueQueuePoint {
                        sample_offset: 99,
                        value: 1.0,
                    },
                ],
            }],
            &proc,
        );

        assert!(audio.is_none());
    }
}

#[test]
fn defends_against_points_outside_buffer() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![ParameterValueQueuePoint {
                    sample_offset: 700,
                    value: 1.0,
                }],
            }],
            &proc,
        );

        assert!(audio.is_none());
    }
}

#[test]
fn defends_against_multiple_queues_for_same_param() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let data = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![
                ParameterValueQueueImpl {
                    param_id: NUMERIC_ID,
                    points: vec![ParameterValueQueuePoint {
                        sample_offset: 99,
                        value: ((1.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                    }],
                },
                ParameterValueQueueImpl {
                    param_id: NUMERIC_ID,
                    points: vec![ParameterValueQueuePoint {
                        sample_offset: 100,
                        value: 1.0,
                    }],
                },
            ],
            &proc,
        );

        assert!(data.is_none());
    }
}

#[test]
fn defends_against_non_zero_time_parameter_change() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let mut data = mock_no_audio_process_data(
            vec![],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![ParameterValueQueuePoint {
                    sample_offset: 1,
                    value: 1.0,
                }],
            }],
        );

        assert_ne!(
            proc.process(&mut data.process_data),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_out_of_range_parameter_values() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        let mut data = mock_no_audio_process_data(
            vec![],
            vec![ParameterValueQueueImpl {
                param_id: NUMERIC_ID,
                points: vec![ParameterValueQueuePoint {
                    sample_offset: 0,
                    value: -5.0,
                }],
            }],
        );

        assert_ne!(
            proc.process(&mut data.process_data),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn can_change_parameters_in_handle_events() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);
        assert_eq!(
            proc.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![
                        ParameterValueQueueImpl {
                            param_id: NUMERIC_ID,
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 1.0,
                            }],
                        },
                        ParameterValueQueueImpl {
                            param_id: ENUM_ID,
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 0.5,
                            }],
                        },
                        ParameterValueQueueImpl {
                            param_id: SWITCH_ID,
                            points: vec![ParameterValueQueuePoint {
                                sample_offset: 0,
                                value: 0.0,
                            }],
                        }
                    ],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );

        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![ParameterValueQueueImpl {
                param_id: SWITCH_ID,
                points: vec![ParameterValueQueuePoint {
                    sample_offset: 100,
                    value: 1.0,
                }],
            }],
            &proc,
        );

        assert!(audio.is_some());
        assert_eq!(audio.as_ref().unwrap()[0][0], 0.0 as f32);
        assert_eq!(audio.as_ref().unwrap()[0][100], 2.0 * MAX_NUMERIC as f32);
    }
}

#[test]
fn defends_against_get_state_before_initialized() {
    let proc = dummy_synth();
    let stream = ComWrapper::new(Stream::new([]))
        .to_com_ptr::<vst3::Steinberg::IBStream>()
        .unwrap();
    let result = unsafe { proc.getState(stream.as_ptr()) };
    assert_ne!(result, vst3::Steinberg::kResultOk);
}

#[test]
fn defends_against_get_state_with_null_stream() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc, &host);
        assert_ne!(
            proc.getState(std::ptr::null_mut()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn get_state_saves_data() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc, &host);
        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert!(stream.data().len() > 0);
    }
}

#[test]
fn defends_against_set_state_before_initialized() {
    let proc = dummy_synth();
    let stream = ComWrapper::new(Stream::new([]))
        .to_com_ptr::<vst3::Steinberg::IBStream>()
        .unwrap();
    let result = unsafe { proc.setState(stream.as_ptr()) };
    assert_ne!(result, vst3::Steinberg::kResultOk);
}

#[test]
fn defends_against_set_state_with_null_stream() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc, &host);
        assert_ne!(
            proc.setState(std::ptr::null_mut()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn set_state_sets_parameters() {
    let proc1 = dummy_synth();
    let proc2 = dummy_synth();
    let proc3 = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc1, &host);
        setup_proc(&proc2, &host);
        setup_proc(&proc3, &host);
        assert_eq!(
            proc1.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: NUMERIC_ID,
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 0,
                            value: 1.0,
                        }],
                    },],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );

        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc1.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );

        // Here we test an instant-readback to make sure we don't have a bug where we
        // only read the correct state after the next process call.
        assert_eq!(
            proc2.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc2.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc3.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );

        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 10,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc3,
        );

        assert!(audio.is_some());

        assert_eq!(audio.as_ref().unwrap()[0][0], 0.0);
        assert_eq!(audio.as_ref().unwrap()[0][10], MAX_NUMERIC);
    }
}

#[test]
fn get_state_sees_automation() {
    let proc1 = dummy_synth();
    let proc2 = dummy_synth();
    let proc3 = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc1, &host);
        setup_proc(&proc2, &host);
        setup_proc(&proc3, &host);
        assert_eq!(
            proc1.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: NUMERIC_ID,
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 0,
                            value: 1.0,
                        }],
                    },],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );

        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc1.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc2.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc2.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: ENUM_ID,
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 0,
                            value: 0.5,
                        }],
                    },],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );

        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc2.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc3.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );

        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 10,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc3,
        );
        assert!(audio.is_some());

        assert_eq!(audio.as_ref().unwrap()[0][0], 0.0);
        assert_eq!(audio.as_ref().unwrap()[0][10], 2.0 * MAX_NUMERIC);
    }
}

#[test]
fn defends_against_invalid_state_data() {
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc, &host);

        // Note - this test makes a (currently valid) assumption that an empty state buffer
        // is not a valid state. If the implementation of processor's serialization changes
        // and this assumption becomes invalid, please replace the empty buffer with an
        // invalid state buffer.
        let invalid_stream = ComWrapper::new(Stream::new([]));
        assert_ne!(
            proc.setState(
                invalid_stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

static INCOMPATIBLE_PARAMETERS: [StaticInfoRef; 1] = [InfoRef {
    title: "Multiplier",
    short_title: "Mult",
    // This is incompatible since the previous version had a
    // parameter of a different type with this ID
    unique_id: ENUM_ID,
    flags: Flags { automatable: true },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: DEFAULT_NUMERIC,
        valid_range: MIN_NUMERIC..=MAX_NUMERIC,
        units: "Hz",
    },
}];

#[derive(Default)]
struct IncompatibleComponent {}

#[derive(Default)]
struct IncompatibleSynth {}

impl Processor for IncompatibleSynth {
    fn set_processing(&mut self, _processing: bool) {}
}

impl Synth for IncompatibleSynth {
    fn handle_events<E: IntoIterator<Item = Data>, P: States>(
        &mut self,
        _events: E,
        _parameters: P,
    ) {
    }

    fn process<E: IntoIterator<Item = Event>, P: BufferStates, O: BufferMut>(
        &mut self,
        _events: Events<E>,
        _parameters: P,
        _output: &mut O,
    ) {
    }
}

impl Component for IncompatibleComponent {
    type Processor = IncompatibleSynth;

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<component::parameters::Info> {
        component::parameters::to_infos(&INCOMPATIBLE_PARAMETERS)
    }
}

#[test]
fn defends_against_load_state_with_incompatible_parameters() {
    let proc1 = dummy_synth();
    let proc2 = create_synth(
        |_: &HostInfo| -> IncompatibleComponent { Default::default() },
        [5; 16],
    );

    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc1, &host);
        setup_proc(&proc2, &host);
        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc1.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            proc2.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
    }
}

#[derive(Default)]
struct NewerComponent {}

#[derive(Default)]
struct NewerSynth {}

impl Processor for NewerSynth {
    fn set_processing(&mut self, _processing: bool) {}
}

impl Synth for NewerSynth {
    fn handle_events<E: IntoIterator<Item = Data>, P: States>(
        &mut self,
        _events: E,
        _parameters: P,
    ) {
    }

    fn process<E: IntoIterator<Item = Event>, P: BufferStates, O: BufferMut>(
        &mut self,
        _events: Events<E>,
        _parameters: P,
        _output: &mut O,
    ) {
    }
}

static NEWER_PARAMETERS: [StaticInfoRef; 3] = [
    InfoRef {
        title: "Multiplier",
        short_title: "Mult",
        unique_id: "mult",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: "Hz",
        },
    },
    InfoRef {
        title: "Enum Multiplier",
        short_title: "Enum",
        unique_id: "enum_mult",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Enum {
            default: DEFAULT_ENUM,
            values: &["1", "2", "3"],
        },
    },
    InfoRef {
        title: "Switch Multipler",
        short_title: "Switch",
        unique_id: "switch",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch {
            default: DEFAULT_SWITCH,
        },
    },
];

impl Component for NewerComponent {
    type Processor = NewerSynth;

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<component::parameters::Info> {
        component::parameters::to_infos(&NEWER_PARAMETERS)
    }
}

#[test]
fn loading_too_new_parameters_loads_default_state() {
    let proc1 = dummy_synth();
    let proc2 = create_synth(
        |_: &HostInfo| -> NewerComponent { Default::default() },
        [5; 16],
    );
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        setup_proc(&proc1, &host);
        setup_proc(&proc2, &host);
        assert_eq!(
            proc1.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: NUMERIC_ID,
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 0,
                            value: 1.0, // Set it to the old max of 10!
                        }],
                    },],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc2.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: NUMERIC_ID,
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 0,
                            value: 1.0, // Set it to the new max of 20!
                        }],
                    },],
                )
                .process_data
            ),
            vst3::Steinberg::kResultOk
        );

        let stream = ComWrapper::new(Stream::new([]));
        assert_eq!(
            proc2.getState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            proc1.setState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr()
            ),
            vst3::Steinberg::kResultOk
        );

        // Now, since proc2 was a newer version than proc1, we expect proc1 to be re-set
        // to its default state.
        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 10,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc1,
        );
        assert!(audio.is_some());

        assert_eq!(audio.as_ref().unwrap()[0][10], 1.0);
    }
}

static DUPLICATE_PARAMETERS: [StaticInfoRef; 2] = [
    InfoRef {
        title: "Multiplier",
        short_title: "Mult",
        unique_id: "mult",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: "Hz",
        },
    },
    InfoRef {
        title: "Multiplier",
        short_title: "Mult",
        unique_id: "mult",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: "Hz",
        },
    },
];

#[derive(Default)]
struct DuplicateParameterComponent {}

impl Component for DuplicateParameterComponent {
    type Processor = IncompatibleSynth;

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<component::parameters::Info> {
        component::parameters::to_infos(&DUPLICATE_PARAMETERS)
    }
}

#[test]
#[should_panic]
fn panic_on_duplicate_ids() {
    let processor = create_synth(
        |_: &HostInfo| -> DuplicateParameterComponent { Default::default() },
        [5; 16],
    );
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        processor.initialize(host.as_com_ref().unwrap().as_ptr());
    }
}

#[test]
fn supports_toggling_active_while_processing() {
    // Note that this call pattern is _explicitly disallowed_ by the spec,
    // but we support it anyway since some DAWs do it (tested Ableton 11.3.20).
    let proc = dummy_synth();
    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        setup_proc(&proc, &host);

        proc.setActive(0);
        proc.setActive(1);

        let audio = mock_process(
            2,
            vec![Event {
                sample_offset: 100,
                data: Data::NoteOn {
                    data: NoteData {
                        channel: 0,
                        id: NoteID::from_id(0),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                },
            }],
            vec![],
            &proc,
        );
        assert!(audio.is_some());
        assert_eq!(audio.as_ref().unwrap()[0][0], 0.0);
        assert_eq!(audio.as_ref().unwrap()[1][100], 1.0);
    }
}
