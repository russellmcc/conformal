//! The VST3 processor implementation.

use std::cell::RefCell;

use crate::mpe;
use crate::{ClassID, ComponentFactory};
use conformal_component::audio::{Buffer, BufferMut, ChannelLayout};
use conformal_component::effect::Effect;
use conformal_component::events::{Event, Events};
use conformal_component::parameters::BufferStates;
use conformal_component::synth::{Synth, SynthParamBufferStates, SynthParamStates};
use conformal_component::{
    Component, ProcessingEnvironment, ProcessingMode, Processor as ProcessorT,
};
use serde::Serialize;
use vst3::Steinberg::Vst::{
    IConnectionPoint, IConnectionPointTrait, IHostApplication, IProcessContextRequirements,
    IProcessContextRequirementsTrait,
};
use vst3::{
    Class,
    Steinberg::{
        IPluginBase, IPluginBaseTrait,
        Vst::{IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentTrait},
    },
};
use vst3::{ComPtr, ComRef};

use super::io::{StreamRead, StreamWrite};
use super::{host_info, to_utf16};

#[cfg(test)]
pub mod test_utils;

pub mod state;

mod parameters;

struct InitializedData<C, CF> {
    conformal_component: C,
    params_main: parameters::MainStore,
    processing_environment: Option<PartialProcessingEnvironment>,

    // Note - this tracks the state of `process_context`, to allow
    // functions that do not have thread-safe access to `process_context`
    // to enforce call sequence rules.
    //
    // We carefully maintain the invariant that this is true whenever
    // `process_context` is in the `Active` state, and this is false
    // whenever `process_context` is in the `Inactive` state.
    //
    // Note that separately we maintain that `InitializedData` never
    // exists when `process_context` is in the `Uninitialized` state.
    //
    // These invariants are tricky! But it allows us to enforce call
    // sequence rules without having to use locks.
    process_context_active: bool,

    factory: CF,
}

struct ActiveProcessContext<P, A> {
    processing: bool,
    params: parameters::ProcessingStore,

    processor: P,
    active_processor: A,

    // Synths need a separate state for MPE.
    mpe: Option<mpe::State>,
}

#[derive(Default)]
enum ProcessContext<P, A> {
    #[default]
    Uninitialized,
    Active(ActiveProcessContext<P, A>),
    Inactive {
        /// Note that according to the [workflow diagrams](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Workflow+Diagrams/Audio+Processor+Call+Sequence.html) in the spec,
        /// `setActive` can only be called when processing is off.
        ///
        /// However, in practice some DAWs (tested Ableton 11.3.20) will call `setActive(0)`
        /// while processing on, and later they will call `setActive(1)` and seem to expect
        /// processing to be on. This is obviously not allowed by the spec, but we support it
        /// by keeping the `processing_active` state here, even when we are inactive.
        processing: bool,
        params: parameters::ProcessingStore,
    },
}

enum State<C, CF> {
    ReadyForInitialization(CF),
    Initialized(InitializedData<C, CF>),
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct SynthBusActivationState {
    event_input_active: bool,
    audio_output_active: bool,
}

#[derive(Clone, Debug)]
struct SynthProcessorCategory {
    channel_layout: ChannelLayout,
    bus_activation_state: SynthBusActivationState,
}

#[derive(Debug)]
enum ActiveSynthProcessor {
    AudioEnabled(ChannelLayout),
    AudioDisabled,
}

impl Default for SynthProcessorCategory {
    fn default() -> Self {
        SynthProcessorCategory {
            channel_layout: ChannelLayout::Stereo,
            bus_activation_state: Default::default(),
        }
    }
}

trait ActiveProcessor<P> {
    type ProcessBuffer<'a>: ProcessBuffer
    where
        P: 'a,
        Self: 'a;

    fn audio_enabled(&self) -> bool;

    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>>;

    unsafe fn handle_events(
        &self,
        processor: &mut P,
        e: Option<impl Iterator<Item = conformal_component::events::Data> + Clone>,
        params: &mut parameters::ProcessingStore,
        mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> vst3::Steinberg::tresult;
}

trait ProcessorCategory {
    type Active;

    fn activate(&self) -> Self::Active;

    fn mpe(&self) -> Option<mpe::State>;

    fn create_processor<C: Component>(
        &self,
        conformal_component: &C,
        env: &PartialProcessingEnvironment,
    ) -> C::Processor;

    unsafe fn get_bus_count(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
    ) -> vst3::Steinberg::int32;

    unsafe fn get_bus_info(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        bus: *mut vst3::Steinberg::Vst::BusInfo,
    ) -> vst3::Steinberg::tresult;

    unsafe fn get_bus_arrangement(
        &self,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        arr: *mut vst3::Steinberg::Vst::SpeakerArrangement,
    ) -> vst3::Steinberg::tresult;

    unsafe fn activate_bus(
        &mut self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        state: vst3::Steinberg::TBool,
    ) -> vst3::Steinberg::tresult;

    unsafe fn set_bus_arrangements(
        &mut self,
        inputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_ins: vst3::Steinberg::int32,
        outputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_outs: vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult;

    fn get_extra_parameters(
        &self,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone;
}

impl ProcessorCategory for SynthProcessorCategory {
    type Active = ActiveSynthProcessor;

    fn activate(&self) -> Self::Active {
        if self.bus_activation_state.event_input_active
            && self.bus_activation_state.audio_output_active
        {
            ActiveSynthProcessor::AudioEnabled(self.channel_layout)
        } else {
            ActiveSynthProcessor::AudioDisabled
        }
    }

    unsafe fn get_bus_count(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
    ) -> vst3::Steinberg::int32 {
        match (
            rtype as vst3::Steinberg::Vst::MediaTypes,
            dir as vst3::Steinberg::Vst::BusDirections,
        ) {
            (
                vst3::Steinberg::Vst::MediaTypes_::kAudio,
                vst3::Steinberg::Vst::BusDirections_::kOutput,
            )
            | (
                vst3::Steinberg::Vst::MediaTypes_::kEvent,
                vst3::Steinberg::Vst::BusDirections_::kInput,
            ) => 1,
            _ => 0,
        }
    }

    unsafe fn get_bus_info(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        bus: *mut vst3::Steinberg::Vst::BusInfo,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            match (
                rtype as vst3::Steinberg::Vst::MediaTypes,
                dir as vst3::Steinberg::Vst::BusDirections,
                index,
            ) {
                (
                    vst3::Steinberg::Vst::MediaTypes_::kAudio,
                    vst3::Steinberg::Vst::BusDirections_::kOutput,
                    0,
                ) => {
                    (*bus).mediaType = rtype;
                    (*bus).direction = dir;
                    (*bus).channelCount = match self.channel_layout {
                        ChannelLayout::Mono => 1,
                        ChannelLayout::Stereo => 2,
                    };
                    (*bus).busType = vst3::Steinberg::Vst::BusTypes_::kMain as i32;
                    (*bus).flags = vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;

                    // fill name
                    to_utf16("Output", &mut (*bus).name);

                    vst3::Steinberg::kResultOk
                }
                (
                    vst3::Steinberg::Vst::MediaTypes_::kEvent,
                    vst3::Steinberg::Vst::BusDirections_::kInput,
                    0,
                ) => {
                    (*bus).mediaType = rtype;
                    (*bus).direction = dir;
                    (*bus).channelCount = 1;
                    (*bus).busType = vst3::Steinberg::Vst::BusTypes_::kMain as i32;
                    (*bus).flags = vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;

                    // Fill name
                    to_utf16("Event In", &mut (*bus).name);

                    vst3::Steinberg::kResultOk
                }
                _ => vst3::Steinberg::kInvalidArgument,
            }
        }
    }

    unsafe fn get_bus_arrangement(
        &self,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        arr: *mut vst3::Steinberg::Vst::SpeakerArrangement,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if index != 0 || dir != vst3::Steinberg::Vst::BusDirections_::kOutput as i32 {
                return vst3::Steinberg::kInvalidArgument;
            }

            match self.channel_layout {
                ChannelLayout::Mono => {
                    *arr = vst3::Steinberg::Vst::SpeakerArr::kMono;
                }
                ChannelLayout::Stereo => {
                    *arr = vst3::Steinberg::Vst::SpeakerArr::kStereo;
                }
            }
            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn activate_bus(
        &mut self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        state: vst3::Steinberg::TBool,
    ) -> vst3::Steinberg::tresult {
        match (
            rtype as vst3::Steinberg::Vst::MediaTypes,
            dir as vst3::Steinberg::Vst::BusDirections,
            index,
        ) {
            (
                vst3::Steinberg::Vst::MediaTypes_::kAudio,
                vst3::Steinberg::Vst::BusDirections_::kOutput,
                0,
            ) => {
                self.bus_activation_state.audio_output_active = state != 0;
                vst3::Steinberg::kResultOk
            }
            (
                vst3::Steinberg::Vst::MediaTypes_::kEvent,
                vst3::Steinberg::Vst::BusDirections_::kInput,
                0,
            ) => {
                self.bus_activation_state.event_input_active = state != 0;
                vst3::Steinberg::kResultOk
            }
            _ => vst3::Steinberg::kInvalidArgument,
        }
    }

    fn create_processor<C: Component>(
        &self,
        conformal_component: &C,
        env: &PartialProcessingEnvironment,
    ) -> C::Processor {
        conformal_component.create_processor(&make_env(env, self.channel_layout))
    }

    unsafe fn set_bus_arrangements(
        &mut self,
        _inputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_ins: vst3::Steinberg::int32,
        outputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_outs: vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if num_ins != 0 || num_outs != 1 {
                return vst3::Steinberg::kInvalidArgument;
            }
            match *outputs {
                vst3::Steinberg::Vst::SpeakerArr::kMono => {
                    self.channel_layout = ChannelLayout::Mono;
                    vst3::Steinberg::kResultTrue
                }
                vst3::Steinberg::Vst::SpeakerArr::kStereo => {
                    self.channel_layout = ChannelLayout::Stereo;
                    vst3::Steinberg::kResultTrue
                }
                _ => vst3::Steinberg::kResultFalse,
            }
        }
    }

    fn get_extra_parameters(
        &self,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone {
        crate::parameters::CONTROLLER_PARAMETERS
            .iter()
            .map(Into::into)
            .chain(mpe::quirks::parameters())
    }

    fn mpe(&self) -> Option<mpe::State> {
        Some(Default::default())
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct EffectBusActivationState {
    audio_input_active: bool,
    audio_output_active: bool,
}

#[derive(Debug)]
struct EffectProcessorCategory {
    channel_layout: ChannelLayout,
    bus_activation_state: EffectBusActivationState,
}

impl Default for EffectProcessorCategory {
    fn default() -> Self {
        EffectProcessorCategory {
            channel_layout: ChannelLayout::Stereo,
            bus_activation_state: Default::default(),
        }
    }
}

#[derive(Debug)]
enum ActiveEffectProcessor {
    AudioEnabled(ChannelLayout),
    AudioDisabled,
}

impl ProcessorCategory for EffectProcessorCategory {
    type Active = ActiveEffectProcessor;

    fn activate(&self) -> Self::Active {
        if self.bus_activation_state.audio_input_active
            && self.bus_activation_state.audio_output_active
        {
            ActiveEffectProcessor::AudioEnabled(self.channel_layout)
        } else {
            ActiveEffectProcessor::AudioDisabled
        }
    }

    fn create_processor<C: Component>(
        &self,
        conformal_component: &C,
        env: &PartialProcessingEnvironment,
    ) -> C::Processor {
        conformal_component.create_processor(&make_env(env, self.channel_layout))
    }

    unsafe fn get_bus_count(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
    ) -> vst3::Steinberg::int32 {
        match (
            rtype as vst3::Steinberg::Vst::MediaTypes,
            dir as vst3::Steinberg::Vst::BusDirections,
        ) {
            (
                vst3::Steinberg::Vst::MediaTypes_::kAudio,
                vst3::Steinberg::Vst::BusDirections_::kOutput
                | vst3::Steinberg::Vst::BusDirections_::kInput,
            ) => 1,
            _ => 0,
        }
    }

    unsafe fn get_bus_info(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        bus: *mut vst3::Steinberg::Vst::BusInfo,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            match (
                rtype as vst3::Steinberg::Vst::MediaTypes,
                dir as vst3::Steinberg::Vst::BusDirections,
                index,
            ) {
                (
                    vst3::Steinberg::Vst::MediaTypes_::kAudio,
                    vst3::Steinberg::Vst::BusDirections_::kOutput,
                    0,
                ) => {
                    (*bus).mediaType = rtype;
                    (*bus).direction = dir;
                    (*bus).channelCount = match self.channel_layout {
                        ChannelLayout::Mono => 1,
                        ChannelLayout::Stereo => 2,
                    };
                    (*bus).busType = vst3::Steinberg::Vst::BusTypes_::kMain as i32;
                    (*bus).flags = vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;

                    // fill name
                    to_utf16("Output", &mut (*bus).name);

                    vst3::Steinberg::kResultOk
                }
                (
                    vst3::Steinberg::Vst::MediaTypes_::kAudio,
                    vst3::Steinberg::Vst::BusDirections_::kInput,
                    0,
                ) => {
                    (*bus).mediaType = rtype;
                    (*bus).direction = dir;
                    (*bus).channelCount = match self.channel_layout {
                        ChannelLayout::Mono => 1,
                        ChannelLayout::Stereo => 2,
                    };
                    (*bus).busType = vst3::Steinberg::Vst::BusTypes_::kMain as i32;
                    (*bus).flags = vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;

                    // Fill name
                    to_utf16("Input", &mut (*bus).name);

                    vst3::Steinberg::kResultOk
                }
                _ => vst3::Steinberg::kInvalidArgument,
            }
        }
    }

    unsafe fn get_bus_arrangement(
        &self,
        _dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        arr: *mut vst3::Steinberg::Vst::SpeakerArrangement,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if index != 0 {
                return vst3::Steinberg::kInvalidArgument;
            }

            match self.channel_layout {
                ChannelLayout::Mono => {
                    *arr = vst3::Steinberg::Vst::SpeakerArr::kMono;
                }
                ChannelLayout::Stereo => {
                    *arr = vst3::Steinberg::Vst::SpeakerArr::kStereo;
                }
            }
            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn activate_bus(
        &mut self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        state: vst3::Steinberg::TBool,
    ) -> vst3::Steinberg::tresult {
        match (
            rtype as vst3::Steinberg::Vst::MediaTypes,
            dir as vst3::Steinberg::Vst::BusDirections,
            index,
        ) {
            (
                vst3::Steinberg::Vst::MediaTypes_::kAudio,
                vst3::Steinberg::Vst::BusDirections_::kOutput,
                0,
            ) => {
                self.bus_activation_state.audio_output_active = state != 0;
                vst3::Steinberg::kResultOk
            }
            (
                vst3::Steinberg::Vst::MediaTypes_::kAudio,
                vst3::Steinberg::Vst::BusDirections_::kInput,
                0,
            ) => {
                self.bus_activation_state.audio_input_active = state != 0;
                vst3::Steinberg::kResultOk
            }
            _ => vst3::Steinberg::kInvalidArgument,
        }
    }

    unsafe fn set_bus_arrangements(
        &mut self,
        inputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_ins: vst3::Steinberg::int32,
        outputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_outs: vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if num_ins != 1 || num_outs != 1 {
                return vst3::Steinberg::kInvalidArgument;
            }
            match *inputs {
                vst3::Steinberg::Vst::SpeakerArr::kMono => {
                    self.channel_layout = ChannelLayout::Mono;
                    if *inputs == *outputs {
                        vst3::Steinberg::kResultTrue
                    } else {
                        vst3::Steinberg::kResultFalse
                    }
                }
                vst3::Steinberg::Vst::SpeakerArr::kStereo => {
                    self.channel_layout = ChannelLayout::Stereo;
                    if *inputs == *outputs {
                        vst3::Steinberg::kResultTrue
                    } else {
                        vst3::Steinberg::kResultFalse
                    }
                }
                _ => vst3::Steinberg::kResultFalse,
            }
        }
    }

    fn get_extra_parameters(
        &self,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone {
        core::iter::empty()
    }

    fn mpe(&self) -> Option<mpe::State> {
        None
    }
}

struct EffectProcessContext<P> {
    parameters: P,
}

impl<P: BufferStates + Clone> conformal_component::effect::ProcessContext
    for EffectProcessContext<P>
{
    fn parameters(&self) -> impl BufferStates {
        self.parameters.clone()
    }
}

struct EffectHandleParametersContext<P> {
    parameters: P,
}

impl<P: conformal_component::parameters::States + Clone>
    conformal_component::effect::HandleParametersContext for EffectHandleParametersContext<P>
{
    fn parameters(&self) -> impl conformal_component::parameters::States {
        self.parameters.clone()
    }
}

struct EffectProcessBuffer<'a, P> {
    processor: &'a mut P,
    input: UnsafeBufferFromRaw,
    output: UnsafeMutBufferFromRaw,
}

impl<P: Effect> ProcessBuffer for EffectProcessBuffer<'_, P> {
    unsafe fn process(
        &mut self,
        _e: Events<impl Iterator<Item = Event> + Clone>,
        store: &mut parameters::ProcessingStore,
        num_frames: usize,
        _mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> i32 {
        if let Some(vst_parameters) = vst_parameters {
            if let Some(buffer_states) =
                unsafe { parameters::param_changes_from_vst3(vst_parameters, store, num_frames) }
            {
                let context = EffectProcessContext {
                    parameters: buffer_states,
                };
                self.processor
                    .process(&context, &self.input, &mut self.output);
                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        } else {
            let context = EffectProcessContext {
                parameters: parameters::existing_buffer_states_from_store(store),
            };
            self.processor
                .process(&context, &self.input, &mut self.output);
            vst3::Steinberg::kResultOk
        }
    }
}

impl<P: Effect> ActiveProcessor<P> for ActiveEffectProcessor {
    type ProcessBuffer<'a>
        = EffectProcessBuffer<'a, P>
    where
        P: 'a;

    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>> {
        match self {
            ActiveEffectProcessor::AudioEnabled(channel_layout) => unsafe {
                if (*data).numOutputs != 1 {
                    return None;
                }
                if (*data).numInputs != 1 {
                    return None;
                }
                Some(EffectProcessBuffer {
                    processor,
                    input: UnsafeBufferFromRaw {
                        ptr: (*(*data).inputs).__field0.channelBuffers32,
                        channel_layout: *channel_layout,
                        num_frames: (*data).numSamples as usize,
                    },
                    output: UnsafeMutBufferFromRaw {
                        ptr: (*(*data).outputs).__field0.channelBuffers32,
                        channel_layout: *channel_layout,
                        num_frames: (*data).numSamples as usize,
                    },
                })
            },
            ActiveEffectProcessor::AudioDisabled => None,
        }
    }

    fn audio_enabled(&self) -> bool {
        match self {
            ActiveEffectProcessor::AudioEnabled(_) => true,
            ActiveEffectProcessor::AudioDisabled => false,
        }
    }

    unsafe fn handle_events(
        &self,
        processor: &mut P,
        _e: Option<impl Iterator<Item = conformal_component::events::Data> + Clone>,
        params: &mut parameters::ProcessingStore,
        _mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> vst3::Steinberg::tresult {
        if let Some(vst_parameters) = vst_parameters {
            if let Some((change_status, param_states)) =
                unsafe { parameters::no_audio_param_changes_from_vst3(vst_parameters, params) }
            {
                if change_status == parameters::ChangesStatus::Changes {
                    let context = EffectHandleParametersContext {
                        parameters: param_states,
                    };
                    processor.handle_parameters(context);
                    vst3::Steinberg::kResultOk
                } else {
                    vst3::Steinberg::kResultOk
                }
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        } else {
            vst3::Steinberg::kResultOk
        }
    }
}

struct PartialProcessingEnvironment {
    sampling_rate: f32,
    max_samples_per_process_call: usize,
    processing_mode: ProcessingMode,
}

fn make_env(
    partial: &PartialProcessingEnvironment,
    layout: ChannelLayout,
) -> ProcessingEnvironment {
    ProcessingEnvironment {
        sampling_rate: partial.sampling_rate,
        max_samples_per_process_call: partial.max_samples_per_process_call,
        processing_mode: partial.processing_mode,
        channel_layout: layout,
    }
}

/// Note that according to the VST3 spec, almost all functions must be called
/// on the main thread. The exceptions this are:
///
/// - `setProcessing`
/// - `process`
///
/// Note that according to the call diagrams [here](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Workflow+Diagrams/Audio+Processor+Call+Sequence.html),
/// `process` may only be called after `setProcessing` has returned - we further assume here
/// that `process` and `setProcessing` are never called concurrently, which
/// seems like a reasonable reading of the spec.
///
/// Note that the call diagrams in the spec also clearly says that `setActive` can
/// not be called concurrently with either `setProcessing` or `process`. Additionally,
/// `initialize` must be called _before_ `setActive(1)` and `terminate` must be called
/// _after_ `setActive(0)`.
///
/// With the assumptions in mind, we guaranetee thread-safety by bundling the state
/// needed by these two functions into a `ProcessContext` struct. This struct
/// is the _only_ state these two functions have access to, and will _only_ be accessed
/// by the following functions:
///
/// - `setActive`
/// - `process`
/// - `setProcessing`
/// - `initialize`
/// - `terminate`
///
/// Other functions can access any other fields.
struct Processor<P, C, CF, PC, APC> {
    controller_cid: ClassID,

    /// NOTE - fairly subtle why we need `Option` here - this allows `initialize` and
    /// `terminate` to be panic-safe. See [this discussion](https://users.rust-lang.org/t/how-can-i-take-and-replace-the-value-of-a-refcell/75369)
    s: RefCell<Option<State<C, CF>>>,

    /// NOTE - this could be part of `InitializedData`, but we are keeping it
    /// in a separate `RefCell` for now to make it easier in the future to
    /// use this from multiple threads.
    host: RefCell<Option<ComPtr<IHostApplication>>>,

    /// This stores data relevant to the current processor category
    /// (i.e., synth, effect, etc.). Note that this generally includes
    /// bus configuration data that may defer between categories.
    //
    /// Note that conceptually this belongs in `InitializedData`, but we
    /// keep it separate for now to be permissive to non-conforming hosts
    /// who illegally query bus info before initialization.
    category: RefCell<PC>,

    /// WATCH OUT - this member has very special rules for access.
    ///
    /// The purpose of these complicated rules is to allow thread-safe,
    /// lock free access to this data, assuming the host follows the
    /// call sequence rules.
    ///
    /// This stores data that can be accessed from the processing context.
    ///
    /// Safety (see above) - this must ONLY be accessed by the following functions:
    ///  - `setActive`
    ///  - `process`
    ///  - `setProcessing`
    ///  - `initialize`
    ///  - `terminate`
    ///
    /// Further, `process` and `setProcessing` must access _no_ other fields.
    ///
    /// In addition to this restricted access, we make sure that all uses
    /// of this member maintain the following invariants:
    ///
    /// We maintain an invariant that this will be in the Uninitialized state
    /// only when `s` is not `Initialized`
    ///
    /// We also maintain an invariant that this will be in the Inactive state
    /// only when `s` is `Initialized` and `process_context_active` is false,
    /// and that this will be in the Active state only when `s` is `Initialized`
    /// and `process_context_active` is true.
    ///
    /// These invariants let us query the state of `process_context` without
    /// accessing the member itself, which is sometimes needed to avoid
    /// violating the access rules above.
    process_context: RefCell<ProcessContext<P, APC>>,
}

impl<P, C, CF, PC, APC> Processor<P, C, CF, PC, APC> {
    fn processing_active(&self) -> bool {
        match self.s.borrow().as_ref() {
            Some(State::Initialized(InitializedData {
                process_context_active,
                ..
            })) => *process_context_active,
            _ => false,
        }
    }
}

impl<P, C, CF, PC, APC> IProcessContextRequirementsTrait for Processor<P, C, CF, PC, APC> {
    unsafe fn getProcessContextRequirements(&self) -> vst3::Steinberg::uint32 {
        // We don't need any special processing context requirements, for now!
        0
    }
}

pub fn create_synth<'a, CF: ComponentFactory<Component: Component<Processor: Synth>> + 'a>(
    factory: CF,
    controller_cid: ClassID,
) -> impl Class<
    Interfaces = (
        IPluginBase,
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IConnectionPoint,
    ),
> + IComponentTrait
+ IAudioProcessorTrait
+ IProcessContextRequirementsTrait
+ IConnectionPointTrait {
    Processor {
        controller_cid,
        s: Some(State::ReadyForInitialization(factory)).into(),
        host: Default::default(),
        process_context: Default::default(),
        category: <RefCell<SynthProcessorCategory> as Default>::default(),
    }
}

pub fn create_effect<'a, CF: ComponentFactory<Component: Component<Processor: Effect>> + 'a>(
    factory: CF,
    controller_cid: ClassID,
) -> impl Class<
    Interfaces = (
        IPluginBase,
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IConnectionPoint,
    ),
> + IComponentTrait
+ IAudioProcessorTrait
+ IProcessContextRequirementsTrait
+ IConnectionPointTrait {
    Processor {
        controller_cid,
        s: Some(State::ReadyForInitialization(factory)).into(),
        host: Default::default(),
        process_context: Default::default(),
        category: <RefCell<EffectProcessorCategory> as Default>::default(),
    }
}

impl<CF: ComponentFactory<Component: Component>, PC, APC> IPluginBaseTrait
    for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, APC>
where
    PC: ProcessorCategory,
{
    unsafe fn initialize(
        &self,
        context: *mut vst3::Steinberg::FUnknown,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if self.host.borrow().is_some() {
                return vst3::Steinberg::kInvalidArgument;
            }

            if let Some(host) = ComRef::from_raw(context).and_then(|context| context.cast()) {
                self.host.replace(Some(host));
            } else {
                return vst3::Steinberg::kInvalidArgument;
            }
        }

        let (s, res) = match (
            self.s.replace(None).unwrap(),
            host_info::get(&self.host.borrow().clone().unwrap()),
        ) {
            (State::ReadyForInitialization(factory), Some(host_info)) => {
                let conformal_component = factory.create(&host_info);
                let (params_main, params_processing) = parameters::create_stores(
                    {
                        let mut infos = conformal_component.parameter_infos();
                        infos.extend(self.category.borrow().get_extra_parameters());
                        infos
                    }
                    .iter()
                    .map(Into::into),
                );
                let s = State::Initialized(InitializedData {
                    conformal_component,
                    params_main,
                    processing_environment: None,
                    process_context_active: false,
                    factory,
                });

                // Check invariant that process_context is uninitialized here.
                assert!(
                    matches!(
                        *self.process_context.borrow(),
                        ProcessContext::Uninitialized
                    ),
                    "Invariant violation - process_context is initialized while we are not initialized"
                );

                // Safety note - this is clearly safe since `initialized` must be called
                // before `setActive(1)` according to the call sequence diagrams.
                self.process_context.replace(ProcessContext::Inactive {
                    processing: false,
                    params: params_processing,
                });
                (s, vst3::Steinberg::kResultOk)
            }
            (s, _) => (s, vst3::Steinberg::kInvalidArgument),
        };
        self.s.replace(Some(s));
        res
    }

    unsafe fn terminate(&self) -> vst3::Steinberg::tresult {
        self.host.replace(None);
        if let Some(State::Initialized(InitializedData { factory, .. })) = self.s.take() {
            // Check invariant that process_context is initialized here.
            assert!(
                !matches!(
                    *self.process_context.borrow(),
                    ProcessContext::Uninitialized
                ),
                "Invariant violation - process_context is uninitialized while we are initialized"
            );

            self.s.replace(Some(State::ReadyForInitialization(factory)));
            self.process_context.replace(ProcessContext::Uninitialized);
            vst3::Steinberg::kResultOk
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }
}

impl<CF: ComponentFactory<Component: Component<Processor: ProcessorT>>, PC: ProcessorCategory>
    IComponentTrait
    for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, PC::Active>
{
    unsafe fn getControllerClassId(
        &self,
        class_id: *mut vst3::Steinberg::TUID,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            self.controller_cid
                .iter()
                .zip((*class_id).iter_mut())
                .for_each(|(a, b)| *b = *a as i8);

            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn setIoMode(&self, _mode: vst3::Steinberg::Vst::IoMode) -> vst3::Steinberg::tresult {
        // This allows us to set up offline processing ahead of initialization.
        // See https://forums.steinberg.net/t/offline-processing/911713/6
        //
        // We currently don't use this, we'll just set up the processing mode
        // during `setupProcessing`.
        vst3::Steinberg::kResultOk
    }

    unsafe fn getBusCount(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
    ) -> vst3::Steinberg::int32 {
        unsafe { self.category.borrow().get_bus_count(rtype, dir) }
    }

    unsafe fn getBusInfo(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        bus: *mut vst3::Steinberg::Vst::BusInfo,
    ) -> vst3::Steinberg::tresult {
        unsafe { self.category.borrow().get_bus_info(rtype, dir, index, bus) }
    }

    unsafe fn getRoutingInfo(
        &self,
        __in_info: *mut vst3::Steinberg::Vst::RoutingInfo,
        __out_info: *mut vst3::Steinberg::Vst::RoutingInfo,
    ) -> vst3::Steinberg::tresult {
        // This is only really relevant for multi-bus plug-ins.
        vst3::Steinberg::kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        rtype: vst3::Steinberg::Vst::MediaType,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        state: vst3::Steinberg::TBool,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            // Note that if processing is active, it's too late to call this!
            if self.processing_active() {
                return vst3::Steinberg::kInvalidArgument;
            }
            self.category
                .borrow_mut()
                .activate_bus(rtype, dir, index, state)
        }
    }

    unsafe fn setActive(&self, state: vst3::Steinberg::TBool) -> vst3::Steinberg::tresult {
        if let Some(State::Initialized(InitializedData {
            conformal_component,
            processing_environment: Some(env),
            process_context_active,
            ..
        })) = self.s.borrow_mut().as_mut()
        {
            // Note that here we use `take`, which _temporarily_ puts `process_context`
            // in an uninitialized state. To maintain our invariant, we _must not call
            // any functions that could re-enter `self` while `process_context` is in this
            // uninitialized state!
            match (self.process_context.take(), state != 0) {
                (active @ ProcessContext::Active(_), true) => {
                    self.process_context.replace(active);
                    vst3::Steinberg::kResultOk
                }
                (inactive @ ProcessContext::Inactive { .. }, false) => {
                    self.process_context.replace(inactive);
                    vst3::Steinberg::kResultOk
                }
                (
                    ProcessContext::Active(ActiveProcessContext {
                        params, processing, ..
                    }),
                    false,
                ) => {
                    self.process_context
                        .replace(ProcessContext::Inactive { processing, params });
                    *process_context_active = false;
                    vst3::Steinberg::kResultOk
                }
                (ProcessContext::Inactive { params, processing }, true) => {
                    let active_processor = self.category.borrow().activate();
                    let mut processor = self
                        .category
                        .borrow()
                        .create_processor(conformal_component, env);
                    if processing {
                        processor.set_processing(true);
                    }
                    self.process_context
                        .replace(ProcessContext::Active(ActiveProcessContext {
                            processing,
                            params,
                            processor,
                            active_processor,
                            mpe: self.category.borrow().mpe(),
                        }));
                    *process_context_active = true;
                    vst3::Steinberg::kResultOk
                }
                (ProcessContext::Uninitialized, _) => {
                    unreachable!(
                        "Invariant violated - process_context is uninitialized while we are initialized"
                    );
                }
            }
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn setState(&self, state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        if let Some(State::Initialized(InitializedData {
            params_main: main_context_store,
            ..
        })) = self.s.borrow_mut().as_mut()
            && let Some(com_state) = unsafe { ComRef::from_raw(state) }
        {
            let read = StreamRead::new(com_state);
            if let Ok(state) = rmp_serde::from_read::<_, state::State>(read) {
                return match main_context_store.apply_snapshot(&state.params) {
                    Ok(()) => vst3::Steinberg::kResultOk,
                    Err(parameters::SnapshotError::QueueTooFull) => {
                        // Note that right now, if we can't apply the snapshot due to the
                        // snapshot queue being full, we just ignore this snapshot and
                        // indiicate we failed. Hopefully we don't hit this, often
                        // If we do we can consider more extreme measures such as stealing
                        // the processor here and forcing it to sync.
                        vst3::Steinberg::kInternalError
                    }
                    Err(parameters::SnapshotError::SnapshotCorrupted) => {
                        vst3::Steinberg::kInvalidArgument
                    }
                };
            }
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn getState(&self, state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(State::Initialized(InitializedData {
                params_main: main_context_store,
                ..
            })) = self.s.borrow().as_ref()
                && let Some(com_state) = ComRef::from_raw(state)
            {
                let writer = StreamWrite::new(com_state);
                if (state::State {
                    params: main_context_store.snapshot_with_tearing(),
                })
                .serialize(&mut rmp_serde::Serializer::new(writer))
                .is_ok()
                {
                    return vst3::Steinberg::kResultOk;
                }
                return vst3::Steinberg::kInternalError;
            }
            vst3::Steinberg::kInvalidArgument
        }
    }
}

struct UnsafeBufferFromRaw {
    ptr: *mut *mut f32,
    channel_layout: ChannelLayout,
    num_frames: usize,
}

impl Buffer for UnsafeBufferFromRaw {
    fn channel_layout(&self) -> ChannelLayout {
        self.channel_layout
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn channel(&self, channel: usize) -> &[f32] {
        unsafe { std::slice::from_raw_parts(*self.ptr.add(channel), self.num_frames) }
    }
}

struct UnsafeMutBufferFromRaw {
    ptr: *mut *mut f32,
    channel_layout: ChannelLayout,
    num_frames: usize,
}

impl Buffer for UnsafeMutBufferFromRaw {
    fn channel_layout(&self) -> ChannelLayout {
        self.channel_layout
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn channel(&self, channel: usize) -> &[f32] {
        unsafe { std::slice::from_raw_parts(*self.ptr.add(channel), self.num_frames) }
    }
}

impl BufferMut for UnsafeMutBufferFromRaw {
    fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        unsafe { std::slice::from_raw_parts_mut(*self.ptr.add(channel), self.num_frames) }
    }
}

pub const NOTE_EXPRESSION_TIMBRE_TYPE_ID: vst3::Steinberg::Vst::NoteExpressionTypeID =
    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kCustomStart;
pub const NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID: vst3::Steinberg::Vst::NoteExpressionTypeID =
    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kCustomStart + 1;

mod events;

struct SynthProcessContext<E, P> {
    events: Events<E>,
    parameters: P,
}

impl<E: Iterator<Item = Event> + Clone, P: SynthParamBufferStates + Clone>
    conformal_component::synth::ProcessContext for SynthProcessContext<E, P>
{
    fn events(&self) -> Events<impl Iterator<Item = Event> + Clone> {
        self.events.clone()
    }
    fn parameters(&self) -> impl SynthParamBufferStates {
        self.parameters.clone()
    }
}

struct SynthHandleEventsContext<E, P> {
    events: E,
    parameters: P,
}

impl<E: Iterator<Item = conformal_component::events::Data> + Clone, P: SynthParamStates + Clone>
    conformal_component::synth::HandleEventsContext for SynthHandleEventsContext<E, P>
{
    fn events(&self) -> impl Iterator<Item = conformal_component::events::Data> + Clone {
        self.events.clone()
    }
    fn parameters(&self) -> impl SynthParamStates {
        self.parameters.clone()
    }
}

struct SynthProcessBuffer<'a, P> {
    synth: &'a mut P,
    output: UnsafeMutBufferFromRaw,
}

trait ProcessBuffer {
    unsafe fn process(
        &mut self,
        e: Events<impl Iterator<Item = Event> + Clone>,
        store: &mut parameters::ProcessingStore,
        num_frames: usize,
        mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> i32;
}

impl<P: Synth> ProcessBuffer for SynthProcessBuffer<'_, P> {
    unsafe fn process(
        &mut self,
        events: Events<impl Iterator<Item = Event> + Clone>,
        store: &mut parameters::ProcessingStore,
        num_frames: usize,
        mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> vst3::Steinberg::tresult {
        // Note - we maintain the invariant that MPE state always exists for synths.
        let mpe = mpe.unwrap();

        if let Some(vst_parameters) = vst_parameters {
            if let Some(buffer_states) = unsafe {
                parameters::synth_param_changes_from_vst3(vst_parameters, store, num_frames, mpe)
            } {
                let context = SynthProcessContext {
                    events,
                    parameters: buffer_states,
                };
                self.synth.process(&context, &mut self.output);

                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        } else {
            let buffer_states =
                parameters::existing_synth_param_buffer_states_from_store(store, mpe);
            let context = SynthProcessContext {
                events,
                parameters: buffer_states.clone(),
            };
            self.synth.process(&context, &mut self.output);
            vst3::Steinberg::kResultOk
        }
    }
}

trait InternalProcessHelper<H> {
    unsafe fn do_process(
        self,
        helper: H,
        params: &mut parameters::ProcessingStore,
        data: *mut vst3::Steinberg::Vst::ProcessData,
        mpe: Option<&mut mpe::State>,
        num_frames: usize,
    ) -> vst3::Steinberg::tresult;
}

impl<H: ProcessBuffer, I: Iterator<Item = conformal_component::events::Event> + Clone>
    InternalProcessHelper<H> for Events<I>
{
    unsafe fn do_process(
        self,
        mut helper: H,
        params: &mut parameters::ProcessingStore,
        data: *mut vst3::Steinberg::Vst::ProcessData,
        mpe: Option<&mut mpe::State>,
        num_frames: usize,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            helper.process(
                self,
                params,
                num_frames,
                mpe,
                vst3::ComRef::from_raw((*data).inputParameterChanges),
            )
        }
    }
}

struct NoAudioProcessHelper<'a, P, C> {
    processor: &'a mut P,
    events_empty: bool,
    active_processor: &'a C,
}

impl<
    'a,
    P: ProcessorT,
    Iter: Iterator<Item = conformal_component::events::Data> + Clone,
    C: ActiveProcessor<P>,
> InternalProcessHelper<NoAudioProcessHelper<'a, P, C>> for Iter
{
    unsafe fn do_process(
        self,
        helper: NoAudioProcessHelper<'a, P, C>,
        params: &mut parameters::ProcessingStore,
        data: *mut vst3::Steinberg::Vst::ProcessData,
        mpe: Option<&mut mpe::State>,
        _: usize,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            helper.active_processor.handle_events(
                helper.processor,
                (!helper.events_empty).then_some(self),
                params,
                mpe,
                vst3::ComRef::from_raw((*data).inputParameterChanges),
            )
        }
    }
}

impl<P: Synth> ActiveProcessor<P> for ActiveSynthProcessor {
    type ProcessBuffer<'a>
        = SynthProcessBuffer<'a, P>
    where
        P: 'a;

    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>> {
        match self {
            ActiveSynthProcessor::AudioEnabled(channel_layout) => unsafe {
                if (*data).numOutputs != 1 {
                    return None;
                }

                Some(SynthProcessBuffer {
                    synth: processor,
                    output: UnsafeMutBufferFromRaw {
                        ptr: (*(*data).outputs).__field0.channelBuffers32,
                        channel_layout: *channel_layout,
                        num_frames: (*data).numSamples as usize,
                    },
                })
            },
            ActiveSynthProcessor::AudioDisabled => None,
        }
    }

    fn audio_enabled(&self) -> bool {
        match self {
            ActiveSynthProcessor::AudioEnabled(_) => true,
            ActiveSynthProcessor::AudioDisabled => false,
        }
    }

    unsafe fn handle_events(
        &self,
        processor: &mut P,
        e: Option<impl Iterator<Item = conformal_component::events::Data> + Clone>,
        params: &mut parameters::ProcessingStore,
        mpe: Option<&mut mpe::State>,
        vst_parameters: Option<ComRef<'_, vst3::Steinberg::Vst::IParameterChanges>>,
    ) -> vst3::Steinberg::tresult {
        let mpe = mpe.unwrap();
        if let Some(vst_parameters) = vst_parameters {
            if let Some((change_status, param_states)) = unsafe {
                parameters::no_audio_synth_param_changes_from_vst3(vst_parameters, params, mpe)
            } {
                if change_status == parameters::ChangesStatus::Changes || e.is_some() {
                    if let Some(e) = e {
                        {
                            let context = SynthHandleEventsContext {
                                events: e,
                                parameters: param_states,
                            };
                            processor.handle_events(context);
                        }
                    } else {
                        {
                            let context = SynthHandleEventsContext {
                                events: std::iter::empty(),
                                parameters: param_states,
                            };
                            processor.handle_events(context);
                        }
                    }
                }
            } else {
                return vst3::Steinberg::kInvalidArgument;
            }
        } else if let Some(e) = e {
            let param_states = unsafe { parameters::existing_synth_params(params, mpe) };
            let context = SynthHandleEventsContext {
                events: e.clone(),
                parameters: param_states.clone(),
            };
            processor.handle_events(context);
        }
        vst3::Steinberg::kResultOk
    }
}

impl<
    CF: ComponentFactory<Component: Component<Processor: ProcessorT>>,
    PC: ProcessorCategory<Active: ActiveProcessor<<CF::Component as Component>::Processor>>,
> IAudioProcessorTrait
    for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, PC::Active>
{
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_ins: vst3::Steinberg::int32,
        outputs: *mut vst3::Steinberg::Vst::SpeakerArrangement,
        num_outs: vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(State::Initialized(InitializedData {
                process_context_active,
                ..
            })) = self.s.borrow().as_ref()
            {
                // This isn't legal if we're active!
                if *process_context_active {
                    return vst3::Steinberg::kInvalidArgument;
                }
                self.category
                    .borrow_mut()
                    .set_bus_arrangements(inputs, num_ins, outputs, num_outs)
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }

    unsafe fn getBusArrangement(
        &self,
        dir: vst3::Steinberg::Vst::BusDirection,
        index: vst3::Steinberg::int32,
        arr: *mut vst3::Steinberg::Vst::SpeakerArrangement,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(State::Initialized(_)) = self.s.borrow().as_ref() {
                self.category.borrow().get_bus_arrangement(dir, index, arr)
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }

    unsafe fn canProcessSampleSize(
        &self,
        symbolic_sample_size: vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        if symbolic_sample_size as u32 == vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 {
            vst3::Steinberg::kResultTrue
        } else {
            vst3::Steinberg::kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> vst3::Steinberg::uint32 {
        0
    }

    unsafe fn setupProcessing(
        &self,
        setup: *mut vst3::Steinberg::Vst::ProcessSetup,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(State::Initialized(InitializedData {
                processing_environment,
                process_context_active,
                ..
            })) = self.s.borrow_mut().as_mut()
            {
                // This isn't legal if we're active!
                if *process_context_active {
                    return vst3::Steinberg::kInvalidArgument;
                }
                *processing_environment = (PartialProcessingEnvironment {
                    // Note that we lose some resolution on the sample rate here, but
                    // it's okay.
                    #[allow(clippy::cast_possible_truncation)]
                    sampling_rate: (*setup).sampleRate as f32,

                    max_samples_per_process_call: (*setup).maxSamplesPerBlock as usize,
                    processing_mode: match (*setup).processMode as u32 {
                        vst3::Steinberg::Vst::ProcessModes_::kRealtime => ProcessingMode::Realtime,
                        vst3::Steinberg::Vst::ProcessModes_::kPrefetch => ProcessingMode::Prefetch,
                        vst3::Steinberg::Vst::ProcessModes_::kOffline => ProcessingMode::Offline,
                        _ => unreachable!(),
                    },
                })
                .into();

                vst3::Steinberg::kResultOk
            } else {
                // We must be initialized!
                vst3::Steinberg::kInvalidArgument
            }
        }
    }

    /// Safety - must _only_ access `self.process_context`, and no other members!
    unsafe fn setProcessing(&self, state: vst3::Steinberg::TBool) -> vst3::Steinberg::tresult {
        if let ProcessContext::Active(ref mut pd) = *self.process_context.borrow_mut() {
            if (state != 0) != pd.processing {
                pd.processing = state != 0;
                pd.processor.set_processing(pd.processing);
            }
            vst3::Steinberg::kResultOk
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    /// Safety - must _only_ access `self.process_context`, and no other members!
    unsafe fn process(
        &self,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let ProcessContext::Active(ref mut pd) = *self.process_context.borrow_mut() {
                if !pd.processing {
                    return vst3::Steinberg::kInvalidArgument;
                }

                pd.params.sync_from_main_thread();
                let num_frames = (*data).numSamples as usize;

                if num_frames == 0 || !pd.active_processor.audio_enabled() {
                    if let Some(input_events) = ComRef::from_raw((*data).inputEvents) {
                        if let Some(event_iter) = events::all_zero_event_iterator(input_events) {
                            let helper = NoAudioProcessHelper {
                                processor: &mut pd.processor,
                                events_empty: event_iter.clone().next().is_none(),
                                active_processor: &pd.active_processor,
                            };
                            return event_iter.do_process(
                                helper,
                                &mut pd.params,
                                data,
                                pd.mpe.as_mut(),
                                0,
                            );
                        }
                    } else {
                        let helper = NoAudioProcessHelper {
                            processor: &mut pd.processor,
                            events_empty: true,
                            active_processor: &pd.active_processor,
                        };
                        return std::iter::empty().do_process(
                            helper,
                            &mut pd.params,
                            data,
                            pd.mpe.as_mut(),
                            0,
                        );
                    }
                    // If we got here, some pre-condition of the parameters was not met by the host
                    return vst3::Steinberg::kInvalidArgument;
                }
                if (*data).symbolicSampleSize
                    != vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32
                {
                    return vst3::Steinberg::kInvalidArgument;
                }

                if let Some(process_buffer) = pd
                    .active_processor
                    .make_process_buffer(&mut pd.processor, data)
                {
                    if let Some(input_events) = ComRef::from_raw((*data).inputEvents) {
                        if let Some(events) =
                            Events::new(events::event_iterator(input_events), num_frames)
                        {
                            return events.do_process(
                                process_buffer,
                                &mut pd.params,
                                data,
                                pd.mpe.as_mut(),
                                num_frames,
                            );
                        }
                    } else {
                        return Events::new(std::iter::empty(), num_frames)
                            .unwrap()
                            .do_process(
                                process_buffer,
                                &mut pd.params,
                                data,
                                pd.mpe.as_mut(),
                                num_frames,
                            );
                    }
                }
            }
            // If we got here, some invariant was not met by the host (i.e., Events was misformated, or wrong audio format.)
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getTailSamples(&self) -> vst3::Steinberg::uint32 {
        0
    }
}

impl<CF: ComponentFactory<Component: Component<Processor: ProcessorT>>, PC: ProcessorCategory>
    IConnectionPointTrait
    for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, PC::Active>
{
    unsafe fn connect(&self, _: *mut IConnectionPoint) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }

    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }

    unsafe fn notify(
        &self,
        _message: *mut vst3::Steinberg::Vst::IMessage,
    ) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultOk
    }
}

impl<
    CF: ComponentFactory<Component: Component>,
    PC: ProcessorCategory<Active: ActiveProcessor<<CF::Component as Component>::Processor>>,
> Class for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, PC::Active>
{
    type Interfaces = (
        IPluginBase,
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IConnectionPoint,
    );
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

    use conformal_component::effect::Effect;
    use conformal_component::synth::{
        NumericGlobalExpression, NumericPerNoteExpression, SynthParamBufferStates,
    };
    use vst3::ComWrapper;
    use vst3::Steinberg::{
        IBStreamTrait, IPluginBaseTrait,
        Vst::{IAudioProcessorTrait, IComponentTrait, IHostApplication},
    };

    use super::test_utils::{DEFAULT_ENV, activate_busses, process_setup, setup_proc};
    use super::{PartialProcessingEnvironment, create_effect, create_synth};
    use crate::HostInfo;
    use crate::fake_ibstream::Stream;
    use crate::mpe::quirks::aftertouch_param_id;
    use crate::processor::test_utils::{
        MockData, MockEvent, ParameterValueQueueImpl, ParameterValueQueuePoint, SAMPLE_COUNT,
        activate_effect_busses, mock_no_audio_process_data, mock_process, mock_process_effect,
        mock_process_mod, setup_proc_effect,
    };
    use crate::{dummy_host, from_utf16_buffer};
    use assert_approx_eq::assert_approx_eq;
    use conformal_component::audio::{BufferMut, channels, channels_mut};
    use conformal_component::events::{Data, NoteID, NoteIDInternals};
    use conformal_component::parameters::{
        BufferStates, Flags, InfoRef, StaticInfoRef, TypeSpecificInfoRef,
    };
    use conformal_component::parameters::{enum_per_sample, numeric_per_sample, switch_per_sample};
    use conformal_component::{
        Component, ProcessingEnvironment, ProcessingMode, Processor, synth::Synth,
    };

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
                units: Some("Hz"),
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
        fn handle_events(&mut self, context: impl conformal_component::synth::HandleEventsContext) {
            for event in context.events() {
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

        fn process(
            &mut self,
            context: &impl conformal_component::synth::ProcessContext,
            output: &mut impl BufferMut,
        ) {
            let events = context.events();
            let parameters = context.parameters();
            let mut events_iter = events.into_iter();
            let mut next_event = events_iter.next();

            let mult_iter = numeric_per_sample(parameters.get_numeric(NUMERIC_ID).unwrap());
            let enum_iter = enum_per_sample(parameters.get_enum(ENUM_ID).unwrap());
            let switch_iter = switch_per_sample(parameters.get_switch(SWITCH_ID).unwrap());
            let global_pitch_bend_iter = numeric_per_sample(
                parameters.get_numeric_global_expression(NumericGlobalExpression::PitchBend),
            );
            #[allow(clippy::iter_skip_zero)]
            let mut mpe_pitchbend_iter =
                self.notes.iter().next().map(|note| {
                    numeric_per_sample(parameters.get_numeric_expression_for_note(
                        NumericPerNoteExpression::PitchBend,
                        *note,
                    ))
                    .skip(0)
                });
            #[allow(clippy::iter_skip_zero)]
            let mut mpe_timbre_iter = self.notes.iter().next().map(|note| {
                numeric_per_sample(
                    parameters
                        .get_numeric_expression_for_note(NumericPerNoteExpression::Timbre, *note),
                )
                .skip(0)
            });
            #[allow(clippy::iter_skip_zero)]
            let mut mpe_aftertouch_iter =
                self.notes.iter().next().map(|note| {
                    numeric_per_sample(parameters.get_numeric_expression_for_note(
                        NumericPerNoteExpression::Aftertouch,
                        *note,
                    ))
                    .skip(0)
                });

            for ((((frame_index, mult), enum_mult), switch_mult), global_pich_bend) in (0..output
                .num_frames())
                .zip(mult_iter)
                .zip(enum_iter)
                .zip(switch_iter)
                .zip(global_pitch_bend_iter)
            {
                while let Some(event) = &next_event {
                    if event.sample_offset == frame_index {
                        match event.data {
                            Data::NoteOn { ref data } => {
                                self.notes.insert(data.id);
                                mpe_pitchbend_iter = Some(
                                    numeric_per_sample(parameters.get_numeric_expression_for_note(
                                        NumericPerNoteExpression::PitchBend,
                                        data.id,
                                    ))
                                    .skip(frame_index),
                                );
                                mpe_timbre_iter = Some(
                                    numeric_per_sample(parameters.get_numeric_expression_for_note(
                                        NumericPerNoteExpression::Timbre,
                                        data.id,
                                    ))
                                    .skip(frame_index),
                                );
                                mpe_aftertouch_iter = Some(
                                    numeric_per_sample(parameters.get_numeric_expression_for_note(
                                        NumericPerNoteExpression::Aftertouch,
                                        data.id,
                                    ))
                                    .skip(frame_index),
                                );
                            }
                            Data::NoteOff { ref data } => {
                                self.notes.remove(&data.id);
                                mpe_pitchbend_iter = None;
                                mpe_timbre_iter = None;
                                mpe_aftertouch_iter = None;
                            }
                        }
                        next_event = events_iter.next();
                    } else {
                        break;
                    }
                }
                let mpe_pitchbend = mpe_pitchbend_iter
                    .as_mut()
                    .and_then(Iterator::next)
                    .unwrap_or(0.0);
                let mpe_timbre = mpe_timbre_iter
                    .as_mut()
                    .and_then(Iterator::next)
                    .unwrap_or(0.0);
                let mpe_aftertouch = mpe_aftertouch_iter
                    .as_mut()
                    .and_then(Iterator::next)
                    .unwrap_or(0.0);
                for channel in channels_mut(output) {
                    channel[frame_index] = mult
                        * ((enum_mult + 1) as f32)
                        * (if switch_mult { 1.0 } else { 0.0 })
                        * self.notes.len() as f32
                        + mpe_pitchbend
                        + mpe_timbre
                        + mpe_aftertouch
                        + global_pich_bend;
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

        fn parameter_infos(&self) -> Vec<conformal_component::parameters::Info> {
            conformal_component::parameters::to_infos(&PARAMETERS)
        }
    }

    fn dummy_synth() -> impl IComponentTrait + IAudioProcessorTrait {
        create_synth(
            |_: &HostInfo| -> FakeSynthComponent<'static> { Default::default() },
            [4; 16],
        )
    }

    fn dummy_synth_with_processing_environment(
        env: &RefCell<Option<ProcessingEnvironment>>,
    ) -> impl IAudioProcessorTrait + IComponentTrait {
        create_synth(
            |_: &HostInfo| FakeSynthComponent {
                last_process_env: Some(env),
                processing: None,
            },
            [4; 16],
        )
    }

    fn dummy_synth_with_host_info(
        host_info: &RefCell<Option<HostInfo>>,
    ) -> impl IAudioProcessorTrait + IComponentTrait {
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

    fn dummy_synth_with_processing(
        env: &RefCell<bool>,
    ) -> impl IAudioProcessorTrait + IComponentTrait {
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
        fn handle_parameters(
            &mut self,
            _context: impl conformal_component::effect::HandleParametersContext,
        ) {
        }

        fn process(
            &mut self,
            context: &impl conformal_component::effect::ProcessContext,
            input: &impl conformal_component::audio::Buffer,
            output: &mut impl conformal_component::audio::BufferMut,
        ) {
            let parameters = context.parameters();
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

        fn parameter_infos(&self) -> Vec<conformal_component::parameters::Info> {
            conformal_component::parameters::to_infos(&PARAMETERS)
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
                proc.canProcessSampleSize(
                    vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32
                ),
                vst3::Steinberg::kResultTrue
            );
        }
    }

    #[test]
    fn cannot_process_f64() {
        let proc = dummy_synth();

        unsafe {
            assert_eq!(
                proc.canProcessSampleSize(
                    vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample64 as i32
                ),
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
    fn allows_activating_without_activating_busses() {
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

            assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
        }
    }

    #[test]
    fn allows_effect_activating_without_activating_busses() {
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

            assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
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
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 100,
                        data: MockData::NoteOff {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 200,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 200,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(1),
                            },
                            pitch: 65,
                            velocity: 0.5,
                            tuning: 0f32,
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
    fn can_process_mpe() {
        let proc = dummy_synth();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            setup_proc(&proc, &host);

            let audio = mock_process(
                2,
                vec![
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 10,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::PitchBend,
                            value: 12.0,
                        },
                    },
                    // Should ignore wrong-id events
                    MockEvent {
                        sample_offset: 11,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(4),
                            },
                            expression: NumericPerNoteExpression::PitchBend,
                            value: 24.0,
                        },
                    },
                    MockEvent {
                        sample_offset: 16,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::Timbre,
                            value: 0.6,
                        },
                    },
                    MockEvent {
                        sample_offset: 20,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::Aftertouch,
                            value: 0.7,
                        },
                    },
                    MockEvent {
                        sample_offset: 90,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::PitchBend,
                            value: 0.0,
                        },
                    },
                    MockEvent {
                        sample_offset: 90,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::Timbre,
                            value: 0.0,
                        },
                    },
                    MockEvent {
                        sample_offset: 90,
                        data: MockData::NoteExpressionChange {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            expression: NumericPerNoteExpression::Aftertouch,
                            value: 0.0,
                        },
                    },
                    MockEvent {
                        sample_offset: 100,
                        data: MockData::NoteOff {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                ],
                vec![],
                &proc,
            );
            assert!(audio.is_some());
            assert_eq!(audio.as_ref().unwrap()[0][0], 1.0);
            assert_approx_eq!(audio.as_ref().unwrap()[0][10], 13.0, 1e-5);
            assert_approx_eq!(audio.as_ref().unwrap()[0][15], 13.0, 1e-5);
            assert_approx_eq!(audio.as_ref().unwrap()[0][16], 13.6, 1e-5);
            assert_approx_eq!(audio.as_ref().unwrap()[0][20], 14.3, 1e-5);
            assert_approx_eq!(audio.as_ref().unwrap()[0][90], 1.0);
            assert_eq!(audio.as_ref().unwrap()[1][100], 0.0);
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
                        param_id: NUMERIC_ID.to_string(),
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
                        param_id: SWITCH_ID.to_string(),
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
                        param_id: ENUM_ID.to_string(),
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
            assert!(
                mock_process(
                    2,
                    vec![MockEvent {
                        sample_offset: SAMPLE_COUNT + 1000,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    }],
                    vec![],
                    &proc
                )
                .is_none()
            );
        }
    }

    #[test]
    fn defends_against_shuffled_events() {
        let proc = dummy_synth();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            setup_proc(&proc, &host);

            assert!(
                mock_process(
                    2,
                    vec![
                        MockEvent {
                            sample_offset: 200,
                            data: MockData::NoteOn {
                                id: NoteID {
                                    internals: NoteIDInternals::NoteIDWithID(0),
                                },
                                pitch: 64,
                                velocity: 0.5,
                                tuning: 0f32,
                            },
                        },
                        MockEvent {
                            sample_offset: 100,
                            data: MockData::NoteOff {
                                id: NoteID {
                                    internals: NoteIDInternals::NoteIDWithID(0),
                                },
                                pitch: 64,
                                velocity: 0.5,
                                tuning: 0f32,
                            },
                        },
                    ],
                    vec![],
                    &proc
                )
                .is_none()
            );
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
                vec![MockEvent {
                    sample_offset: 100,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(1),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                ],
                vec![],
                &proc,
            );

            assert!(audio.is_some());

            let mut data2 = mock_no_audio_process_data(
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOff {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
                vec![MockEvent {
                    sample_offset: 100,
                    data: MockData::NoteOff {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(1),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
    fn can_handle_events_when_activated_without_audio() {
        let proc = dummy_synth();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            let host_ref = host.as_com_ref::<IHostApplication>().unwrap();
            assert_eq!(
                proc.initialize(host_ref.cast().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                proc.setupProcessing(&mut process_setup(&DEFAULT_ENV)),
                vst3::Steinberg::kResultOk
            );
            let mut out_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
            assert_eq!(
                proc.setBusArrangements(std::ptr::null_mut(), 0, &mut out_arrangement, 1),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
            assert_eq!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);

            let audio = mock_process(
                2,
                vec![
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(0),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                    MockEvent {
                        sample_offset: 0,
                        data: MockData::NoteOn {
                            id: NoteID {
                                internals: NoteIDInternals::NoteIDWithID(1),
                            },
                            pitch: 64,
                            velocity: 0.5,
                            tuning: 0f32,
                        },
                    },
                ],
                vec![],
                &proc,
            );
            assert!(audio.is_some());
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![
                    ParameterValueQueueImpl {
                        param_id: NUMERIC_ID.to_string(),
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
                        param_id: SWITCH_ID.to_string(),
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
                        param_id: ENUM_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: NUMERIC_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: "some garbage parameter".to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: NUMERIC_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: NUMERIC_ID.to_string(),
                    points: vec![
                        ParameterValueQueuePoint {
                            sample_offset: 100,
                            value: (1.0 - MIN_NUMERIC as f64)
                                / ((MAX_NUMERIC - MIN_NUMERIC) as f64),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: NUMERIC_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: NUMERIC_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![
                    ParameterValueQueueImpl {
                        param_id: NUMERIC_ID.to_string(),
                        points: vec![ParameterValueQueuePoint {
                            sample_offset: 99,
                            value: ((1.0 - MIN_NUMERIC) / (MAX_NUMERIC - MIN_NUMERIC)) as f64,
                        }],
                    },
                    ParameterValueQueueImpl {
                        param_id: NUMERIC_ID.to_string(),
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
                    param_id: NUMERIC_ID.to_string(),
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
                    param_id: NUMERIC_ID.to_string(),
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
                                param_id: NUMERIC_ID.to_string(),
                                points: vec![ParameterValueQueuePoint {
                                    sample_offset: 0,
                                    value: 1.0,
                                }],
                            },
                            ParameterValueQueueImpl {
                                param_id: ENUM_ID.to_string(),
                                points: vec![ParameterValueQueuePoint {
                                    sample_offset: 0,
                                    value: 0.5,
                                }],
                            },
                            ParameterValueQueueImpl {
                                param_id: SWITCH_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 0,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: SWITCH_ID.to_string(),
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
                        vec![
                            ParameterValueQueueImpl {
                                param_id: NUMERIC_ID.to_string(),
                                points: vec![ParameterValueQueuePoint {
                                    sample_offset: 0,
                                    value: 1.0,
                                }],
                            },
                            // Test that pitch bend parameter is _not_ saved
                            ParameterValueQueueImpl {
                                param_id: crate::parameters::PITCH_BEND_PARAMETER.to_string(),
                                points: vec![ParameterValueQueuePoint {
                                    sample_offset: 0,
                                    value: 1.0,
                                }],
                            }
                        ],
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
                vec![MockEvent {
                    sample_offset: 10,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
                            param_id: NUMERIC_ID.to_string(),
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
                            param_id: ENUM_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 10,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
            units: Some("Hz"),
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
        fn handle_events(
            &mut self,
            _context: impl conformal_component::synth::HandleEventsContext,
        ) {
        }

        fn process(
            &mut self,
            _context: &impl conformal_component::synth::ProcessContext,
            _output: &mut impl BufferMut,
        ) {
        }
    }

    impl Component for IncompatibleComponent {
        type Processor = IncompatibleSynth;

        fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
            Default::default()
        }

        fn parameter_infos(&self) -> Vec<conformal_component::parameters::Info> {
            conformal_component::parameters::to_infos(&INCOMPATIBLE_PARAMETERS)
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
        fn handle_events(
            &mut self,
            _context: impl conformal_component::synth::HandleEventsContext,
        ) {
        }

        fn process(
            &mut self,
            _context: &impl conformal_component::synth::ProcessContext,
            _output: &mut impl BufferMut,
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
                units: Some("Hz"),
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

        fn parameter_infos(&self) -> Vec<conformal_component::parameters::Info> {
            conformal_component::parameters::to_infos(&NEWER_PARAMETERS)
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
                            param_id: NUMERIC_ID.to_string(),
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
                            param_id: NUMERIC_ID.to_string(),
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
                vec![MockEvent {
                    sample_offset: 10,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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
                units: Some("Hz"),
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
                units: Some("Hz"),
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

        fn parameter_infos(&self) -> Vec<conformal_component::parameters::Info> {
            conformal_component::parameters::to_infos(&DUPLICATE_PARAMETERS)
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
                vec![MockEvent {
                    sample_offset: 100,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDWithID(0),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
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

    #[test]
    fn supports_mpe_quirks() {
        let proc = dummy_synth();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            setup_proc(&proc, &host);

            let audio = mock_process(
                2,
                vec![MockEvent {
                    sample_offset: 10,
                    data: MockData::NoteOn {
                        id: NoteID {
                            internals: NoteIDInternals::NoteIDFromChannelID(1),
                        },
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0f32,
                    },
                }],
                vec![ParameterValueQueueImpl {
                    param_id: aftertouch_param_id(1).to_string(),
                    points: vec![ParameterValueQueuePoint {
                        sample_offset: 10,
                        value: 1.0,
                    }],
                }],
                &proc,
            );

            assert_approx_eq!(audio.as_ref().unwrap()[0][10], 2.0);
        }
    }

    #[test]
    fn supports_mpe_quirks_no_audio() {
        let proc = dummy_synth();
        let host = ComWrapper::new(dummy_host::Host::default());

        unsafe {
            setup_proc(&proc, &host);
            assert_eq!(
                proc.process(
                    &mut mock_no_audio_process_data(
                        vec![MockEvent {
                            sample_offset: 0,
                            data: MockData::NoteOn {
                                id: NoteID {
                                    internals: NoteIDInternals::NoteIDFromChannelID(1),
                                },
                                pitch: 64,
                                velocity: 0.5,
                                tuning: 0f32,
                            },
                        }],
                        vec![],
                    )
                    .process_data
                ),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                proc.process(
                    &mut mock_no_audio_process_data(
                        vec![],
                        vec![ParameterValueQueueImpl {
                            param_id: aftertouch_param_id(1).to_string(),
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

            let audio = mock_process(2, vec![], vec![], &proc);

            assert_approx_eq!(audio.as_ref().unwrap()[0][10], 2.0);
        }
    }
}
