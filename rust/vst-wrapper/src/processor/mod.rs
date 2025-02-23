//! The VST3 processor implementation.

use std::cell::RefCell;

use crate::mpe_quirks::{
    self, add_mpe_quirk_events_buffer, add_mpe_quirk_events_no_audio,
    update_mpe_quirk_events_buffer, update_mpe_quirk_events_no_audio, Support,
};
use crate::{ClassID, ComponentFactory, HostInfo};
use conformal_component::audio::{Buffer, BufferMut, ChannelLayout};
use conformal_component::effect::Effect;
use conformal_component::events::{Event, Events};
use conformal_component::parameters::BufferStates;
use conformal_component::synth::{Synth, CONTROLLER_PARAMETERS};
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
mod tests;

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
    category: A,

    /// If we support hosts with MPE Quirks, the current state for MPE quirks.
    mpe_quirks: Option<mpe_quirks::State>,
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

        /// Whether we support host quirks for MPE note expression.
        /// see [`crate::mpe_quirks`] for more details.
        support_mpe_quirks: Support,
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

struct ActiveSynthProcessorCategory {
    channel_layout: ChannelLayout,
}

impl Default for SynthProcessorCategory {
    fn default() -> Self {
        SynthProcessorCategory {
            channel_layout: ChannelLayout::Stereo,
            bus_activation_state: Default::default(),
        }
    }
}

trait ActiveProcessorCategory<P> {
    type ProcessBuffer<'a>: ProcessBuffer
    where
        P: 'a,
        Self: 'a;
    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>>;

    fn handle_events<
        E: Iterator<Item = conformal_component::events::Data> + Clone,
        Parameters: conformal_component::parameters::States,
    >(
        &self,
        processor: &mut P,
        e: E,
        p: Parameters,
    );
}

trait ProcessorCategory {
    type Active;

    fn activate(&self) -> Option<Self::Active>;

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
        host_info: &HostInfo,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone;
}

impl ProcessorCategory for SynthProcessorCategory {
    type Active = ActiveSynthProcessorCategory;

    fn activate(&self) -> Option<Self::Active> {
        // We can only be activated if all our buses are active.
        if self.bus_activation_state.event_input_active
            && self.bus_activation_state.audio_output_active
        {
            Some(ActiveSynthProcessorCategory {
                channel_layout: self.channel_layout,
            })
        } else {
            None
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
        host_info: &HostInfo,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone {
        CONTROLLER_PARAMETERS.iter().map(Into::into).chain(
            mpe_quirks::parameters()
                .filter(|_| mpe_quirks::should_support(host_info) == Support::SupportQuirks),
        )
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
struct ActiveEffectProcessorCategory {
    channel_layout: ChannelLayout,
}

impl ProcessorCategory for EffectProcessorCategory {
    type Active = ActiveEffectProcessorCategory;

    fn activate(&self) -> Option<Self::Active> {
        // We can only be activated if all our buses are active.
        if self.bus_activation_state.audio_input_active
            && self.bus_activation_state.audio_output_active
        {
            Some(ActiveEffectProcessorCategory {
                channel_layout: self.channel_layout,
            })
        } else {
            None
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
        _: &HostInfo,
    ) -> impl Iterator<Item = conformal_component::parameters::Info> + Clone {
        core::iter::empty()
    }
}

struct EffectProcessBuffer<'a, P> {
    processor: &'a mut P,
    input: UnsafeBufferFromRaw,
    output: UnsafeMutBufferFromRaw,
}

impl<P: Effect> ProcessBuffer for EffectProcessBuffer<'_, P> {
    fn process<E: IntoIterator<Item = Event> + Clone, Parameters: BufferStates>(
        &mut self,
        _e: Events<E>,
        p: Parameters,
    ) {
        self.processor.process(p, &self.input, &mut self.output);
    }
}

impl<P: Effect> ActiveProcessorCategory<P> for ActiveEffectProcessorCategory {
    type ProcessBuffer<'a>
        = EffectProcessBuffer<'a, P>
    where
        P: 'a;

    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>> {
        unsafe {
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
                    channel_layout: self.channel_layout,
                    num_frames: (*data).numSamples as usize,
                },
                output: UnsafeMutBufferFromRaw {
                    ptr: (*(*data).outputs).__field0.channelBuffers32,
                    channel_layout: self.channel_layout,
                    num_frames: (*data).numSamples as usize,
                },
            })
        }
    }

    fn handle_events<
        E: IntoIterator<Item = conformal_component::events::Data> + Clone,
        Parameters: conformal_component::parameters::States,
    >(
        &self,
        processor: &mut P,
        _e: E,
        p: Parameters,
    ) {
        processor.handle_parameters(p);
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
       + IConnectionPointTrait
       + 'a {
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
       + IConnectionPointTrait
       + 'a {
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
                        infos.extend(self.category.borrow().get_extra_parameters(&host_info));
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
                assert!(matches!(
                    *self.process_context.borrow(),
                    ProcessContext::Uninitialized
                ), "Invariant violation - process_context is initialized while we are not initialized");

                // Safety note - this is clearly safe since `initialized` must be called
                // before `setActive(1)` according to the call sequence diagrams.
                self.process_context.replace(ProcessContext::Inactive {
                    processing: false,
                    params: params_processing,
                    support_mpe_quirks: mpe_quirks::should_support(&host_info),
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
                        params,
                        processing,
                        mpe_quirks,
                        ..
                    }),
                    false,
                ) => {
                    self.process_context.replace(ProcessContext::Inactive {
                        processing,
                        params,
                        support_mpe_quirks: if mpe_quirks.is_some() {
                            Support::SupportQuirks
                        } else {
                            Support::DoNotSupportQuirks
                        },
                    });
                    *process_context_active = false;
                    vst3::Steinberg::kResultOk
                }
                (
                    ProcessContext::Inactive {
                        params,
                        processing,
                        support_mpe_quirks,
                    },
                    true,
                ) => {
                    if let Some(category) = self.category.borrow().activate() {
                        let mut processor = self
                            .category
                            .borrow()
                            .create_processor(conformal_component, env);
                        if processing {
                            processor.set_processing(true);
                        }
                        self.process_context.replace(ProcessContext::Active(
                            ActiveProcessContext {
                                processing,
                                params,
                                processor,
                                category,
                                mpe_quirks: if support_mpe_quirks == Support::SupportQuirks {
                                    Some(Default::default())
                                } else {
                                    None
                                },
                            },
                        ));
                        *process_context_active = true;
                        vst3::Steinberg::kResultOk
                    } else {
                        self.process_context.replace(ProcessContext::Inactive {
                            processing,
                            params,
                            support_mpe_quirks,
                        });
                        vst3::Steinberg::kInvalidArgument
                    }
                }
                (ProcessContext::Uninitialized, _) => {
                    unreachable!("Invariant violated - process_context is uninitialized while we are initialized");
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
        {
            if let Some(com_state) = ComRef::from_raw(state) {
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
        }
        vst3::Steinberg::kInvalidArgument
    }

    unsafe fn getState(&self, state: *mut vst3::Steinberg::IBStream) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(State::Initialized(InitializedData {
                params_main: main_context_store,
                ..
            })) = self.s.borrow().as_ref()
            {
                if let Some(com_state) = ComRef::from_raw(state) {
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

struct SynthProcessBuffer<'a, P> {
    synth: &'a mut P,
    output: UnsafeMutBufferFromRaw,
}

trait ProcessBuffer {
    fn process<E: Iterator<Item = Event> + Clone, P: BufferStates>(&mut self, e: Events<E>, p: P);
}

impl<P: Synth> ProcessBuffer for SynthProcessBuffer<'_, P> {
    fn process<E: Iterator<Item = Event> + Clone, Parameters: BufferStates>(
        &mut self,
        e: Events<E>,
        p: Parameters,
    ) {
        self.synth.process(e, p, &mut self.output);
    }
}

trait InternalProcessHelper<H> {
    unsafe fn do_process(
        self,
        helper: H,
        params: &mut parameters::ProcessingStore,
        data: *mut vst3::Steinberg::Vst::ProcessData,
        mpe_quirks: Option<&mut mpe_quirks::State>,
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
        mpe_quirks: Option<&mut mpe_quirks::State>,
        num_frames: usize,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(com_changes) = vst3::ComRef::from_raw((*data).inputParameterChanges) {
                if let Some(buffer_states) =
                    parameters::param_changes_from_vst3(com_changes, params, num_frames)
                {
                    if let Some(mpe_quirks) = mpe_quirks {
                        let buffer_states_clone = buffer_states.clone();
                        let events = add_mpe_quirk_events_buffer(
                            self.clone().into_iter(),
                            mpe_quirks.clone(),
                            &buffer_states_clone,
                            num_frames,
                        );
                        helper.process(events, buffer_states.clone());
                        update_mpe_quirk_events_buffer(
                            self.into_iter(),
                            mpe_quirks,
                            &buffer_states,
                        );
                    } else {
                        helper.process(self, buffer_states);
                    }
                    vst3::Steinberg::kResultOk
                } else {
                    vst3::Steinberg::kInvalidArgument
                }
            } else {
                let buffer_states = parameters::ExistingBufferStates::new(params);
                helper.process(self.clone(), buffer_states.clone());
                if let Some(mpe_quirks) = mpe_quirks {
                    update_mpe_quirk_events_buffer(self.into_iter(), mpe_quirks, &buffer_states);
                }
                vst3::Steinberg::kResultOk
            }
        }
    }
}

struct NoAudioProcessHelper<'a, P, C> {
    processor: &'a mut P,
    events_empty: bool,
    category: &'a C,
}

impl<
        'a,
        P: ProcessorT,
        Iter: Iterator<Item = conformal_component::events::Data> + Clone,
        C: ActiveProcessorCategory<P>,
    > InternalProcessHelper<NoAudioProcessHelper<'a, P, C>> for Iter
{
    unsafe fn do_process(
        self,
        helper: NoAudioProcessHelper<'a, P, C>,
        params: &mut parameters::ProcessingStore,
        data: *mut vst3::Steinberg::Vst::ProcessData,
        mpe_quirks: Option<&mut mpe_quirks::State>,
        _: usize,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let Some(param_changes) = ComRef::from_raw((*data).inputParameterChanges) {
                if let Some((change_status, param_states)) =
                    parameters::no_audio_param_changes_from_vst3(param_changes, params)
                {
                    if change_status == parameters::ChangesStatus::Changes || !helper.events_empty {
                        if let Some(mpe_quirks) = mpe_quirks {
                            let param_states_clone = param_states.clone();
                            let events = add_mpe_quirk_events_no_audio(
                                self.clone(),
                                mpe_quirks.clone(),
                                &param_states_clone,
                            );
                            helper.category.handle_events(
                                helper.processor,
                                events,
                                param_states.clone(),
                            );
                            update_mpe_quirk_events_no_audio(self, mpe_quirks, &param_states);
                        } else {
                            helper
                                .category
                                .handle_events(helper.processor, self, param_states);
                        }
                    }
                    return vst3::Steinberg::kResultOk;
                }
                return vst3::Steinberg::kInvalidArgument;
            }
            if !helper.events_empty {
                helper
                    .category
                    .handle_events(helper.processor, self, &*params);
            }
            vst3::Steinberg::kResultOk
        }
    }
}

impl<P: Synth> ActiveProcessorCategory<P> for ActiveSynthProcessorCategory {
    type ProcessBuffer<'a>
        = SynthProcessBuffer<'a, P>
    where
        P: 'a;

    unsafe fn make_process_buffer<'a>(
        &self,
        processor: &'a mut P,
        data: *mut vst3::Steinberg::Vst::ProcessData,
    ) -> Option<Self::ProcessBuffer<'a>> {
        unsafe {
            if (*data).numOutputs != 1 {
                return None;
            }

            Some(SynthProcessBuffer {
                synth: processor,
                output: UnsafeMutBufferFromRaw {
                    ptr: (*(*data).outputs).__field0.channelBuffers32,
                    channel_layout: self.channel_layout,
                    num_frames: (*data).numSamples as usize,
                },
            })
        }
    }

    fn handle_events<
        E: Iterator<Item = conformal_component::events::Data> + Clone,
        Parameters: conformal_component::parameters::States,
    >(
        &self,
        processor: &mut P,
        e: E,
        p: Parameters,
    ) {
        processor.handle_events(e, p);
    }
}

impl<
        CF: ComponentFactory<Component: Component<Processor: ProcessorT>>,
        PC: ProcessorCategory<Active: ActiveProcessorCategory<<CF::Component as Component>::Processor>>,
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

                if num_frames == 0 {
                    if let Some(input_events) = ComRef::from_raw((*data).inputEvents) {
                        if let Some(event_iter) = events::all_zero_event_iterator(
                            input_events,
                            if pd.mpe_quirks.is_some() {
                                Support::SupportQuirks
                            } else {
                                Support::DoNotSupportQuirks
                            },
                        ) {
                            let helper = NoAudioProcessHelper {
                                processor: &mut pd.processor,
                                events_empty: event_iter.clone().next().is_none(),
                                category: &pd.category,
                            };
                            return event_iter.do_process(
                                helper,
                                &mut pd.params,
                                data,
                                pd.mpe_quirks.as_mut(),
                                0,
                            );
                        }
                    } else {
                        let helper = NoAudioProcessHelper {
                            processor: &mut pd.processor,
                            events_empty: true,
                            category: &pd.category,
                        };
                        return std::iter::empty().do_process(
                            helper,
                            &mut pd.params,
                            data,
                            pd.mpe_quirks.as_mut(),
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

                if let Some(process_buffer) =
                    pd.category.make_process_buffer(&mut pd.processor, data)
                {
                    if let Some(input_events) = ComRef::from_raw((*data).inputEvents) {
                        if let Some(events) = Events::new(
                            events::event_iterator(
                                input_events,
                                if pd.mpe_quirks.is_some() {
                                    Support::SupportQuirks
                                } else {
                                    Support::DoNotSupportQuirks
                                },
                            ),
                            num_frames,
                        ) {
                            return events.do_process(
                                process_buffer,
                                &mut pd.params,
                                data,
                                pd.mpe_quirks.as_mut(),
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
                                pd.mpe_quirks.as_mut(),
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
        PC: ProcessorCategory<Active: ActiveProcessorCategory<<CF::Component as Component>::Processor>>,
    > Class
    for Processor<<CF::Component as Component>::Processor, CF::Component, CF, PC, PC::Active>
{
    type Interfaces = (
        IPluginBase,
        IComponent,
        IAudioProcessor,
        IProcessContextRequirements,
        IConnectionPoint,
    );
}
