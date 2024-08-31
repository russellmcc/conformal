use vst3::{
    Class, ComWrapper,
    Steinberg::Vst::{
        AudioBusBuffers__type0, IAudioProcessorTrait, IComponentTrait, IEventList, IEventListTrait,
        IHostApplication, IHostApplicationTrait, IParameterChanges, IParameterChangesTrait,
    },
};

use conformal_component::{
    events::{to_vst_note_id, Data, Event},
    parameters::hash_id,
    ProcessingMode,
};

use super::PartialProcessingEnvironment;

pub(super) const DEFAULT_ENV: PartialProcessingEnvironment = PartialProcessingEnvironment {
    sampling_rate: 44100.0,
    max_samples_per_process_call: 512,
    processing_mode: ProcessingMode::Realtime,
};

pub(super) fn process_setup(
    env: &PartialProcessingEnvironment,
) -> vst3::Steinberg::Vst::ProcessSetup {
    vst3::Steinberg::Vst::ProcessSetup {
        processMode: match env.processing_mode {
            ProcessingMode::Realtime => vst3::Steinberg::Vst::ProcessModes_::kRealtime,
            ProcessingMode::Prefetch => vst3::Steinberg::Vst::ProcessModes_::kPrefetch,
            ProcessingMode::Offline => vst3::Steinberg::Vst::ProcessModes_::kOffline,
        } as i32,
        symbolicSampleSize: vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32,
        maxSamplesPerBlock: env.max_samples_per_process_call as i32,
        sampleRate: env.sampling_rate as f64,
    }
}

pub unsafe fn activate_busses<P: IComponentTrait>(proc: &P) {
    assert_eq!(
        proc.activateBus(
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
            vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
            0,
            1
        ),
        vst3::Steinberg::kResultOk
    );
    assert_eq!(
        proc.activateBus(
            vst3::Steinberg::Vst::MediaTypes_::kEvent as i32,
            vst3::Steinberg::Vst::BusDirections_::kInput as i32,
            0,
            1
        ),
        vst3::Steinberg::kResultOk
    );
}

pub unsafe fn activate_effect_busses<P: IComponentTrait>(proc: &P) {
    assert_eq!(
        proc.activateBus(
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
            vst3::Steinberg::Vst::BusDirections_::kOutput as i32,
            0,
            1
        ),
        vst3::Steinberg::kResultOk
    );
    assert_eq!(
        proc.activateBus(
            vst3::Steinberg::Vst::MediaTypes_::kAudio as i32,
            vst3::Steinberg::Vst::BusDirections_::kInput as i32,
            0,
            1
        ),
        vst3::Steinberg::kResultOk
    );
}

pub unsafe fn setup_proc<
    P: IAudioProcessorTrait + IComponentTrait,
    H: IHostApplicationTrait + Class,
>(
    proc: &P,
    host: &ComWrapper<H>,
) {
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
    activate_busses(proc);
    assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    assert_eq!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
}

pub unsafe fn setup_proc_effect<
    P: IAudioProcessorTrait + IComponentTrait,
    H: IHostApplicationTrait + Class,
>(
    proc: &P,
    host: &ComWrapper<H>,
) {
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
    let mut in_arrangement = vst3::Steinberg::Vst::SpeakerArr::kStereo;
    assert_eq!(
        proc.setBusArrangements(&mut in_arrangement, 1, &mut out_arrangement, 1),
        vst3::Steinberg::kResultOk
    );
    activate_effect_busses(proc);
    assert_eq!(proc.setActive(1u8), vst3::Steinberg::kResultOk);
    assert_eq!(proc.setProcessing(1u8), vst3::Steinberg::kResultOk);
}

pub struct ParameterValueQueuePoint {
    pub sample_offset: usize,
    pub value: f64,
}

pub struct ParameterValueQueueImpl {
    pub param_id: &'static str,
    pub points: Vec<ParameterValueQueuePoint>,
}

impl vst3::Steinberg::Vst::IParamValueQueueTrait for ParameterValueQueueImpl {
    unsafe fn getParameterId(&self) -> vst3::Steinberg::Vst::ParamID {
        hash_id(self.param_id)
    }

    unsafe fn getPointCount(&self) -> vst3::Steinberg::int32 {
        self.points.len() as i32
    }

    unsafe fn getPoint(
        &self,
        index: vst3::Steinberg::int32,
        sample_offset: *mut vst3::Steinberg::int32,
        value: *mut vst3::Steinberg::Vst::ParamValue,
    ) -> vst3::Steinberg::tresult {
        if let Some(point) = self.points.get(index as usize) {
            *sample_offset = point.sample_offset as i32;
            *value = point.value;
            vst3::Steinberg::kResultOk
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn addPoint(
        &self,
        _sample_offset: vst3::Steinberg::int32,
        _value: vst3::Steinberg::Vst::ParamValue,
        _index: *mut vst3::Steinberg::int32,
    ) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kNotImplemented
    }
}

impl Class for ParameterValueQueueImpl {
    type Interfaces = (vst3::Steinberg::Vst::IParamValueQueue,);
}

struct ParameterChangesImpl {
    queues: Vec<vst3::ComPtr<vst3::Steinberg::Vst::IParamValueQueue>>,
}

impl ParameterChangesImpl {
    fn new(queues: Vec<ParameterValueQueueImpl>) -> Self {
        ParameterChangesImpl {
            queues: queues
                .into_iter()
                .map(|x| ComWrapper::new(x).to_com_ptr().unwrap())
                .collect::<Vec<_>>(),
        }
    }
}

impl IParameterChangesTrait for ParameterChangesImpl {
    unsafe fn getParameterCount(&self) -> vst3::Steinberg::int32 {
        self.queues.len() as i32
    }

    unsafe fn getParameterData(
        &self,
        index: vst3::Steinberg::int32,
    ) -> *mut vst3::Steinberg::Vst::IParamValueQueue {
        if let Some(queue) = self.queues.get(index as usize) {
            queue.as_ptr()
        } else {
            std::ptr::null_mut()
        }
    }

    unsafe fn addParameterData(
        &self,
        _id: *const vst3::Steinberg::Vst::ParamID,
        _index: *mut vst3::Steinberg::int32,
    ) -> *mut vst3::Steinberg::Vst::IParamValueQueue {
        std::ptr::null_mut()
    }
}

impl Class for ParameterChangesImpl {
    type Interfaces = (IParameterChanges,);
}

struct EventList {
    events: Vec<Event>,
}

fn event_to_vst3_event(event: &Event) -> vst3::Steinberg::Vst::Event {
    match &event.data {
        Data::NoteOn { data } => vst3::Steinberg::Vst::Event {
            busIndex: 0,
            sampleOffset: event.sample_offset as i32,
            ppqPosition: 0f64,
            flags: 0,
            r#type: vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent as u16,
            __field0: vst3::Steinberg::Vst::Event__type0 {
                noteOn: vst3::Steinberg::Vst::NoteOnEvent {
                    channel: data.channel as i16,
                    pitch: data.pitch as i16,
                    tuning: data.tuning,
                    velocity: data.velocity,
                    length: 0,
                    noteId: to_vst_note_id(data.id),
                },
            },
        },
        Data::NoteOff { data } => vst3::Steinberg::Vst::Event {
            busIndex: 0,
            sampleOffset: event.sample_offset as i32,
            ppqPosition: 0f64,
            flags: 0,
            r#type: vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent as u16,
            __field0: vst3::Steinberg::Vst::Event__type0 {
                noteOff: vst3::Steinberg::Vst::NoteOffEvent {
                    channel: data.channel as i16,
                    pitch: data.pitch as i16,
                    tuning: data.tuning,
                    velocity: data.velocity,
                    noteId: to_vst_note_id(data.id),
                },
            },
        },
    }
}

impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> vst3::Steinberg::int32 {
        self.events.len() as i32
    }

    unsafe fn getEvent(
        &self,
        index: vst3::Steinberg::int32,
        e: *mut vst3::Steinberg::Vst::Event,
    ) -> vst3::Steinberg::tresult {
        if let Some(event) = self.events.get(index as usize) {
            (*e) = event_to_vst3_event(event);
            vst3::Steinberg::kResultOk
        } else {
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn addEvent(&self, _e: *mut vst3::Steinberg::Vst::Event) -> vst3::Steinberg::tresult {
        unimplemented!()
    }
}

impl Class for EventList {
    type Interfaces = (IEventList,);
}

pub const SAMPLE_COUNT: usize = 512;

pub unsafe fn mock_process_mod<
    D: IAudioProcessorTrait,
    F: FnOnce(&mut vst3::Steinberg::Vst::ProcessData) -> (),
>(
    channel_count: usize,
    events: Vec<Event>,
    params: Vec<ParameterValueQueueImpl>,
    processor: &D,
    mod_data: F,
) -> Option<Vec<Vec<f32>>> {
    let input_parameter_changes = ComWrapper::new(ParameterChangesImpl::new(params))
        .to_com_ptr::<IParameterChanges>()
        .unwrap();
    let input_events = ComWrapper::new(EventList { events })
        .to_com_ptr::<IEventList>()
        .unwrap();

    let mut output_audio_channels = vec![vec![0f32; SAMPLE_COUNT]; channel_count];
    let mut output_audio_channels_ptr = output_audio_channels
        .iter_mut()
        .map(|x| x.as_mut_ptr())
        .collect::<Vec<_>>();
    let mut audio_buffer_struct = Box::new(vst3::Steinberg::Vst::AudioBusBuffers {
        numChannels: channel_count as i32,
        silenceFlags: 0,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: output_audio_channels_ptr.as_mut_ptr(),
        },
    });

    let mut process_data = vst3::Steinberg::Vst::ProcessData {
        processMode: vst3::Steinberg::Vst::ProcessModes_::kRealtime as i32,
        symbolicSampleSize: vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32,
        numSamples: SAMPLE_COUNT as i32,
        numInputs: 0,
        numOutputs: 1,
        inputs: std::ptr::null_mut(),
        outputs: audio_buffer_struct.as_mut(),
        inputParameterChanges: input_parameter_changes.as_ptr(),
        outputParameterChanges: std::ptr::null_mut(),
        inputEvents: input_events.as_ptr(),
        outputEvents: std::ptr::null_mut(),
        processContext: std::ptr::null_mut(),
    };
    mod_data(&mut process_data);
    if vst3::Steinberg::kResultOk == processor.process(&mut process_data) {
        Some(output_audio_channels)
    } else {
        None
    }
}

pub unsafe fn mock_process<D: IAudioProcessorTrait>(
    channel_count: usize,
    events: Vec<Event>,
    params: Vec<ParameterValueQueueImpl>,
    processor: &D,
) -> Option<Vec<Vec<f32>>> {
    mock_process_mod(channel_count, events, params, processor, |_| ())
}

pub unsafe fn mock_process_effect<D: IAudioProcessorTrait>(
    inputs: Vec<Vec<f32>>,
    params: Vec<ParameterValueQueueImpl>,
    processor: &D,
) -> Option<Vec<Vec<f32>>> {
    let input_parameter_changes = ComWrapper::new(ParameterChangesImpl::new(params))
        .to_com_ptr::<IParameterChanges>()
        .unwrap();

    let mut input_audio_channels_ptr = inputs
        .iter()
        .map(|x| x.as_ptr() as *mut f32)
        .collect::<Vec<_>>();

    let mut output_audio_channels = vec![vec![0f32; inputs[0].len()]; inputs.len()];
    let mut output_audio_channels_ptr = output_audio_channels
        .iter_mut()
        .map(|x| x.as_mut_ptr())
        .collect::<Vec<_>>();
    let mut input_audio_buffer_struct = Box::new(vst3::Steinberg::Vst::AudioBusBuffers {
        numChannels: inputs.len() as i32,
        silenceFlags: 0,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: input_audio_channels_ptr.as_mut_ptr(),
        },
    });

    let mut output_audio_buffer_struct = Box::new(vst3::Steinberg::Vst::AudioBusBuffers {
        numChannels: inputs.len() as i32,
        silenceFlags: 0,
        __field0: AudioBusBuffers__type0 {
            channelBuffers32: output_audio_channels_ptr.as_mut_ptr(),
        },
    });
    let mut process_data = vst3::Steinberg::Vst::ProcessData {
        processMode: vst3::Steinberg::Vst::ProcessModes_::kRealtime as i32,
        symbolicSampleSize: vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32,
        numSamples: inputs[0].len() as i32,
        numInputs: 1,
        numOutputs: 1,
        inputs: input_audio_buffer_struct.as_mut(),
        outputs: output_audio_buffer_struct.as_mut(),
        inputParameterChanges: input_parameter_changes.as_ptr(),
        outputParameterChanges: std::ptr::null_mut(),
        inputEvents: std::ptr::null_mut(),
        outputEvents: std::ptr::null_mut(),
        processContext: std::ptr::null_mut(),
    };
    if vst3::Steinberg::kResultOk == processor.process(&mut process_data) {
        Some(output_audio_channels)
    } else {
        None
    }
}

// We need `dead_code` here since some members keep alive raw pointers
// in `process_data`.
#[allow(dead_code)]
pub struct MockNoAudioProcessData {
    input_parameter_changes: vst3::ComPtr<IParameterChanges>,
    input_events: vst3::ComPtr<IEventList>,
    pub process_data: vst3::Steinberg::Vst::ProcessData,
}

pub unsafe fn mock_no_audio_process_data(
    events: Vec<Event>,
    parameters: Vec<ParameterValueQueueImpl>,
) -> MockNoAudioProcessData {
    let input_parameter_changes = ComWrapper::new(ParameterChangesImpl::new(parameters))
        .to_com_ptr::<IParameterChanges>()
        .unwrap();
    let input_events = ComWrapper::new(EventList { events })
        .to_com_ptr::<IEventList>()
        .unwrap();
    let process_data = vst3::Steinberg::Vst::ProcessData {
        processMode: vst3::Steinberg::Vst::ProcessModes_::kRealtime as i32,
        symbolicSampleSize: vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32 as i32,
        numSamples: 0,
        numInputs: 0,
        numOutputs: 0,
        inputs: std::ptr::null_mut(),
        outputs: std::ptr::null_mut(),
        inputParameterChanges: input_parameter_changes.as_ptr(),
        outputParameterChanges: std::ptr::null_mut(),
        inputEvents: input_events.as_ptr(),
        outputEvents: std::ptr::null_mut(),
        processContext: std::ptr::null_mut(),
    };
    MockNoAudioProcessData {
        input_parameter_changes,
        input_events,
        process_data,
    }
}
