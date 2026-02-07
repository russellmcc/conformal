#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

use self::state::State;
use conformal_component::{
    ProcessingEnvironment,
    audio::{BufferMut, channels_mut},
    events as component_events, synth,
};

pub use conformal_component::events::NoteData;

/// The data associated with an event, independent of the time it occurred.
#[derive(Clone, Debug, PartialEq)]
pub enum EventData {
    /// A note began.
    NoteOn {
        /// Data associated with the note.
        data: NoteData,
    },
    /// A note ended.
    NoteOff {
        /// Data associated with the note.
        data: NoteData,
    },
}

/// An event that occurred at a specific time within a buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Number of sample frames after the beginning of the buffer that this event occurred.
    pub sample_offset: usize,
    /// Data about the event.
    pub data: EventData,
}

impl TryFrom<component_events::Data> for EventData {
    type Error = ();
    fn try_from(value: component_events::Data) -> Result<Self, Self::Error> {
        #[allow(unreachable_patterns)]
        match value {
            component_events::Data::NoteOn { data } => Ok(EventData::NoteOn { data }),
            component_events::Data::NoteOff { data } => Ok(EventData::NoteOff { data }),
            _ => Err(()),
        }
    }
}

impl TryFrom<component_events::Event> for Event {
    type Error = ();
    fn try_from(value: component_events::Event) -> Result<Self, Self::Error> {
        Ok(Event {
            sample_offset: value.sample_offset,
            data: value.data.try_into()?,
        })
    }
}

fn add_in_place(x: &[f32], y: &mut [f32]) {
    for (x, y) in x.iter().zip(y.iter_mut()) {
        *y += *x;
    }
}

fn mul_constant_in_place(x: f32, y: &mut [f32]) {
    for y in y.iter_mut() {
        *y *= x;
    }
}

// Optimization opportunity - allow `Voice` to indicate that not all output
// was filled. This will let us skip rendering until a voice is playing
// and also skip mixing silence.

/// A single voice in a polyphonic synth.
pub trait Voice {
    /// Data that is shared across all voices. This could include things like
    /// low frequency oscillators that are used by multiple voices.
    type SharedData<'a>: Clone;

    /// Creates a new voice.
    fn new(max_samples_per_process_call: usize, sampling_rate: f32) -> Self;

    /// Handles a single event outside of audio processing.
    ///
    /// Note that events sent during a [`process`](`Voice::process`) call must be handled there.
    fn handle_event(&mut self, event: &EventData);

    /// Renders audio for this voice.
    ///
    /// Audio for the voice will be written into the `output` buffer, which will
    /// start out filled with silence.
    fn process(
        &mut self,
        events: impl IntoIterator<Item = Event>,
        params: &impl synth::SynthParamBufferStates,
        data: Self::SharedData<'_>,
        output: &mut [f32],
    );

    /// Returns whether this voice is currently outputng audio.
    ///
    /// When this returns `true`, [`process`](`Voice::process`) will not be called for this
    /// voice again until a new note is started. This can improve performance by
    /// allowing voices to skip processing.
    #[must_use]
    fn quiescent(&self) -> bool;

    /// Called in lieu of [`process`](`Voice::process`) when the voice is quiescent.
    ///
    /// Voices can use this call to update internal state such as oscillator
    /// phase, to simulate the effect we'd get if we had processed `num_samples`
    /// of audio.
    fn skip_samples(&mut self, _num_samples: usize) {}

    /// Resets the voice to its initial state.
    fn reset(&mut self);
}

/// A helper struct for implementing polyphonic synths.
///
/// This struct handles common tasks such as routing events to voices, updating note expression curves,
/// and mixing the output of voices.
///
/// To use it, you must implement the [`Voice`] trait for your synth. Then, use the methods
/// on this struct to implement the required [`conformal_component::synth::Synth`] trait methods.
pub struct Poly<V> {
    voices: Vec<V>,
    state: State,
    voice_scratch_buffer: Vec<f32>,
}

impl<V: std::fmt::Debug> std::fmt::Debug for Poly<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Poly")
            .field("voices", &self.voices)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

mod state;

impl<V: Voice> Poly<V> {
    /// Creates a new [`Poly`] struct.
    #[must_use]
    pub fn new(environment: &ProcessingEnvironment, max_voices: usize) -> Self {
        let voices = std::iter::repeat_with(|| {
            V::new(
                environment.max_samples_per_process_call,
                environment.sampling_rate,
            )
        })
        .take(max_voices)
        .collect();
        let state = State::new(max_voices);

        Self {
            voices,
            state,
            voice_scratch_buffer: vec![0f32; environment.max_samples_per_process_call],
        }
    }

    /// Handles a set of events without rendering audio.
    ///
    /// This can be used to implement [`conformal_component::synth::Synth::handle_events`].
    pub fn handle_events(&mut self, events: impl Iterator<Item = component_events::Data> + Clone) {
        let poly_events = events.filter_map(|data| {
            EventData::try_from(data).ok().map(|data| Event {
                sample_offset: 0,
                data,
            })
        });

        for (v, ev) in self.state.clone().dispatch_events(poly_events.clone()) {
            self.voices[v].handle_event(&ev.data);
        }

        self.state.update(poly_events);
    }

    /// Renders the audio for the synth.
    ///
    /// This can be used to implement [`conformal_component::synth::Synth::process`].
    /// For any voices with active notes, [`Voice::process`] will be called.
    pub fn process(
        &mut self,
        events: impl Iterator<Item = component_events::Event> + Clone,
        params: &impl synth::SynthParamBufferStates,
        shared_data: &V::SharedData<'_>,
        output: &mut impl BufferMut,
    ) {
        let poly_events = events.filter_map(|e| Event::try_from(e).ok());
        self.process_inner(poly_events, params, shared_data, output);
    }

    fn process_inner(
        &mut self,
        events: impl Iterator<Item = Event> + Clone,
        params: &impl synth::SynthParamBufferStates,
        shared_data: &V::SharedData<'_>,
        output: &mut impl BufferMut,
    ) {
        let buffer_size = output.num_frames();
        #[allow(clippy::cast_precision_loss)]
        let voice_scale = 1f32 / self.voices.len() as f32;
        let mut cleared = false;
        for (index, voice) in self.voices.iter_mut().enumerate() {
            let voice_events = || {
                self.state
                    .clone()
                    .dispatch_events(events.clone())
                    .into_iter()
                    .filter_map(|(i, event)| if i == index { Some(event) } else { None })
            };
            if voice_events().next().is_none() && voice.quiescent() {
                voice.skip_samples(buffer_size);
                continue;
            }
            voice.process(
                voice_events(),
                params,
                shared_data.clone(),
                &mut self.voice_scratch_buffer[0..output.num_frames()],
            );
            mul_constant_in_place(voice_scale, &mut self.voice_scratch_buffer);
            if cleared {
                for channel_mut in channels_mut(output) {
                    add_in_place(&self.voice_scratch_buffer[0..buffer_size], channel_mut);
                }
            } else {
                for channel_mut in channels_mut(output) {
                    channel_mut.copy_from_slice(&self.voice_scratch_buffer[0..buffer_size]);
                }
                cleared = true;
            }
        }
        if !cleared {
            for channel_mut in channels_mut(output) {
                channel_mut.fill(0f32);
            }
        }
        self.state.update(events);
    }

    /// Resets the state of the polyphonic synth.
    ///
    /// This can be used to implement [`conformal_component::Processor::set_processing`].
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.state.reset();
    }
}
