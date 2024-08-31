use crate::audio::{Buffer, BufferMut};
use crate::{parameters, parameters::BufferStates, Processor};

pub trait Effect: Processor {
    /// Handle parameter changes without processing any data.
    /// Must not allocate or block.
    ///
    /// Note that this will be called any time events come in without audio,
    /// or when parameters are changed without audio.
    fn handle_parameters<P: parameters::States>(&mut self, parameters: P);

    /// Process audio!
    fn process<P: BufferStates, I: Buffer, O: BufferMut>(
        &mut self,
        parameters: P,
        input: &I,
        output: &mut O,
    );
}
