//! Abstractions for processors that effect audio.

use crate::Processor;
use crate::audio::{Buffer, BufferMut};
use crate::parameters::{BufferStates, States};

/// A trait for metadata during an audio processing call
pub trait HandleParametersContext {
    /// The parameters to handle
    fn parameters(&self) -> impl States;
}

/// A trait for metadata during an audio processing call
pub trait ProcessContext {
    /// Parameter states for this call
    ///
    /// In order to consume the parameters, you can use the [`crate::pzip`] macro
    /// to convert the parameters into an iterator of tuples that represent
    /// the state of the parameters at each sample.
    fn parameters(&self) -> impl BufferStates;
}

/// A trait for audio effects
///
/// An effect is a processor that processes audio, and has both an input and an output
/// audio stream. It will receive information about the current state of the parameters
/// specified by the [`crate::Component`] that created it.
pub trait Effect: Processor {
    /// Handle parameter changes without processing any audio data.
    ///
    /// Must not allocate or block.
    fn handle_parameters(&mut self, context: &impl HandleParametersContext);

    /// Actually process audio data.
    ///
    /// Must not allocate or block.
    ///
    /// `input` and `output` will be the same length.
    ///
    /// `output` will be received in an undetermined state and must
    /// be filled with audio by the processor during this call.
    ///
    /// In addition to recieving the audio, this function also receives
    /// information about the state of the parameters throughout the buffer
    /// being processed.
    ///
    /// The sample rate of the audio was provided in `environment.sampling_rate`
    /// in the call to `crate::Component::create_processor`.
    ///
    /// Note that it's guaranteed that `output` will be no longer than
    /// `environment.max_samples_per_process_call` provided in the call to
    /// `crate::Component::create_processor`.
    fn process(
        &mut self,
        context: &impl ProcessContext,
        input: &impl Buffer,
        output: &mut impl BufferMut,
    );
}
