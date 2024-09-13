//! Abstractions for processors that effect audio.

use crate::audio::{Buffer, BufferMut};
use crate::{parameters, parameters::BufferStates, Processor};

/// A trait for audio effects
///
/// An effect is a processor that processes audio, and has both an input and an output
/// audio stream. It will receive information about the current state of the parameters
/// specified by the [`crate::Component`] that created it.
pub trait Effect: Processor {
    /// Handle parameter changes without processing any audio data.
    ///
    /// Must not allocate or block.
    fn handle_parameters<P: parameters::States>(&mut self, parameters: P);

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
    fn process<P: BufferStates, I: Buffer, O: BufferMut>(
        &mut self,
        parameters: P,
        input: &I,
        output: &mut O,
    );
}
