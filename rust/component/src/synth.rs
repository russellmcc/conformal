use crate::{
    events,
    events::{Event, Events},
    parameters,
    parameters::BufferStates,
    Processor,
};
use audio::BufferMut;

pub trait Synth: Processor {
    /// Handle parameter changes and events without processing any data.
    /// Must not allocate or block.
    ///
    /// Note that this will be called any time events come in without audio,
    /// or when parameters are changed without audio.
    fn handle_events<E: IntoIterator<Item = events::Data> + Clone, P: parameters::States>(
        &mut self,
        events: E,
        parameters: P,
    );

    /// Process a buffer of events into a buffer of audio. Must not allocate or block.
    ///
    /// Note that `events` will be sorted by `sample_offset`
    fn process<E: IntoIterator<Item = Event> + Clone, P: BufferStates, O: BufferMut>(
        &mut self,
        events: Events<E>,
        parameters: P,
        output: &mut O,
    );
}
