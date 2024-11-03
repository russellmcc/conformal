#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    missing_docs,
    rustdoc::private_doc_tests,
    rustdoc::unescaped_backticks,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]
#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

use self::state::State;
use conformal_component::{
    audio::{channels_mut, BufferMut},
    events::{Data, Event as CEvent, NoteData},
    parameters, ProcessingEnvironment,
};

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

/// The data associated with an event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventData {
    /// This event is sent when a note is started.
    NoteOn {
        /// The data associated with the note.
        data: NoteData,
    },
    /// This event is sent when a note is stopped.
    NoteOff {
        /// The data associated with the note.
        data: NoteData,
    },
}

/// An event sent to a voice at a particular time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    /// The offset relative to the start of the buffer in samples when the event occurred.
    pub sample_offset: usize,
    /// The data associated with the event.
    pub data: EventData,
}

/// The current state of all note expression controllers for a voice.
///
/// See the documentation for [`conformal_component::events::NoteExpression`] for
/// more information.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoteExpressionState {
    /// The current value of the pitch bend for this voice in semitones away from the root note.
    pub pitch_bend: f32,

    /// The current value of the "timbre" controller for this voice.
    ///
    /// On many controllers, this represents the vertical or "y" position.
    /// This is referred to as "slide" in Ableton Live.
    ///
    /// This value varies from 0 to 1, with 0 being neutral.
    pub timbre: f32,

    /// The current value of the aftertouch controller for this voice.
    ///
    /// This value varies from 0 to 1, with 0 being neutral.
    pub aftertouch: f32,
}

/// A single point in a note expression curve.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteExpressionPoint {
    /// The time, expressed as samples relative to the start of the buffer.
    pub sample_offset: usize,

    /// The current value of the expression controllers for a voice.
    pub state: NoteExpressionState,
}

/// A note expression curve is a series of [`NoteExpressionPoint`]s over a buffer.
///
/// Note that the following invariants will hold:
///
///   - The number of points is at least 1
///   - The points are sorted by time
///   - The time of the first point is 0
///
/// Between points, the value the expression is constant - this makes it
/// different from [`conformal_component::parameters::PiecewiseLinearCurve`],
/// where the value is linearly interpolated between points.
#[derive(Debug, Clone)]
pub struct NoteExpressionCurve<I> {
    points: I,
}

impl<I: Iterator<Item = NoteExpressionPoint>> IntoIterator for NoteExpressionCurve<I> {
    type Item = NoteExpressionPoint;
    type IntoIter = I;

    fn into_iter(self) -> Self::IntoIter {
        self.points
    }
}

impl<I: Iterator<Item = NoteExpressionPoint> + Clone> NoteExpressionCurve<I> {
    /// Creates an iterator that yields the note expression state for each sample
    #[allow(clippy::missing_panics_doc)]
    pub fn iter_by_sample(self) -> impl Iterator<Item = NoteExpressionState> + Clone {
        let mut iter = self.points.peekable();
        let mut last_state = None;
        (0..).map(move |sample_index| {
            while let Some(point) = iter.peek() {
                if point.sample_offset > sample_index {
                    break;
                }
                last_state = Some(point.state);
                iter.next();
            }
            // Note that this will never panic, since the curve is guaranteed to have a point at time 0
            last_state.unwrap()
        })
    }
}

/// Return a note expression curve that is constant, with all expressions set to zero.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn default_note_expression_curve(
) -> NoteExpressionCurve<impl Iterator<Item = NoteExpressionPoint> + Clone> {
    NoteExpressionCurve::new(std::iter::once(NoteExpressionPoint {
        sample_offset: 0,
        state: Default::default(),
    }))
    .unwrap()
}

impl<I: Iterator<Item = NoteExpressionPoint> + Clone> NoteExpressionCurve<I> {
    /// Creates a new note expression curve from an iterator of points.
    ///
    /// Returns `None` if the curve does not satisfy the invariants described
    /// in the documentation for [`NoteExpressionCurve`].
    pub fn new(points: I) -> Option<Self> {
        let points_iter = points.clone().peekable();
        let mut contains_zero = false;
        let mut last_time = None;
        // Check invariants
        for point in points_iter {
            if !contains_zero {
                if point.sample_offset != 0 {
                    return None;
                }
                contains_zero = true;
            } else if let Some(last_time) = last_time {
                if point.sample_offset < last_time {
                    return None;
                }
            }
            last_time = Some(point.sample_offset);
        }
        Some(Self { points })
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
        params: &impl parameters::BufferStates,
        note_expressions: NoteExpressionCurve<impl Iterator<Item = NoteExpressionPoint> + Clone>,
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
    pub fn handle_events(&mut self, events: impl IntoIterator<Item = Data> + Clone) {
        for (v, ev) in self
            .state
            .clone()
            .dispatch_events(events.clone().into_iter().map(|data| CEvent {
                sample_offset: 0,
                data,
            }))
        {
            self.voices[v].handle_event(&ev.data);
        }

        self.state.update(events.into_iter().map(|data| CEvent {
            sample_offset: 0,
            data,
        }));
    }

    /// Renders the audio for the synth.
    ///
    /// This can be used to implement [`conformal_component::synth::Synth::process`].
    /// For any voices with active notes, [`Voice::process`] will be called.
    pub fn process(
        &mut self,
        events: impl Iterator<Item = CEvent> + Clone,
        params: &impl parameters::BufferStates,
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
                self.state
                    .clone()
                    .note_expressions_for_voice(index, events.clone()),
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
