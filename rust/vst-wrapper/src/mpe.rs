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

use std::collections::{HashMap, VecDeque};
use std::iter::once;
use std::ops::RangeInclusive;

use conformal_component::parameters::{
    self, NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint,
};
use conformal_component::{
    events::{NoteID, NoteIDInternals},
    synth::{NumericGlobalExpression, NumericPerNoteExpression, SwitchGlobalExpression},
};
use itertools::Either;
pub mod quirks;

#[derive(Default, Debug, Clone)]
struct PerNoteState {
    pitch_bend: f32,
    timbre: f32,
    aftertouch: f32,
    internal_id: u64,
    added_release_to_queue: bool,
}

impl PerNoteState {
    fn new_note(internal_id: u64) -> Self {
        Self {
            internal_id,
            ..Default::default()
        }
    }

    fn get_expression(&self, expression: NumericPerNoteExpression) -> f32 {
        match expression {
            NumericPerNoteExpression::PitchBend => self.pitch_bend,
            NumericPerNoteExpression::Timbre => self.timbre,
            NumericPerNoteExpression::Aftertouch => self.aftertouch,
        }
    }
}

#[derive(Debug, Clone)]
struct GlobalExpressionHashes {
    pitch_bend: parameters::IdHash,
    mod_wheel: parameters::IdHash,
    expression_pedal: parameters::IdHash,
    aftertouch: parameters::IdHash,
    timbre: parameters::IdHash,
    sustain_pedal: parameters::IdHash,
}

impl Default for GlobalExpressionHashes {
    fn default() -> Self {
        use crate::parameters::{
            parameter_id_for_numeric_global_expression, parameter_id_for_switch_global_expression,
        };
        Self {
            pitch_bend: parameters::hash_id(parameter_id_for_numeric_global_expression(
                NumericGlobalExpression::PitchBend,
            )),
            mod_wheel: parameters::hash_id(parameter_id_for_numeric_global_expression(
                NumericGlobalExpression::ModWheel,
            )),
            expression_pedal: parameters::hash_id(parameter_id_for_numeric_global_expression(
                NumericGlobalExpression::ExpressionPedal,
            )),
            aftertouch: parameters::hash_id(parameter_id_for_numeric_global_expression(
                NumericGlobalExpression::Aftertouch,
            )),
            timbre: parameters::hash_id(parameter_id_for_numeric_global_expression(
                NumericGlobalExpression::Timbre,
            )),
            sustain_pedal: parameters::hash_id(parameter_id_for_switch_global_expression(
                SwitchGlobalExpression::SustainPedal,
            )),
        }
    }
}

impl GlobalExpressionHashes {
    fn numeric(&self, expression: NumericGlobalExpression) -> parameters::IdHash {
        match expression {
            NumericGlobalExpression::PitchBend => self.pitch_bend,
            NumericGlobalExpression::ModWheel => self.mod_wheel,
            NumericGlobalExpression::ExpressionPedal => self.expression_pedal,
            NumericGlobalExpression::Aftertouch => self.aftertouch,
            NumericGlobalExpression::Timbre => self.timbre,
        }
    }

    fn switch(&self, expression: SwitchGlobalExpression) -> parameters::IdHash {
        match expression {
            SwitchGlobalExpression::SustainPedal => self.sustain_pedal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ReleaseRecord {
    id: i32,
    internal_id: u64,
}

#[derive(Debug, Clone)]
pub struct State {
    quirks_hashes: quirks::Hashes,
    global_expression_hashes: GlobalExpressionHashes,
    release_order: VecDeque<ReleaseRecord>,
    expression_states: HashMap<i32, PerNoteState>,
    next_internal_id: u64,
}

const MAX_NOTES_BEFORE_ALLOCATION: usize = 256;

impl Default for State {
    fn default() -> Self {
        Self {
            quirks_hashes: Default::default(),
            global_expression_hashes: Default::default(),
            // Note that we cheat a bit on the "no allocation" rule here, in that we
            // will allocate if we exceed this initial capacity. However, playing [`MAX_NOTES_BEFORE_ALLOCATION`] notes
            // at once is a bit excessive, so it's probably okay to allocate once in that case.
            release_order: VecDeque::with_capacity(MAX_NOTES_BEFORE_ALLOCATION),
            expression_states: HashMap::with_capacity(MAX_NOTES_BEFORE_ALLOCATION),
            next_internal_id: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NoteEventData {
    On {
        note_id: i32,
    },
    Off {
        note_id: i32,
    },
    ExpressionChange {
        note_id: i32,
        expression: NumericPerNoteExpression,
        value: f32,
    },
}

#[derive(Clone, Debug)]
pub struct NoteEvent {
    pub sample_offset: usize,
    pub data: NoteEventData,
}

/// A wrapper around an interator with a few invariants:
///
/// - The events must be sorted by sample offset
//' - The sample offsets must be in the range of the buffer
///
/// These are enforced by [`NoteEvents::new`].
#[derive(Clone)]
pub struct NoteEvents<I> {
    events: I,
    buffer_size: usize,
}

fn valid_range_for_per_note_expression(
    expression: NumericPerNoteExpression,
) -> RangeInclusive<f32> {
    match expression {
        NumericPerNoteExpression::PitchBend => -120.0..=120.0,
        NumericPerNoteExpression::Timbre | NumericPerNoteExpression::Aftertouch => 0.0..=1.0,
    }
}

fn check_note_events_invariants<I: Iterator<Item = NoteEvent>>(
    iter: I,
    buffer_size: usize,
) -> bool {
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

        if let NoteEventData::ExpressionChange {
            expression, value, ..
        } = event.data
            && !valid_range_for_per_note_expression(expression).contains(&value)
        {
            return false;
        }
    }
    true
}

impl<I: Iterator<Item = NoteEvent>> IntoIterator for NoteEvents<I> {
    type Item = NoteEvent;
    type IntoIter = I;

    fn into_iter(self) -> Self::IntoIter {
        self.events
    }
}

impl<I: Iterator<Item = NoteEvent> + Clone> NoteEvents<I> {
    pub fn new(events: I, buffer_size: usize) -> Option<Self> {
        if check_note_events_invariants(events.clone(), buffer_size) {
            Some(Self {
                events,
                buffer_size,
            })
        } else {
            None
        }
    }
}

fn left_numeric_buffer<
    A: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    B: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
>(
    state: NumericBufferState<A>,
) -> NumericBufferState<Either<A, B>> {
    match state {
        NumericBufferState::Constant(value) => NumericBufferState::Constant(value),
        NumericBufferState::PiecewiseLinear(curve) => {
            let buffer_size = curve.buffer_size();
            // Note we're sure that `curve` is valid, so so must be Either::Left(curve)
            NumericBufferState::PiecewiseLinear(unsafe {
                PiecewiseLinearCurve::from_parts_unchecked(
                    Either::Left(curve.into_iter()),
                    buffer_size,
                )
            })
        }
    }
}

fn right_numeric_buffer<
    A: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    B: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
>(
    state: NumericBufferState<B>,
) -> NumericBufferState<Either<A, B>> {
    match state {
        NumericBufferState::Constant(value) => NumericBufferState::Constant(value),
        NumericBufferState::PiecewiseLinear(curve) => {
            let buffer_size = curve.buffer_size();
            NumericBufferState::PiecewiseLinear(unsafe {
                // Note we're sure that `curve` is valid, so so must be Either::Right(curve)
                PiecewiseLinearCurve::from_parts_unchecked(
                    Either::Right(curve.into_iter()),
                    buffer_size,
                )
            })
        }
    }
}

fn get_numeric_buffer_for_note_expression(
    note_id: i32,
    expression: NumericPerNoteExpression,
    initial_value: f32,
    events: NoteEvents<impl Iterator<Item = NoteEvent> + Clone>,
) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
    let buffer_size = events.buffer_size;
    let iter = events
        .into_iter()
        .filter_map(move |event| match event.data {
            NoteEventData::ExpressionChange {
                note_id: event_note_id,
                expression: event_expression,
                value,
            } => {
                if event_note_id == note_id && event_expression == expression {
                    Some(PiecewiseLinearCurvePoint {
                        sample_offset: event.sample_offset,
                        value,
                    })
                } else {
                    None
                }
            }
            NoteEventData::On {
                note_id: event_note_id,
            } => {
                if event_note_id == note_id {
                    Some(PiecewiseLinearCurvePoint {
                        sample_offset: event.sample_offset,
                        value: initial_value,
                    })
                } else {
                    None
                }
            }
            NoteEventData::Off { .. } => None,
        });
    if iter.clone().next().is_some() {
        // Note this is a bit subtle.
        //
        // Invariants we have from `check_note_events_invariants` above:
        //  - events are sorted
        //  - events sample offsets are in the range of the buffer
        //  - events values are in the valid range for the expression
        //
        // However, we're missing some invariants we need for PiecewiseLinearCurve:
        //  - the first event must have sample offset 0
        //  - no two events can have the same sample offset
        //
        // We handle the first invariant by prepending a point with the initial value.
        // We handle the second invariant by filtering out all but the last event at a sample offset.
        // We do this by zipping with a shifted version - since we prepend the initial value,
        // iter.clone() is naturally offset by one position.
        let chained = once(PiecewiseLinearCurvePoint {
            sample_offset: 0,
            value: initial_value,
        })
        .chain(iter.clone());
        let shifted = iter.map(Some).chain(once(None));
        NumericBufferState::PiecewiseLinear(
            PiecewiseLinearCurve::new(
                chained
                    .zip(shifted)
                    .filter_map(|(current, next)| match next {
                        Some(n) if n.sample_offset == current.sample_offset => None,
                        _ => Some(current),
                    }),
                buffer_size,
                valid_range_for_per_note_expression(expression),
            )
            // Note that since we have ensured the invariants, a panic here indicates some bug
            // in the logic above.
            .unwrap(),
        )
    } else {
        NumericBufferState::Constant(initial_value)
    }
}

impl State {
    pub fn update_for_event_data(&mut self, event_datas: impl Iterator<Item = NoteEventData>) {
        for data in event_datas {
            match data {
                NoteEventData::On { note_id } => {
                    // Note that we have a couple of goals here (in order of priority):
                    //  - We *never* allocate unless there are more than `MAX_NOTES_BEFORE_ALLOCATION`
                    //    notes concurrently active
                    //  - We never drop expressions for notes that are still active
                    //  - We keep expressions around for inactive notes as long as possible
                    //    given the above constraints
                    //
                    // To achieve this, we maintain a queue of inactive notes from `Off` events.
                    // However, some of them may have been re-triggered, so we'll have to skip them.
                    // We use a 64-bit internal_id to identify re-triggers - each re-trigger gets
                    // a unique internal_id. We don't worry about collisions since the internal_id
                    // is 64 bits.
                    if self.expression_states.len() >= MAX_NOTES_BEFORE_ALLOCATION
                        && !self.expression_states.contains_key(&note_id)
                    {
                        while let Some(record) = self.release_order.pop_front() {
                            if self
                                .expression_states
                                .get(&record.id)
                                .is_some_and(|state| state.internal_id == record.internal_id)
                            {
                                self.expression_states.remove(&record.id);
                                // We only needed space for one note, so break here.
                                break;
                            }
                        }
                    }

                    self.expression_states
                        .insert(note_id, PerNoteState::new_note(self.next_internal_id));
                    self.next_internal_id += 1;
                }
                NoteEventData::Off { note_id } => {
                    // See above note for the overall scheme for avoiding allocations while maintaining
                    // expressions for as long as possible.
                    if let Some(state) = self.expression_states.get_mut(&note_id)
                        && !state.added_release_to_queue
                    {
                        state.added_release_to_queue = true;
                        let internal_id = state.internal_id;

                        // We might need to clear stale entries from note order that refer to notes
                        // that have since been retriggered. We only do this step if we need to do this
                        // to prevent allocation.
                        if self.release_order.len() >= MAX_NOTES_BEFORE_ALLOCATION {
                            self.release_order.retain(|record| {
                                self.expression_states
                                    .get(&record.id)
                                    .is_some_and(|state| state.internal_id == record.internal_id)
                            });
                        }

                        self.release_order.push_back(ReleaseRecord {
                            id: note_id,
                            internal_id,
                        });
                    }
                }
                NoteEventData::ExpressionChange {
                    note_id,
                    expression,
                    value,
                } => {
                    if let Some(state) = self.expression_states.get_mut(&note_id) {
                        match expression {
                            NumericPerNoteExpression::PitchBend => {
                                state.pitch_bend = value;
                            }
                            NumericPerNoteExpression::Timbre => {
                                state.timbre = value;
                            }
                            NumericPerNoteExpression::Aftertouch => {
                                state.aftertouch = value;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn update_for_events(&mut self, events: NoteEvents<impl Iterator<Item = NoteEvent>>) {
        self.update_for_event_data(events.into_iter().map(|event| event.data));
    }

    pub fn get_numeric_expression_for_note(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
        parameters: &impl parameters::States,
    ) -> f32 {
        match note_id.internals {
            NoteIDInternals::NoteIDFromChannelID(channel @ 1..=16) => parameters
                .numeric_by_hash(self.quirks_hashes[(expression, channel)])
                .unwrap_or_default(),
            NoteIDInternals::NoteIDWithID(note_id) => self
                .expression_states
                .get(&note_id)
                .map(|state| state.get_expression(expression))
                .unwrap_or_default(),
            _ => Default::default(),
        }
    }
    pub fn set_processing(&mut self, processing: bool) {
        if !processing {
            self.expression_states.clear();
        }
    }

    pub fn get_numeric_expression_for_note_buffer(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
        parameters: &impl parameters::BufferStates,
        events: Option<NoteEvents<impl Iterator<Item = NoteEvent> + Clone>>,
    ) -> parameters::NumericBufferState<
        impl Iterator<Item = parameters::PiecewiseLinearCurvePoint> + Clone,
    > {
        match note_id.internals {
            NoteIDInternals::NoteIDFromChannelID(channel) => {
                if let channel @ 1..=16 = channel
                    && let Some(state) =
                        parameters.numeric_by_hash(self.quirks_hashes[(expression, channel)])
                {
                    return left_numeric_buffer(state);
                }
            }
            NoteIDInternals::NoteIDWithID(note_id) => {
                let initial_value = self
                    .expression_states
                    .get(&note_id)
                    .map(|state| state.get_expression(expression))
                    .unwrap_or_default();
                if let Some(events) = events {
                    return right_numeric_buffer(get_numeric_buffer_for_note_expression(
                        note_id,
                        expression,
                        initial_value,
                        events,
                    ));
                }
                return parameters::NumericBufferState::Constant(initial_value);
            }
            NoteIDInternals::NoteIDFromPitch(_) => {}
        }

        parameters::NumericBufferState::Constant(Default::default())
    }

    pub fn get_numeric_global_expression(
        &self,
        expression: NumericGlobalExpression,
        parameters: &impl parameters::States,
    ) -> f32 {
        parameters
            .numeric_by_hash(self.global_expression_hashes.numeric(expression))
            .unwrap()
    }

    pub fn get_switch_global_expression(
        &self,
        expression: SwitchGlobalExpression,
        parameters: &impl parameters::States,
    ) -> bool {
        parameters
            .switch_by_hash(self.global_expression_hashes.switch(expression))
            .unwrap()
    }

    pub fn get_numeric_global_expression_buffer(
        &self,
        expression: NumericGlobalExpression,
        parameters: &impl parameters::BufferStates,
    ) -> parameters::NumericBufferState<
        impl Iterator<Item = parameters::PiecewiseLinearCurvePoint> + Clone,
    > {
        parameters
            .numeric_by_hash(self.global_expression_hashes.numeric(expression))
            .unwrap()
    }

    pub fn get_switch_global_expression_buffer(
        &self,
        expression: SwitchGlobalExpression,
        parameters: &impl parameters::BufferStates,
    ) -> parameters::SwitchBufferState<impl Iterator<Item = parameters::TimedValue<bool>> + Clone>
    {
        parameters
            .switch_by_hash(self.global_expression_hashes.switch(expression))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::mpe::quirks::timbre_param_id;
    use assert_approx_eq::assert_approx_eq;
    use conformal_component::parameters::{ConstantBufferStates, InternalValue, StatesMap};

    use super::*;

    #[test]
    fn guards_against_out_of_order_notes() {
        assert!(
            NoteEvents::new(
                [
                    NoteEvent {
                        sample_offset: 1,
                        data: NoteEventData::On { note_id: 0 },
                    },
                    NoteEvent {
                        sample_offset: 0,
                        data: NoteEventData::On { note_id: 1 },
                    }
                ]
                .into_iter(),
                100,
            )
            .is_none()
        );
    }

    #[test]
    fn guards_against_out_of_bounds_notes() {
        assert!(
            NoteEvents::new(
                [NoteEvent {
                    sample_offset: 100,
                    data: NoteEventData::On { note_id: 0 },
                }]
                .into_iter(),
                100,
            )
            .is_none()
        );
    }

    #[test]
    fn guards_against_out_of_range_values() {
        assert!(
            NoteEvents::new(
                [NoteEvent {
                    sample_offset: 0,
                    data: NoteEventData::ExpressionChange {
                        note_id: 0,
                        expression: NumericPerNoteExpression::Timbre,
                        value: 1.1
                    }
                }]
                .into_iter(),
                100,
            )
            .is_none()
        );
    }

    #[test]
    fn quirks_notes_pass_through_to_parameters_no_audio() {
        let state = StatesMap::from(HashMap::from([(
            timbre_param_id(1),
            InternalValue::Numeric(0.5),
        )]));
        let mpe = State::default();
        let timbre = mpe.get_numeric_expression_for_note(
            NumericPerNoteExpression::Timbre,
            NoteID {
                internals: NoteIDInternals::NoteIDFromChannelID(1),
            },
            &state,
        );
        assert_approx_eq!(timbre, 0.5);
    }

    #[test]
    fn quirks_notes_pass_through_to_parameters_audio() {
        let state = ConstantBufferStates::new(StatesMap::from(HashMap::from([(
            timbre_param_id(1),
            InternalValue::Numeric(0.5),
        )])));
        let mpe = State::default();
        let timbre = mpe.get_numeric_expression_for_note_buffer(
            NumericPerNoteExpression::Timbre,
            NoteID {
                internals: NoteIDInternals::NoteIDFromChannelID(1),
            },
            &state,
            Some(NoteEvents::new(vec![].into_iter(), 100).unwrap()),
        );
        if let NumericBufferState::Constant(value) = timbre {
            assert_approx_eq!(value, 0.5);
        } else {
            panic!("Expected constant value");
        }
    }

    #[test]
    fn expression_buffer_uses_last_event_at_sample_offset() {
        let mut mpe = State::default();
        mpe.update_for_event_data(once(NoteEventData::On { note_id: 42 }));

        let events = NoteEvents::new(
            vec![
                NoteEvent {
                    sample_offset: 10,
                    data: NoteEventData::ExpressionChange {
                        note_id: 42,
                        expression: NumericPerNoteExpression::Timbre,
                        value: 0.3,
                    },
                },
                NoteEvent {
                    sample_offset: 10,
                    data: NoteEventData::ExpressionChange {
                        note_id: 42,
                        expression: NumericPerNoteExpression::Timbre,
                        value: 0.7,
                    },
                },
            ]
            .into_iter(),
            100,
        )
        .unwrap();

        let state =
            ConstantBufferStates::new(StatesMap::from(HashMap::<String, InternalValue>::new()));
        let timbre = mpe.get_numeric_expression_for_note_buffer(
            NumericPerNoteExpression::Timbre,
            NoteID {
                internals: NoteIDInternals::NoteIDWithID(42),
            },
            &state,
            Some(events),
        );

        if let NumericBufferState::PiecewiseLinear(curve) = timbre {
            let points: Vec<_> = curve.into_iter().collect();
            assert_eq!(points.len(), 2);
            assert_eq!(points[0].sample_offset, 0);
            assert_eq!(points[1].sample_offset, 10);
            assert_approx_eq!(points[1].value, 0.7);
        } else {
            panic!("Expected piecewise linear curve");
        }
    }

    #[test]
    fn update_for_event_data_tracks_expression_state() {
        let mut mpe = State::default();
        let empty_params =
            ConstantBufferStates::new(StatesMap::from(HashMap::<String, InternalValue>::new()));

        mpe.update_for_event_data(
            [
                NoteEventData::On { note_id: 1 },
                NoteEventData::ExpressionChange {
                    note_id: 1,
                    expression: NumericPerNoteExpression::Timbre,
                    value: 0.75,
                },
            ]
            .into_iter(),
        );

        let timbre = mpe.get_numeric_expression_for_note_buffer(
            NumericPerNoteExpression::Timbre,
            NoteID {
                internals: NoteIDInternals::NoteIDWithID(1),
            },
            &empty_params,
            Some(NoteEvents::new(vec![].into_iter(), 100).unwrap()),
        );
        if let NumericBufferState::Constant(value) = timbre {
            assert_approx_eq!(value, 0.75);
        } else {
            panic!("Expected constant value");
        }
    }

    #[test]
    fn update_for_event_data_note_off_does_not_clear_state() {
        let mut mpe = State::default();
        let empty_params =
            ConstantBufferStates::new(StatesMap::from(HashMap::<String, InternalValue>::new()));

        mpe.update_for_event_data(
            [
                NoteEventData::On { note_id: 1 },
                NoteEventData::ExpressionChange {
                    note_id: 1,
                    expression: NumericPerNoteExpression::PitchBend,
                    value: 12.0,
                },
                NoteEventData::Off { note_id: 1 },
            ]
            .into_iter(),
        );

        let pitch_bend = mpe.get_numeric_expression_for_note_buffer(
            NumericPerNoteExpression::PitchBend,
            NoteID {
                internals: NoteIDInternals::NoteIDWithID(1),
            },
            &empty_params,
            Some(NoteEvents::new(vec![].into_iter(), 100).unwrap()),
        );
        if let NumericBufferState::Constant(value) = pitch_bend {
            assert_approx_eq!(value, 12.0);
        } else {
            panic!("Expected constant value");
        }
    }

    #[test]
    fn update_for_events_extracts_data_from_note_events() {
        let mut mpe = State::default();
        let empty_params =
            ConstantBufferStates::new(StatesMap::from(HashMap::<String, InternalValue>::new()));

        let events = NoteEvents::new(
            vec![
                NoteEvent {
                    sample_offset: 0,
                    data: NoteEventData::On { note_id: 5 },
                },
                NoteEvent {
                    sample_offset: 10,
                    data: NoteEventData::ExpressionChange {
                        note_id: 5,
                        expression: NumericPerNoteExpression::Aftertouch,
                        value: 0.5,
                    },
                },
            ]
            .into_iter(),
            100,
        )
        .unwrap();

        mpe.update_for_events(events);

        let aftertouch = mpe.get_numeric_expression_for_note_buffer(
            NumericPerNoteExpression::Aftertouch,
            NoteID {
                internals: NoteIDInternals::NoteIDWithID(5),
            },
            &empty_params,
            Some(NoteEvents::new(vec![].into_iter(), 100).unwrap()),
        );
        if let NumericBufferState::Constant(value) = aftertouch {
            assert_approx_eq!(value, 0.5);
        } else {
            panic!("Expected constant value");
        }
    }
}
