//! MPE support
//!
//! Note that there are two main ways hosts support MPE:
//!
//!  - The official way, documented in the VST sdk, is to use
//!    kNoteExpressionValueEvent events in the event stream on
//!    each change.
//!  - The "quirks" way, which is a completely undocumented method
//!    used by ableton. Quirks is our own terminology for this.
//!    this works by exposing MIDI maps for each MPE channel. Plug-ins
//!    must set up mappings to params, and then changes are provided
//!    as params.
//!
//! We support both methods. We use the first method for note IDs with
//! channel 0 and a non-negative-one VST Node ID, and the second method
//! for non-channel-0 note IDs.
//!
//! Note that eventually we may want to legitimately support more channels,
//! in which case we'll have to be smarter about when to interpret channels
//! as MPE quirks.

use conformal_component::parameters;
use conformal_component::{
    events::{NoteID, NoteIDInternals},
    synth::NumericPerNoteExpression,
};
pub mod quirks;

#[derive(Default, Debug, Clone)]
pub struct State {
    quirks_hashes: quirks::Hashes,
}

impl State {
    pub fn get_numeric_expression_for_note(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
        parameters: &impl parameters::States,
    ) -> f32 {
        if let NoteIDInternals::NoteIDFromChannelID(channel) = note_id.internals
            && let channel @ 1..=16 = channel
        {
            return parameters
                .numeric_by_hash(self.quirks_hashes[(expression, channel - 1)])
                .unwrap_or_default();
        }

        Default::default()
    }

    pub fn get_numeric_expression_for_note_buffer(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
        parameters: &impl parameters::BufferStates,
    ) -> parameters::NumericBufferState<
        impl Iterator<Item = parameters::PiecewiseLinearCurvePoint> + Clone,
    > {
        if let NoteIDInternals::NoteIDFromChannelID(channel) = note_id.internals
            && let channel @ 1..=16 = channel
            && let Some(state) =
                parameters.numeric_by_hash(self.quirks_hashes[(expression, channel)])
        {
            return state;
        }

        parameters::NumericBufferState::Constant(Default::default())
    }
}
