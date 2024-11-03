#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventData {
    NoteOn { data: NoteData },
    NoteOff { data: NoteData },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub sample_offset: usize,
    pub data: EventData,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoteExpressionState {
    pub pitch_bend: f32,
    pub timbre: f32,
    pub aftertouch: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteExpressionPoint {
    /// The time, relative to the start of the buffer.
    pub time: usize,

    // The current value of the expressions.
    pub state: NoteExpressionState,
}

/// A note expression is a series of points. Note that the following invariants
/// hold:
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
    /// Create an iterator that yields the note expression state for each sample
    #[allow(clippy::missing_panics_doc)]
    pub fn iter_by_sample(self) -> impl Iterator<Item = NoteExpressionState> + Clone {
        let mut iter = self.points.peekable();
        let mut last_state = None;
        (0..).map(move |sample_index| {
            while let Some(point) = iter.peek() {
                if point.time > sample_index {
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
        time: 0,
        state: Default::default(),
    }))
    .unwrap()
}

impl<I: Iterator<Item = NoteExpressionPoint> + Clone> NoteExpressionCurve<I> {
    pub fn new(points: I) -> Option<Self> {
        let points_iter = points.clone().peekable();
        let mut contains_zero = false;
        let mut last_time = None;
        // Check invariants
        for point in points_iter {
            if !contains_zero {
                if point.time != 0 {
                    return None;
                }
                contains_zero = true;
            } else if let Some(last_time) = last_time {
                if point.time < last_time {
                    return None;
                }
            }
            last_time = Some(point.time);
        }
        Some(Self { points })
    }
}

// Optimization opportunity - allow `Voice` to indicate that not all output
// was filled. This will let us skip rendering until a voice is playing
// and also skip mixing silence.

pub trait Voice {
    type SharedData<'a>: Clone;
    fn new(max_samples_per_process_call: usize, sampling_rate: f32) -> Self;
    fn handle_event(&mut self, event: &EventData);
    fn render_audio(
        &mut self,
        events: impl IntoIterator<Item = Event>,
        params: &impl parameters::BufferStates,
        note_expressions: NoteExpressionCurve<impl Iterator<Item = NoteExpressionPoint> + Clone>,
        data: Self::SharedData<'_>,
        output: &mut [f32],
    );
    #[must_use]
    fn quiescent(&self) -> bool;
    fn skip_samples(&mut self, _num_samples: usize) {}
    fn reset(&mut self);
}

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

    pub fn render_audio(
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
            voice.render_audio(
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

    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.state.reset();
    }
}
