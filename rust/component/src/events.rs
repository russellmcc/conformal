//! Contains data structures representing _events_ sent to [`crate::synth::Synth`]s

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteIDInternals {
    NoteIDWithID(i32),
    NoteIDFromPitch(u8),
    NoteIDFromChannelID(i16),
}

/// Represents an identifier for a note
///
/// This is an opaque identifier that can be used to refer to a specific note
/// that is playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoteID {
    /// This is only for use by implementors of format adaptors, do not look into this
    /// as a plug-in writer.
    #[doc(hidden)]
    pub internals: NoteIDInternals,
}

impl NoteID {
    /// Create a note ID from a pitch.
    #[must_use]
    pub fn from_pitch(pitch: u8) -> Self {
        Self {
            internals: NoteIDInternals::NoteIDFromPitch(pitch),
        }
    }

    /// Create a note ID from a channel ID.
    #[must_use]
    pub fn from_channel_id(channel_id: i16) -> Self {
        Self {
            internals: NoteIDInternals::NoteIDFromChannelID(channel_id),
        }
    }

    /// Create a note ID from a VST Node ID.
    #[must_use]
    pub fn from_id(id: i32) -> Self {
        Self {
            internals: NoteIDInternals::NoteIDWithID(id),
        }
    }
}

/// Contains data common to both `NoteOn` and `NoteOff` events.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteData {
    /// Opaque ID of the note.
    pub id: NoteID,

    /// Pitch of the note in terms of semitones higher than C-2
    pub pitch: u8,

    /// 0->1 velocity of the note on or off
    pub velocity: f32,

    /// Microtuning of the note in cents.
    pub tuning: f32,
}

/// The data associated with an event, independent of the time it occurred.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// A note began.
    ///
    /// This will never be sent while a note with the same ID is still playing.
    NoteOn {
        /// Data associated with the note.
        data: NoteData,
    },

    /// A note ended.
    ///
    /// This will never be sent while a note with the same ID is not playing.
    NoteOff {
        /// Data associated with the note.
        data: NoteData,
    },
}

/// An event that occurred at a specific time within a buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Number of sample frames after the beginning of the buffer that this event occurred
    pub sample_offset: usize,

    /// Data about the event!
    pub data: Data,
}

/// Contains an iterator that yields events in order of increasing sample offset.
///
/// Invariants:
///  - All events will have a sample offset in the range of the buffer
///  - Events are sorted by `sample_offset`
#[derive(Clone, Debug)]
pub struct Events<I> {
    events: I,
}

fn check_events_invariants<I: Iterator<Item = Event>>(iter: I, buffer_size: usize) -> bool {
    let mut last = None;
    for event in iter {
        if event.sample_offset >= buffer_size {
            return false;
        }
        if let Some(last) = last
            && event.sample_offset < last
        {
            return false;
        }
        last = Some(event.sample_offset);
    }
    true
}

impl<I: Iterator<Item = Event> + Clone> Events<I> {
    /// Create an `Events` object from the given iterator of events.
    ///
    /// Note that if any of the invariants are missed, this will return `None`.
    pub fn new(events: I, buffer_size: usize) -> Option<Self> {
        if check_events_invariants(events.clone(), buffer_size) {
            Some(Self { events })
        } else {
            None
        }
    }
}

impl<I: Iterator<Item = Event>> IntoIterator for Events<I> {
    type Item = Event;
    type IntoIter = I;

    fn into_iter(self) -> Self::IntoIter {
        self.events
    }
}

#[cfg(test)]
mod tests {
    use super::{Data, Event, Events, NoteData, NoteID};

    static EXAMPLE_NOTE: NoteData = NoteData {
        id: NoteID {
            internals: super::NoteIDInternals::NoteIDFromPitch(60),
        },
        pitch: 60,
        velocity: 1.0,
        tuning: 0.0,
    };

    #[test]
    fn out_of_order_events_rejected() {
        assert!(
            Events::new(
                (&[
                    Event {
                        sample_offset: 5,
                        data: Data::NoteOn {
                            data: EXAMPLE_NOTE.clone()
                        }
                    },
                    Event {
                        sample_offset: 4,
                        data: Data::NoteOff {
                            data: EXAMPLE_NOTE.clone()
                        }
                    }
                ])
                    .iter()
                    .cloned(),
                10
            )
            .is_none()
        )
    }

    #[test]
    fn out_of_bounds_events_rejected() {
        assert!(
            Events::new(
                (&[Event {
                    sample_offset: 50,
                    data: Data::NoteOn {
                        data: EXAMPLE_NOTE.clone()
                    }
                },])
                    .iter()
                    .cloned(),
                10
            )
            .is_none()
        )
    }

    #[test]
    fn empty_events_accepted() {
        assert!(Events::new((&[]).iter().cloned(), 10).is_some())
    }
}
