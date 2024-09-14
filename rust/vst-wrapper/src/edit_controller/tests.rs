use std::cell::RefCell;
use std::rc;

use vst3::Class;
use vst3::Steinberg::Vst::{
    IAudioProcessorTrait, IComponentHandler, IComponentHandlerTrait, IComponentTrait,
    IHostApplication, IMidiMappingTrait,
};
use vst3::Steinberg::{IBStreamTrait, IPluginBaseTrait};
use vst3::{ComWrapper, Steinberg::Vst::IEditControllerTrait};

use crate::fake_ibstream::Stream;
use crate::processor::test_utils::{
    mock_no_audio_process_data, setup_proc, ParameterValueQueueImpl, ParameterValueQueuePoint,
};
use crate::HostInfo;
use crate::{dummy_host, from_utf16_buffer, to_utf16};
use crate::{processor, ExtraParameters, ParameterModel};
use conformal_component::audio::BufferMut;
use conformal_component::events::{Data, Event, Events};
use conformal_component::parameters::{self, hash_id, BufferStates, Flags, States, StaticInfoRef};
use conformal_component::{
    parameters::{InfoRef, TypeSpecificInfoRef},
    synth::Synth,
    Component, ProcessingEnvironment, Processor,
};
use conformal_core::parameters::store;
use conformal_core::parameters::store::Store;

use super::GetStore;

#[derive(Default)]
struct DummyComponent {}

#[derive(Default)]
struct DummySynth {}

impl Processor for DummySynth {
    fn set_processing(&mut self, _processing: bool) {}
}

impl Synth for DummySynth {
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
        unimplemented!()
    }
}

static DEFAULT_NUMERIC: f32 = 2.0;
static MIN_NUMERIC: f32 = 1.0;
static MAX_NUMERIC: f32 = 10.0;
static NUMERIC_EPSILON: f64 = 1e-7;

static NUMERIC_ID: &str = "numeric";
static ENUM_ID: &str = "enum";
static SWITCH_ID: &str = "switch";

fn numeric_hash() -> u32 {
    parameters::hash_id(NUMERIC_ID)
}

fn enum_hash() -> u32 {
    parameters::hash_id(ENUM_ID)
}

fn switch_hash() -> u32 {
    parameters::hash_id(SWITCH_ID)
}

static PARAMETERS: [InfoRef<'static, &'static str>; 3] = [
    InfoRef {
        title: "Test Numeric",
        short_title: "Num",
        unique_id: NUMERIC_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=MAX_NUMERIC,
            units: Some("Hz"),
        },
    },
    InfoRef {
        title: "Test Enum",
        short_title: "Enum",
        unique_id: ENUM_ID,
        flags: Flags { automatable: false },
        type_specific: TypeSpecificInfoRef::Enum {
            default: 0,
            values: &["A", "B", "C"],
        },
    },
    InfoRef {
        title: "Test Switch",
        short_title: "Switch",
        unique_id: SWITCH_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch { default: false },
    },
];

impl Component for DummyComponent {
    type Processor = DummySynth;

    fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&PARAMETERS)
    }
}

static INCOMPATIBLE_PARAMETERS: [StaticInfoRef; 1] = [InfoRef {
    title: "Test Numeric",
    short_title: "Num",
    unique_id: SWITCH_ID, // This is incompatible since the previous version used this ID for a switch
    flags: Flags { automatable: true },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: DEFAULT_NUMERIC,
        valid_range: MIN_NUMERIC..=MAX_NUMERIC,
        units: Some("Hz"),
    },
}];

#[derive(Default)]
struct IncompatibleComponent {}

impl Component for IncompatibleComponent {
    type Processor = DummySynth;

    fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&INCOMPATIBLE_PARAMETERS)
    }
}

static NEWER_PARAMETERS: [StaticInfoRef; 3] = [
    InfoRef {
        title: "Test Numeric",
        short_title: "Num",
        unique_id: NUMERIC_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: Some("Hz"),
        },
    },
    InfoRef {
        title: "Test Enum",
        short_title: "Enum",
        unique_id: ENUM_ID,
        flags: Flags { automatable: false },
        type_specific: TypeSpecificInfoRef::Enum {
            default: 0,
            values: &["A", "B", "C"],
        },
    },
    InfoRef {
        title: "Test Switch",
        short_title: "Switch",
        unique_id: SWITCH_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch { default: false },
    },
];

#[derive(Default)]
struct NewerComponent {}

impl Component for NewerComponent {
    type Processor = DummySynth;

    fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&NEWER_PARAMETERS)
    }
}

static DUPLICATE_PARAMETERS: [StaticInfoRef; 2] = [
    InfoRef {
        title: "Test Numeric",
        short_title: "Num",
        unique_id: NUMERIC_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: Some("Hz"),
        },
    },
    InfoRef {
        title: "Test Numeric",
        short_title: "Num",
        unique_id: NUMERIC_ID,
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: DEFAULT_NUMERIC,
            valid_range: MIN_NUMERIC..=20.0,
            units: Some("Hz"),
        },
    },
];

fn create_parameter_model<F: Fn(&HostInfo) -> Vec<parameters::Info> + 'static>(
    f: F,
    extra_parameters: ExtraParameters,
) -> ParameterModel {
    ParameterModel {
        parameter_infos: Box::new(f),
        extra_parameters,
    }
}

fn dummy_edit_controller() -> impl IPluginBaseTrait + IEditControllerTrait + GetStore {
    super::create_internal(
        create_parameter_model(
            |_: &HostInfo| parameters::to_infos(&PARAMETERS),
            ExtraParameters::None,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        None,
    )
}

fn dummy_processor() -> impl IComponentTrait + IAudioProcessorTrait {
    processor::create_synth(
        |_: &HostInfo| -> DummyComponent { Default::default() },
        [4; 16],
    )
}

#[test]
fn defends_against_initializing_twice() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
    unsafe {
        assert_ne!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn defends_against_termination_before_initialization() {
    let ec = dummy_edit_controller();
    unsafe { assert_ne!(ec.terminate(), vst3::Steinberg::kResultOk) }
}

#[test]
fn allow_initialize_twice_with_terminate() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
    unsafe { assert_eq!(ec.terminate(), vst3::Steinberg::kResultOk) }
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn parameter_basics() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
    let num_params = unsafe { ec.getParameterCount() };
    assert_eq!(num_params, i32::try_from(PARAMETERS.len()).unwrap());

    let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
        id: 0,
        title: [0; 128],
        shortTitle: [0; 128],
        units: [0; 128],
        stepCount: 0,
        defaultNormalizedValue: 0f64,
        unitId: 0,
        flags: 0,
    };

    unsafe {
        assert_eq!(
            ec.getParameterInfo(0, &mut param_info),
            vst3::Steinberg::kResultOk
        );
    }

    assert_eq!(param_info.id, numeric_hash());
    assert_eq!(
        from_utf16_buffer(&param_info.title),
        Some(PARAMETERS[0].title.to_string())
    );
    assert_eq!(
        from_utf16_buffer(&param_info.shortTitle),
        Some(PARAMETERS[0].short_title.to_string())
    );
    assert_eq!(
        param_info.flags,
        vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate as i32
    );
    assert_eq!(param_info.stepCount, 0);
    assert_eq!(from_utf16_buffer(&param_info.units), Some("Hz".to_string()));

    unsafe {
        assert_eq!(
            ec.getParameterInfo(1, &mut param_info),
            vst3::Steinberg::kResultOk
        );
    }

    assert_eq!(param_info.id, enum_hash());
    assert_eq!(
        from_utf16_buffer(&param_info.title),
        Some(PARAMETERS[1].title.to_string())
    );
    assert_eq!(
        from_utf16_buffer(&param_info.shortTitle),
        Some(PARAMETERS[1].short_title.to_string())
    );
    assert_eq!(
        param_info.flags,
        vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsList as i32
    );
    assert_eq!(param_info.stepCount, 2);
    assert_eq!(from_utf16_buffer(&param_info.units), Some("".to_string()));
    assert_eq!(param_info.defaultNormalizedValue, 0.0);

    unsafe {
        assert_eq!(
            ec.getParameterInfo(2, &mut param_info),
            vst3::Steinberg::kResultOk
        );
    }

    assert_eq!(param_info.id, switch_hash());
    assert_eq!(
        from_utf16_buffer(&param_info.title),
        Some(PARAMETERS[2].title.to_string())
    );
    assert_eq!(
        from_utf16_buffer(&param_info.shortTitle),
        Some(PARAMETERS[2].short_title.to_string())
    );
    assert_eq!(
        param_info.flags,
        vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kCanAutomate as i32
    );
    assert_eq!(param_info.stepCount, 1);
    assert_eq!(from_utf16_buffer(&param_info.units), Some("".to_string()));
    assert_eq!(param_info.defaultNormalizedValue, 0.0);
}

#[test]
fn defends_against_count_without_initialize() {
    let ec = dummy_edit_controller();
    unsafe {
        assert_eq!(ec.getParameterCount(), 0);
    }
}

#[test]
fn defends_against_get_param_info_without_initialize() {
    let ec = dummy_edit_controller();
    let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
        id: 0,
        title: [0; 128],
        shortTitle: [0; 128],
        units: [0; 128],
        stepCount: 0,
        defaultNormalizedValue: 0f64,
        unitId: 0,
        flags: 0,
    };
    assert_ne!(
        unsafe { ec.getParameterInfo(0, &mut param_info) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_bad_param_index() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }

    let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
        id: 0,
        title: [0; 128],
        shortTitle: [0; 128],
        units: [0; 128],
        stepCount: 0,
        defaultNormalizedValue: 0f64,
        unitId: 0,
        flags: 0,
    };
    unsafe {
        assert_ne!(
            ec.getParameterInfo(-1, &mut param_info),
            vst3::Steinberg::kResultOk
        );
        assert_ne!(
            ec.getParameterInfo(77, &mut param_info),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn normalization_basics() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }

    // Test numeric parameter normalized -> plain.
    // Note that this should be the identity function (see note in implementation)
    unsafe {
        assert_eq!(ec.normalizedParamToPlain(numeric_hash(), 0.0), 0.0);
        assert_eq!(ec.normalizedParamToPlain(numeric_hash(), 1.0), 1.0);
        assert!((ec.normalizedParamToPlain(numeric_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
    }

    // Test numeric parameter plain -> normalized
    unsafe {
        assert_eq!(ec.plainParamToNormalized(numeric_hash(), 0.0 as f64), 0.0);
        assert_eq!(ec.plainParamToNormalized(numeric_hash(), 1.0 as f64), 1.0);
        assert!((ec.plainParamToNormalized(numeric_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
    }

    // Test enum parameter normalized -> plain
    unsafe {
        assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.0), 0.0);
        assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.8), 0.8);
        assert_eq!(ec.normalizedParamToPlain(enum_hash(), 1.0), 1.0);
        assert_eq!(ec.normalizedParamToPlain(enum_hash(), 0.5), 0.5);
    }

    // Test enum parameter plain -> normalized
    unsafe {
        assert_eq!(ec.plainParamToNormalized(enum_hash(), 0.0), 0.0);
        assert_eq!(ec.plainParamToNormalized(enum_hash(), 1.0), 1.0);
        assert!((ec.plainParamToNormalized(enum_hash(), 0.5) - 0.5).abs() < NUMERIC_EPSILON);
    }

    // Test switch parameter normalized -> plain
    unsafe {
        assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.0), 0.0);
        assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.8), 0.8);
        assert_eq!(ec.normalizedParamToPlain(switch_hash(), 0.5), 0.5);
    }

    // Test switch parameter plain -> normalized
    unsafe {
        assert_eq!(ec.plainParamToNormalized(switch_hash(), 0.0), 0.0);
        assert_eq!(ec.plainParamToNormalized(switch_hash(), 1.0), 1.0);
    }
}

#[test]
fn value_to_string_basics() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
    let mut string = [0; 128];

    // Test numeric parameter
    unsafe {
        assert_eq!(
            ec.getParamStringByValue(
                numeric_hash(),
                f64::from((5.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)),
                string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
            ),
            vst3::Steinberg::kResultOk
        );
    }
    assert_eq!(
        from_utf16_buffer(&string),
        Some(format!("{:.2}", 5.0).to_string())
    );

    // Test enum parameter
    unsafe {
        assert_eq!(
            ec.getParamStringByValue(
                enum_hash(),
                0.5,
                string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
            ),
            vst3::Steinberg::kResultOk
        );
    }
    assert_eq!(from_utf16_buffer(&string), Some("B".to_string()));

    // Test switch parameter
    unsafe {
        assert_eq!(
            ec.getParamStringByValue(
                switch_hash(),
                1.0,
                string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
            ),
            vst3::Steinberg::kResultOk
        );
    }

    assert_eq!(from_utf16_buffer(&string), Some("On".to_string()));
}

#[test]
fn defends_against_value_to_string_without_initialize() {
    let ec = dummy_edit_controller();
    let mut string = [0; 128];
    assert_ne!(
        unsafe {
            ec.getParamStringByValue(
                0,
                0.0,
                string.as_mut_ptr() as *mut vst3::Steinberg::Vst::String128,
            )
        },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn string_to_value_basics() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        )
    }
    let mut string = [0; 128];

    // Test numeric
    to_utf16("5", &mut string);

    let mut value = 0.0;
    unsafe {
        assert_eq!(
            ec.getParamValueByString(numeric_hash(), string.as_mut_ptr(), &mut value),
            vst3::Steinberg::kResultOk
        );
        assert!(
            (value - f64::from((5.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC))).abs()
                < NUMERIC_EPSILON
        );
    }

    // Test enum
    to_utf16("B", &mut string);

    unsafe {
        assert_eq!(
            ec.getParamValueByString(enum_hash(), string.as_mut_ptr(), &mut value),
            vst3::Steinberg::kResultOk
        );
        assert!((value - 0.5).abs() < NUMERIC_EPSILON);
    }

    // Test switch
    to_utf16("On", &mut string);

    unsafe {
        assert_eq!(
            ec.getParamValueByString(switch_hash(), string.as_mut_ptr(), &mut value),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(value, 1.0);
    }
}

#[test]
fn defends_against_get_param_normalized_called_too_early() {
    let ec = dummy_edit_controller();
    assert_eq!(unsafe { ec.getParamNormalized(numeric_hash()) }, 0.0);
}

#[test]
fn defends_against_get_param_normalized_invalid_parameter() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(ec.initialize(host.cast().unwrap().as_ptr()), 0);
    }
    assert_eq!(unsafe { ec.getParamNormalized(400) }, 0.0);
}

#[test]
fn get_param_normalized_starts_default() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();
    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        // Test numeric
        let param_hash = numeric_hash();
        assert!(
            (ec.getParamNormalized(param_hash)
                - f64::from((DEFAULT_NUMERIC - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)))
            .abs()
                < NUMERIC_EPSILON
        );
    }
}

#[test]
fn defends_against_set_param_normalized_called_too_early() {
    let ec = dummy_edit_controller();
    assert_ne!(
        unsafe { ec.setParamNormalized(numeric_hash(), 0.5) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_set_param_normalized_called_out_of_range() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }

    assert_ne!(
        unsafe { ec.setParamNormalized(numeric_hash(), -0.5) },
        vst3::Steinberg::kResultOk
    );
    assert_ne!(
        unsafe { ec.setParamNormalized(numeric_hash(), 1.5) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_set_param_normalized_called_on_bad_parameter() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }

    assert_ne!(
        unsafe { ec.setParamNormalized(400, 0.5) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn set_param_normalized_can_change_value() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }

    unsafe {
        assert_eq!(
            ec.setParamNormalized(numeric_hash(), 0.5),
            vst3::Steinberg::kResultOk
        );
        assert!((ec.getParamNormalized(numeric_hash()) - 0.5).abs() < NUMERIC_EPSILON);
    }
}

#[test]
fn set_param_normalized_switch_and_enum() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }

    unsafe {
        assert_eq!(
            ec.setParamNormalized(enum_hash(), 0.6),
            vst3::Steinberg::kResultOk
        );
        assert!((ec.getParamNormalized(enum_hash()) - 0.5).abs() < NUMERIC_EPSILON);
    }
    unsafe {
        assert_eq!(
            ec.setParamNormalized(switch_hash(), 0.6),
            vst3::Steinberg::kResultOk
        );
        assert!((ec.getParamNormalized(switch_hash()) - 1.0).abs() < NUMERIC_EPSILON);
    }
}

#[test]
fn defends_against_calling_set_component_state_too_early() {
    let ec = dummy_edit_controller();
    let stream = ComWrapper::new(Stream::new([]));
    assert_ne!(
        unsafe {
            ec.setComponentState(
                stream
                    .as_com_ref::<vst3::Steinberg::IBStream>()
                    .unwrap()
                    .as_ptr(),
            )
        },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_calling_set_component_state_null() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default())
        .to_com_ptr::<IHostApplication>()
        .unwrap();

    unsafe {
        assert_eq!(
            ec.initialize(host.cast().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
    assert_ne!(
        unsafe { ec.setComponentState(std::ptr::null_mut()) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn set_component_state_basics() {
    let proc = dummy_processor();
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        setup_proc(&proc, &host);

        assert_eq!(
            proc.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: ENUM_ID,
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
            proc.getState(
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
            ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            ec.getParamNormalized(enum_hash()),
            1.0,
            "Parameter value should be restored"
        );
    }
}

#[test]
fn set_component_incompatible_error() {
    let proc = processor::create_synth(
        |_: &HostInfo| -> IncompatibleComponent { Default::default() },
        [5; 16],
    );
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
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
        assert_eq!(
            stream.seek(
                0,
                vst3::Steinberg::IBStream_::IStreamSeekMode_::kIBSeekSet as i32,
                std::ptr::null_mut(),
            ),
            vst3::Steinberg::kResultOk
        );

        assert_ne!(
            ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
}

#[test]
fn set_component_newer_loads_defaults() {
    let proc = processor::create_synth(
        |_: &HostInfo| -> NewerComponent { Default::default() },
        [5; 16],
    );
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        setup_proc(&proc, &host);

        assert_eq!(
            ec.setParamNormalized(numeric_hash(), 1.0),
            vst3::Steinberg::kResultOk
        );

        assert_eq!(
            proc.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: NUMERIC_ID,
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
            proc.getState(
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
            ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );

        assert!(
            (ec.getParamNormalized(numeric_hash())
                - ((DEFAULT_NUMERIC - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64)
                .abs()
                < NUMERIC_EPSILON
        );
    }
}

#[derive(PartialEq)]
enum ComponentHandlerCalls {
    BeginEdit(u32),
    PerformEdit(u32, f64),
    EndEdit(u32),
    RestartComponent(i32),
}

#[derive(Default)]
struct ComponentHandlerSpy {
    calls: RefCell<Vec<ComponentHandlerCalls>>,
}

impl IComponentHandlerTrait for ComponentHandlerSpy {
    unsafe fn beginEdit(&self, id: vst3::Steinberg::Vst::ParamID) -> vst3::Steinberg::tresult {
        self.calls
            .borrow_mut()
            .push(ComponentHandlerCalls::BeginEdit(id));
        vst3::Steinberg::kResultOk
    }

    unsafe fn performEdit(
        &self,
        id: vst3::Steinberg::Vst::ParamID,
        value_normalized: vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::tresult {
        self.calls
            .borrow_mut()
            .push(ComponentHandlerCalls::PerformEdit(id, value_normalized));
        vst3::Steinberg::kResultOk
    }

    unsafe fn endEdit(&self, id: vst3::Steinberg::Vst::ParamID) -> vst3::Steinberg::tresult {
        self.calls
            .borrow_mut()
            .push(ComponentHandlerCalls::EndEdit(id));
        vst3::Steinberg::kResultOk
    }

    unsafe fn restartComponent(&self, flags: vst3::Steinberg::int32) -> vst3::Steinberg::tresult {
        self.calls
            .borrow_mut()
            .push(ComponentHandlerCalls::RestartComponent(flags));
        vst3::Steinberg::kResultOk
    }
}

impl Class for ComponentHandlerSpy {
    type Interfaces = (IComponentHandler,);
}

#[test]
fn defends_against_set_component_handler_called_too_soon() {
    let ec = dummy_edit_controller();
    let handler = ComWrapper::new(ComponentHandlerSpy::default());
    assert_ne!(
        unsafe { ec.setComponentHandler(handler.as_com_ref().unwrap().as_ptr()) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_set_component_handler_null() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    assert_eq!(
        unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) },
        vst3::Steinberg::kResultOk
    );
    assert_ne!(
        unsafe { ec.setComponentHandler(std::ptr::null_mut()) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn set_component_handler_succeeds() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    assert_eq!(
        unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) },
        vst3::Steinberg::kResultOk
    );
    let handler = ComWrapper::new(ComponentHandlerSpy::default());
    assert_eq!(
        unsafe { ec.setComponentHandler(handler.as_com_ref().unwrap().as_ptr()) },
        vst3::Steinberg::kResultOk
    );
}

#[test]
fn defends_against_create_view_called_with_weird_name() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let weird_name = std::ffi::CString::new("weird name").unwrap();
        assert!(vst3::ComPtr::from_raw(ec.createView(weird_name.as_ptr())).is_none());
    }
}

#[test]
#[should_panic]
fn panic_on_duplicate_ids() {
    let ec = super::create_internal(
        create_parameter_model(
            |_: &HostInfo| parameters::to_infos(&DUPLICATE_PARAMETERS),
            ExtraParameters::None,
        ),
        "test_prefs".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        None,
    );
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        ec.initialize(host.as_com_ref().unwrap().as_ptr());
    }
}

#[test]
fn get_parameters_from_store() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }
    let store = ec.get_store();
    assert!(store.is_some());
    let store = store.unwrap();
    assert_eq!(
        store.get(NUMERIC_ID),
        Some(parameters::Value::Numeric(DEFAULT_NUMERIC as f32))
    );
    assert_eq!(
        store.get(ENUM_ID),
        Some(parameters::Value::Enum("A".to_string()))
    );
    assert_eq!(store.get(SWITCH_ID), Some(parameters::Value::Switch(false)));
    assert_eq!(store.get("Invalid"), None);
}

struct SpyListener {
    param_changes: RefCell<Vec<(String, parameters::Value)>>,
}

impl store::Listener for SpyListener {
    fn parameter_changed(&self, id: &str, value: &parameters::Value) {
        self.param_changes
            .borrow_mut()
            .push((id.to_string(), value.clone()));
    }
}

#[test]
fn changing_parameters_in_store() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let store = ec.get_store();
        assert!(store.is_some());
        let mut store = store.unwrap();
        let listener = rc::Rc::new(SpyListener {
            param_changes: RefCell::new(vec![]),
        });
        store.set_listener(rc::Rc::downgrade(
            &(listener.clone() as rc::Rc<dyn store::Listener>),
        ));
        assert_eq!(
            ec.setParamNormalized(enum_hash(), 1.0),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.get(ENUM_ID),
            Some(parameters::Value::Enum("C".to_string()))
        );
        assert_eq!(
            listener.param_changes.borrow().as_slice(),
            &[(
                ENUM_ID.to_string(),
                parameters::Value::Enum("C".to_string())
            )]
        );
    }
}

#[test]
fn set_component_state_sets_params() {
    let proc = dummy_processor();
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let store = ec.get_store();
        assert!(store.is_some());
        let mut store = store.unwrap();
        let listener = rc::Rc::new(SpyListener {
            param_changes: RefCell::new(vec![]),
        });
        store.set_listener(rc::Rc::downgrade(
            &(listener.clone() as rc::Rc<dyn store::Listener>),
        ));

        setup_proc(&proc, &host);

        assert_eq!(
            proc.process(
                &mut mock_no_audio_process_data(
                    vec![],
                    vec![ParameterValueQueueImpl {
                        param_id: ENUM_ID,
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
            proc.getState(
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
            ec.setComponentState(stream.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.get(ENUM_ID),
            Some(parameters::Value::Enum("C".to_string()))
        );
        assert!(listener.param_changes.borrow().as_slice().contains(&(
            ENUM_ID.to_string(),
            parameters::Value::Enum("C".to_string())
        )),);
    }
}

#[test]
fn set_from_store_forwarded_to_component_handler() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.set(ENUM_ID, parameters::Value::Enum("C".to_string())),
            Ok(())
        );
        assert!(spy
            .calls
            .borrow()
            .iter()
            .any(|call| call
                == &ComponentHandlerCalls::PerformEdit(parameters::hash_id(ENUM_ID), 1.0)));
    }
}

#[test]
fn invalid_id_fails_set() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.set("Not a real ID", parameters::Value::Enum("C".to_string())),
            Err(store::SetError::NotFound)
        );
    }
}

#[test]
fn no_component_handler_fails_set() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            store.set(ENUM_ID, parameters::Value::Enum("C".to_string())),
            Err(store::SetError::InternalError)
        );
    }
}

#[test]
fn invalid_enum_fails_set() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.set(
                ENUM_ID,
                parameters::Value::Enum("Not a real value".to_string())
            ),
            Err(store::SetError::InvalidValue)
        );
    }
}

#[test]
fn out_of_range_numeric_fails_set() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.set(NUMERIC_ID, parameters::Value::Numeric(MAX_NUMERIC + 1.0)),
            Err(store::SetError::InvalidValue)
        );
    }
}

#[test]
fn wrong_type_fails_set() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(
            store.set(NUMERIC_ID, parameters::Value::Switch(false)),
            Err(store::SetError::WrongType)
        );
    }
}

#[test]
fn set_grabbed_from_store_forwarded_to_component_handler() {
    let ec = dummy_edit_controller();

    let host = ComWrapper::new(dummy_host::Host::default());
    let spy = ComWrapper::new(ComponentHandlerSpy::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let mut store = ec.get_store().unwrap();
        assert_eq!(
            ec.setComponentHandler(spy.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        assert_eq!(store.set_grabbed(ENUM_ID, true), Ok(()));
        assert_eq!(store.set_grabbed(ENUM_ID, false), Ok(()));
        assert!(spy
            .calls
            .borrow()
            .iter()
            .any(|call| call == &ComponentHandlerCalls::BeginEdit(parameters::hash_id(ENUM_ID))));
        assert!(spy
            .calls
            .borrow()
            .iter()
            .any(|call| call == &ComponentHandlerCalls::EndEdit(parameters::hash_id(ENUM_ID))));
    }
}

#[test]
fn get_info_basics() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
        let store = ec.get_store().unwrap();
        assert_eq!(store.get_info(ENUM_ID), Some((&PARAMETERS[1]).into()));
    }
}

#[test]
fn defends_against_get_info_bad_id() {
    let ec = dummy_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
        let store = ec.get_store().unwrap();
        assert_eq!(store.get_info("Not a real ID"), None);
    }
}

#[test]
#[should_panic]
fn defends_against_missing_bypass_param() {
    let ec = super::create_internal(
        create_parameter_model(
            |_: &HostInfo| parameters::to_infos(&PARAMETERS),
            ExtraParameters::None,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        Some("missing"),
    );

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
}

#[test]
#[should_panic]
fn defends_against_non_switch_bypass_param() {
    let ec = super::create_internal(
        create_parameter_model(
            |_: &HostInfo| parameters::to_infos(&PARAMETERS),
            ExtraParameters::None,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        Some(NUMERIC_ID),
    );

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
}

#[test]
#[should_panic]
fn defends_against_default_on_bypass_param() {
    let ec = super::create_internal(
        create_parameter_model(
            |_: &HostInfo| {
                parameters::to_infos(&[InfoRef {
                    title: "Test Switch",
                    short_title: "Switch",
                    unique_id: SWITCH_ID,
                    flags: Flags { automatable: true },
                    type_specific: TypeSpecificInfoRef::Switch { default: true },
                }])
            },
            ExtraParameters::None,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        Some(SWITCH_ID),
    );

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe { ec.initialize(host.as_com_ref().unwrap().as_ptr()) };
}

#[test]
fn bypass_parameter_exposed() {
    let ec = super::create_internal(
        create_parameter_model(
            |_: &HostInfo| {
                parameters::to_infos(&[InfoRef {
                    title: "Test Switch",
                    short_title: "Switch",
                    unique_id: SWITCH_ID,
                    flags: Flags { automatable: true },
                    type_specific: TypeSpecificInfoRef::Switch { default: false },
                }])
            },
            ExtraParameters::None,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        Some(SWITCH_ID),
    );

    let host = ComWrapper::new(dummy_host::Host::default());

    unsafe {
        assert_eq!(
            ec.initialize(host.as_com_ref().unwrap().as_ptr()),
            vst3::Steinberg::kResultOk
        );
    }

    let mut param_info = vst3::Steinberg::Vst::ParameterInfo {
        id: 0,
        title: [0; 128],
        shortTitle: [0; 128],
        units: [0; 128],
        stepCount: 0,
        defaultNormalizedValue: 0f64,
        unitId: 0,
        flags: 0,
    };

    unsafe {
        assert_eq!(
            ec.getParameterInfo(0, &mut param_info),
            vst3::Steinberg::kResultOk
        );
    }

    assert!(
        param_info.flags & vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass as i32
            != 0
    );
}

fn dummy_synth_edit_controller(
) -> impl IPluginBaseTrait + IEditControllerTrait + IMidiMappingTrait + GetStore {
    super::create_internal(
        create_parameter_model(
            |_: &HostInfo| parameters::to_infos(&[]),
            ExtraParameters::SynthControlParameters,
        ),
        "dummy_domain".to_string(),
        conformal_ui::Size {
            width: 0,
            height: 0,
        },
        None,
    )
}

#[test]
fn synth_control_parameters_exposed() {
    let ec = dummy_synth_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        let check_assignment = |vst_id: std::ffi::c_uint, param_id| {
            let mut id: vst3::Steinberg::Vst::ParamID = 0;
            assert_eq!(
                ec.getMidiControllerAssignment(0, 0, vst_id.try_into().unwrap(), &mut id as *mut _),
                vst3::Steinberg::kResultTrue
            );

            assert_eq!(hash_id(param_id), id);
        };
        check_assignment(
            vst3::Steinberg::Vst::ControllerNumbers_::kPitchBend,
            conformal_component::synth::PITCH_BEND_PARAMETER,
        );
        check_assignment(
            vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel,
            conformal_component::synth::MOD_WHEEL_PARAMETER,
        );
        check_assignment(
            vst3::Steinberg::Vst::ControllerNumbers_::kCtrlExpression,
            conformal_component::synth::EXPRESSION_PARAMETER,
        );
        check_assignment(
            vst3::Steinberg::Vst::ControllerNumbers_::kCtrlSustainOnOff,
            conformal_component::synth::SUSTAIN_PARAMETER,
        );
        check_assignment(
            vst3::Steinberg::Vst::ControllerNumbers_::kAfterTouch,
            conformal_component::synth::AFTERTOUCH_PARAMETER,
        );

        assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
        let store = ec.get_store().unwrap();
        assert_eq!(
            store.get(conformal_component::synth::PITCH_BEND_PARAMETER),
            Some(parameters::Value::Numeric(0.0))
        );
    }
}

#[test]
fn midi_mapping_bad_context_false() {
    let ec = dummy_synth_edit_controller();
    let host = ComWrapper::new(dummy_host::Host::default());
    unsafe {
        assert_eq!(ec.initialize(host.as_com_ref().unwrap().as_ptr()), 0);
        let mut id: vst3::Steinberg::Vst::ParamID = 0;
        assert_eq!(
            ec.getMidiControllerAssignment(
                1,
                0,
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel
                    .try_into()
                    .unwrap(),
                &mut id as *mut _
            ),
            vst3::Steinberg::kResultFalse
        );
        assert_eq!(
            ec.getMidiControllerAssignment(
                0,
                1,
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlModWheel
                    .try_into()
                    .unwrap(),
                &mut id as *mut _
            ),
            vst3::Steinberg::kResultFalse
        );
        assert_eq!(
            ec.getMidiControllerAssignment(
                0,
                0,
                // This test will have to change if we ever support kCtrlGPC8...
                vst3::Steinberg::Vst::ControllerNumbers_::kCtrlGPC8
                    .try_into()
                    .unwrap(),
                &mut id as *mut _
            ),
            vst3::Steinberg::kResultFalse
        );
    }
}
