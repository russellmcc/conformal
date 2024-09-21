//! Contains data structures representing _events_ sent to [`crate::synth::Synth`]s

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum NoteIDInternals {
    NoteIDWithID(i32),
    NoteIDFromPitch(u8),
}

/// Represents an identifier for a note
///
/// This is an opaque identifier that can be used to refer to a specific note
/// that is playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoteID {
    internals: NoteIDInternals,
}

impl NoteID {
    /// Create a new `NoteID` from a numeric ID.
    ///
    /// Note that the `NoteID`s will be considered equal if they come from
    /// the same numeric ID, and different if they come from different numeric IDs.
    ///
    /// # Examples
    ///
    /// ```
    /// use conformal_component::events::NoteID;
    /// assert_eq!(NoteID::from_id(42), NoteID::from_id(42));
    /// assert_ne!(NoteID::from_id(42), NoteID::from_id(43));
    /// ```
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        Self {
            internals: NoteIDInternals::NoteIDWithID(id),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn from_pitch(pitch: u8) -> Self {
        Self {
            internals: NoteIDInternals::NoteIDFromPitch(pitch),
        }
    }
}

#[doc(hidden)]
#[must_use]
pub fn to_vst_note_id(note_id: NoteID) -> i32 {
    match note_id.internals {
        NoteIDInternals::NoteIDWithID(id) => id,
        NoteIDInternals::NoteIDFromPitch(_) => -1,
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

/// A specific type of note expression.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NoteExpression {
    /// Tuning note expression.
    ///
    /// This is expressed in semitones away from the root pitch of the note.
    Tuning(f32),

    /// Vertical movement note expression.
    ///
    /// This is called "slide" in some DAW UIs.
    ///
    /// This value varies from 0->1, 0 being the bottommost position,
    /// and 1 being the topmost position.
    Vertical(f32),

    /// Depthwise note expression.
    ///
    /// This is called "Pressure" in some DAW UIs.
    ///
    /// This value varies from 0->1, 0 being neutral, and 1 being the maximum depth.
    Depth(f32),
}

/// Contains data about note expression.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteExpressionData {
    /// Opaque ID of the note. This will always refer to a note that is
    /// currently "on".
    pub id: NoteID,

    /// The expression that is being sent.
    pub expression: NoteExpression,
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

    /// A note expression was sent.
    ///
    /// This will never be sent while a note with the same ID is not playing.
    NoteExpression {
        /// Data associated with the note expression.
        data: NoteExpressionData,
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
        if let Some(last) = last {
            if event.sample_offset < last {
                return false;
            }
        }
        last = Some(event.sample_offset);
    }
    true
}

impl<I: IntoIterator<Item = Event> + Clone> Events<I> {
    /// Create an `Events` object from the given iterator of events.
    ///
    /// Note that if any of the invariants are missed, this will return `None`.
    pub fn new(events: I, buffer_size: usize) -> Option<Self> {
        if check_events_invariants(events.clone().into_iter(), buffer_size) {
            Some(Self { events })
        } else {
            None
        }
    }
}

impl<I: IntoIterator<Item = Event>> IntoIterator for Events<I> {
    type Item = Event;
    type IntoIter = I::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}
