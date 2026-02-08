#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

use crate::splice::{TimedStateChange, splice_numeric_buffer_states};

use self::state::State;
use conformal_component::{
    ProcessingEnvironment,
    audio::{BufferMut, channels_mut},
    events::{self as component_events, NoteID},
    parameters::{
        NumericBufferState, PiecewiseLinearCurvePoint, left_numeric_buffer, right_numeric_buffer,
    },
    synth::{self, valid_range_for_per_note_expression},
};

pub use conformal_component::events::NoteData;

mod splice;
mod state;

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

/// Non-audio data availble to voices during the processing call.
///
/// This includes events that occur during the buffer, as well as relevant parameter values.
pub trait VoiceProcessContext {
    /// The type of shared data shared between all voices.
    type SharedData: Clone;

    /// Returns an iterator of events that occurred for this voice during the processing call.
    fn events(&self) -> impl Iterator<Item = Event> + Clone;

    /// Returns the parameter states for this processing call.
    fn parameters(&self) -> &impl synth::SynthParamBufferStates;

    /// Returns the state of per-note expression routed to this voice.
    ///
    /// Note that most of the time, this will include data for just one note.
    /// However in some cases, a voice will have to play multiple notes within one buffer,
    /// which is handled by this call.
    fn per_note_expression(
        &self,
        expression: synth::NumericPerNoteExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>;

    /// Returns the shared data for this processing call.
    fn shared_data(&self) -> &Self::SharedData;
}

/// A single voice in a polyphonic synth.
pub trait Voice {
    /// Data that is shared across all voices. This could include things like
    /// low frequency oscillators that are used by multiple voices.
    type SharedData<'a>: Clone;

    /// Creates a new voice.
    fn new(voice_index: usize, max_samples_per_process_call: usize, sampling_rate: f32) -> Self;

    /// Handles a single event outside of audio processing.
    ///
    /// Note that events sent during a [`process`](`Voice::process`) call must be handled there.
    fn handle_event(&mut self, event: &EventData);

    /// Renders audio for this voice.
    ///
    /// Audio for the voice will be written into the `output` buffer, which will
    /// start out filled with silence.
    fn process<'a>(
        &mut self,
        context: &impl VoiceProcessContext<SharedData = Self::SharedData<'a>>,
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

struct ProcessContextImpl<'a, E, P, SD> {
    initial_note_id: Option<NoteID>,
    events: E,
    parameters: &'a P,
    shared_data: &'a SD,
    buffer_size: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct TimedNoteChange {
    note_id: NoteID,
    sample_offset: usize,
}

// Returns an iterator of _changes_ in note id from the given initial note id in an event stream.
//
// Note offs do not change the effective note id, so they are ignored.
//
// Note ons only represent note _changes_ if they represent a change from the previous note id.
fn note_changes(
    initial_note_id: Option<NoteID>,
    events: impl Iterator<Item = Event> + Clone,
) -> impl Iterator<Item = TimedNoteChange> + Clone {
    let mut last_note_id = initial_note_id;
    events.filter_map(move |e| match e.data {
        EventData::NoteOn { data } => {
            if Some(data.id) == last_note_id {
                None
            } else {
                last_note_id = Some(data.id);
                Some(TimedNoteChange {
                    note_id: data.id,
                    sample_offset: e.sample_offset,
                })
            }
        }
        _ => None,
    })
}

impl<E: Iterator<Item = Event> + Clone, P: synth::SynthParamBufferStates, SD: Clone>
    VoiceProcessContext for ProcessContextImpl<'_, E, P, SD>
{
    type SharedData = SD;

    fn events(&self) -> impl Iterator<Item = Event> + Clone {
        self.events.clone()
    }

    fn parameters(&self) -> &impl synth::SynthParamBufferStates {
        self.parameters
    }

    fn shared_data(&self) -> &Self::SharedData {
        self.shared_data
    }

    fn per_note_expression(
        &self,
        expression: synth::NumericPerNoteExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        // There are two cases to consider:
        //  1) The note does not change during the buffer (common case). We can make one
        //     query to the parameter state to get the buffer state for this expression.
        //     in this case we use the "left" branch.
        //  2) We have a note change during the buffer and we have to splice buffer states.
        //     this is a much more awkward case, and we use the "splice" helper and the "right"
        //     branch.

        let mut note_changes = note_changes(self.initial_note_id, self.events());
        let first_change = note_changes.next();

        let note_change_to_state_change =
            move |TimedNoteChange {
                      note_id,
                      sample_offset,
                  }| TimedStateChange {
                sample_offset,
                state: self
                    .parameters
                    .get_numeric_expression_for_note(expression, note_id),
            };
        match (self.initial_note_id, first_change) {
            // Easy case - we have a note playing, and we received no events. In this case,
            // we just grab the state of the note we started with.
            (Some(initial_note_id), None) => left_numeric_buffer(
                self.parameters
                    .get_numeric_expression_for_note(expression, initial_note_id),
            ),
            // Easy case - we have no note playing, and we received no events. In this case,
            // we just return a constant zero. Note that this is in range for all expression types.
            (None, None) => NumericBufferState::Constant(Default::default()),
            // In this case, we definitely have to splice
            (Some(initial_note_id), Some(first_change)) => {
                right_numeric_buffer(splice_numeric_buffer_states(
                    self.parameters
                        .get_numeric_expression_for_note(expression, initial_note_id),
                    std::iter::once(first_change)
                        .chain(note_changes)
                        .map(note_change_to_state_change),
                    self.buffer_size,
                    valid_range_for_per_note_expression(expression),
                ))
            }
            // In this case, we started without a note but got at least one note change.
            // In this case, we might be able to get away with a single lookup if there was
            // only a single change!
            (None, Some(first_change)) => {
                let next_change = note_changes.next();
                match next_change {
                    // TODO - actually ensure preconditions of splice are met here.
                    Some(next_change) => right_numeric_buffer(splice_numeric_buffer_states(
                        self.parameters
                            .get_numeric_expression_for_note(expression, first_change.note_id),
                        std::iter::once(next_change)
                            .chain(note_changes)
                            .map(note_change_to_state_change),
                        self.buffer_size,
                        valid_range_for_per_note_expression(expression),
                    )),
                    None => left_numeric_buffer(
                        self.parameters
                            .get_numeric_expression_for_note(expression, first_change.note_id),
                    ),
                }
            }
        }
    }
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

impl<V: Voice> Poly<V> {
    /// Creates a new [`Poly`] struct.
    #[must_use]
    pub fn new(environment: &ProcessingEnvironment, max_voices: usize) -> Self {
        let voices = (0..max_voices)
            .map(|voice_index| {
                V::new(
                    voice_index,
                    environment.max_samples_per_process_call,
                    environment.sampling_rate,
                )
            })
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
    pub fn handle_events(&mut self, context: &impl synth::HandleEventsContext) {
        let poly_events = context.events().filter_map(|data| {
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
        context: &impl synth::ProcessContext,
        shared_data: &V::SharedData<'_>,
        output: &mut impl BufferMut,
    ) {
        let params = context.parameters();
        let poly_events = context
            .events()
            .into_iter()
            .filter_map(|e| Event::try_from(e).ok());
        self.process_inner(poly_events, &params, shared_data, output);
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
            let voice_events = self
                .state
                .clone()
                .dispatch_events(events.clone())
                .filter_map(|(i, event)| if i == index { Some(event) } else { None });
            if voice_events.clone().next().is_none() && voice.quiescent() {
                voice.skip_samples(buffer_size);
                // Clear the "prev note" id for this voice since it's no longer active.
                self.state.clear_prev_note_id_for_voice(index);
                continue;
            }
            voice.process(
                &ProcessContextImpl {
                    initial_note_id: self.state.note_id_for_voice(index),
                    events: voice_events,
                    parameters: params,
                    shared_data,
                    buffer_size: output.num_frames(),
                },
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
