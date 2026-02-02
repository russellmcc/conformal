//! Abstractions for processors that generate audio.

use crate::{
    Processor,
    audio::BufferMut,
    events::{self, Event, Events},
    parameters::{
        self, NumericBufferState, PiecewiseLinearCurvePoint, SwitchBufferState, TimedValue,
    },
};

/// Numeric expression controllers available on each synth.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NumericGlobalExpression {
    /// The global pitch bend.
    ///
    /// This ranges from -1.0 to 1.0, and represents the current state of the
    /// pitch bend controller. How to interpret this value in semitones
    /// precisely is up to each synth.
    ///
    /// Note that there is also a per-note pitch bend expression parameter,
    /// this should be combined with the global pitch bend to get the total
    /// amount of bend for each note.
    PitchBend,

    /// The mod wheel.
    ///
    /// This ranges from 0.0 to 1.0, and represents the current state of the
    /// mod wheel.
    ModWheel,

    /// The expression pedal.
    ///
    /// This ranges from 0.0 to 1.0, and represents the current state of the
    /// expression pedal.
    ExpressionPedal,

    /// Aftertouch, or "pressure" in some DAW UIs.
    ///
    /// This ranges from 0.0 to 1.0, and represents the current state of the
    /// global aftertouch.
    ///
    /// Note that there is also a per-note aftertouch expression parameter,
    /// this should be combined with the global aftertouch to get the total
    /// amount of aftertouch for each note.
    Aftertouch,

    /// Timbre, or "slide" in some DAW UIs.
    ///
    /// This ranges from 0.0 to 1.0, and represents the current state of the
    /// global timbre control.
    ///
    /// Note that there is also a per-note timbre expression parameter,
    /// this should be combined with the global timbre to get the total
    /// amount of timbre for each note.
    Timbre,
}

/// Switch expression controllers available on each synth.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SwitchGlobalExpression {
    /// The sustain pedal.
    ///
    /// This represents the current state of the sustain pedal controller.
    SustainPedal,
}

/// Extention to the [`parameters::States`] trait for synths.
pub trait SynthParamStates: parameters::States {
    /// Get the current value of a numeric global expression controller.
    fn get_numeric_global_expression(&self, expression: NumericGlobalExpression) -> f32;

    /// Get the current value of a switch global expression controller.
    fn get_switch_global_expression(&self, expression: SwitchGlobalExpression) -> bool;
}

/// A trait for metadata during an audio processing call
pub trait HandleEventsContext {
    /// The events to handle
    fn events(&self) -> impl Iterator<Item = events::Data> + Clone;

    /// Parameter state
    fn parameters(&self) -> impl SynthParamStates;
}

/// Extension to the [`parameters::BufferStates`] trait for synths.
pub trait SynthParamBufferStates: parameters::BufferStates {
    /// Get the current value of a numeric global expression controller.
    fn get_numeric_global_expression(
        &self,
        expression: NumericGlobalExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>;

    /// Get the current value of a switch global expression controller.
    fn get_switch_global_expression(
        &self,
        expression: SwitchGlobalExpression,
    ) -> SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone>;
}

/// A trait for metadata during an audio processing call
pub trait ProcessContext {
    /// The events for this processing call
    fn events(&self) -> Events<impl Iterator<Item = Event> + Clone>;

    /// Parameter states for this call
    ///
    /// In order to consume the parameters, you can use the [`crate::pzip`] macro
    /// to convert the parameters into an iterator of tuples that represent
    /// the state of the parameters at each sample.
    fn parameters(&self) -> impl SynthParamBufferStates;
}

/// A trait for synthesizers
///
/// A synthesizer is a processor that creates audio from a series of _events_,
/// such as Note On, or Note Off.
pub trait Synth: Processor {
    /// Handle parameter changes and events without processing any data.
    /// Must not allocate or block.
    fn handle_events(&mut self, context: impl HandleEventsContext);

    /// Process a buffer of events into a buffer of audio. Must not allocate or block.
    ///
    /// Note that `events` will be sorted by `sample_offset`
    ///
    /// `output` will be received in an undetermined state and must
    /// be filled with audio by the processor during this call.
    ///
    /// The sample rate of the audio was provided in `environment.sampling_rate`
    /// in the call to `crate::Component::create_processor`.
    ///
    /// Note that it's guaranteed that `output` will be no longer than
    /// `environment.max_samples_per_process_call` provided in the call to
    /// `crate::Component::create_processor`.
    fn process(&mut self, context: &impl ProcessContext, output: &mut impl BufferMut);
}
