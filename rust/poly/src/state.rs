use crate::{Event, EventData};
use conformal_component::events::{NoteData, NoteID};

#[derive(Clone, Debug, PartialEq)]
enum VoicePlayingState {
    Idle {
        order: usize,
        prev_note_id: Option<NoteID>,
    },
    Note {
        order: usize,
        id: NoteID,
        pitch: u8,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    playing: VoicePlayingState,
}

/// Scratch space used by [`State::update`] to avoid repeated allocation.
///
/// This is separated from [`State`] so that it is not included in clones of `State`.
#[derive(Default)]
pub struct UpdateScratch<const MAX_VOICES: usize = 32> {
    buf: arrayvec::ArrayVec<(usize, usize), MAX_VOICES>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State<const MAX_VOICES: usize = 32> {
    voices: arrayvec::ArrayVec<Voice, MAX_VOICES>,
}

#[derive(Clone, Debug, PartialEq)]
struct EventStreamStep {
    voice: usize,
    sample_offset: usize,
    first: Option<EventData>,
    second: Option<EventData>,
}

impl Iterator for EventStreamStep {
    type Item = (usize, Event);

    fn next(&mut self) -> Option<Self::Item> {
        let first: Option<EventData> = self.first.take();
        let second = self.second.take();
        match (first, second) {
            (Some(first), Some(second)) => {
                self.first = Some(second);
                self.second = None;
                Some((
                    self.voice,
                    Event {
                        sample_offset: self.sample_offset,
                        data: first,
                    },
                ))
            }
            (Some(first), None) => {
                self.first = None;
                Some((
                    self.voice,
                    Event {
                        sample_offset: self.sample_offset,
                        data: first,
                    },
                ))
            }
            (None, Some(_)) => {
                panic!("Invariant violation")
            }
            (None, None) => None,
        }
    }
}

impl EventStreamStep {
    fn new1(voice: usize, first: Event) -> Self {
        Self {
            voice,
            sample_offset: first.sample_offset,
            first: Some(first.data),
            second: None,
        }
    }
    fn new2(voice: usize, first: Event, second: EventData) -> Self {
        Self {
            voice,
            sample_offset: first.sample_offset,
            first: Some(first.data),
            second: Some(second),
        }
    }
    fn new0() -> Self {
        Self {
            voice: 0,
            sample_offset: 0,
            first: None,
            second: None,
        }
    }
}

fn synthetic_note_off(id: NoteID, pitch: u8) -> EventData {
    EventData::NoteOff {
        data: NoteData {
            id,
            pitch,
            velocity: 1.0,
            tuning: 0.0,
        },
    }
}

impl<const MAX_VOICES: usize> State<MAX_VOICES> {
    pub fn new() -> Self {
        Self {
            voices: (0..MAX_VOICES)
                .map(|i| Voice {
                    playing: VoicePlayingState::Idle {
                        order: i,
                        prev_note_id: None,
                    },
                })
                .collect(),
        }
    }

    pub fn note_id_for_voice(&self, voice_index: usize) -> Option<NoteID> {
        self.voices
            .get(voice_index)
            .and_then(|voice| match &voice.playing {
                VoicePlayingState::Note { id, .. } => Some(*id),
                VoicePlayingState::Idle { prev_note_id, .. } => *prev_note_id,
            })
    }

    pub fn clear_prev_note_id_for_voice(&mut self, voice_index: usize) {
        if let Some(VoicePlayingState::Idle { prev_note_id, .. }) = self
            .voices
            .get_mut(voice_index)
            .map(|voice| &mut voice.playing)
        {
            *prev_note_id = None;
        }
    }

    pub fn reset(&mut self) {
        let num_voices = self.voices.len();
        self.voices.clear();
        self.voices.extend((0..num_voices).map(|i| Voice {
            playing: VoicePlayingState::Idle {
                order: i,
                prev_note_id: None,
            },
        }));
    }

    /// Note that the events must be sorted by time!
    pub fn dispatch_events(
        mut self,
        events: impl Iterator<Item = Event> + Clone,
    ) -> impl Iterator<Item = (usize, Event)> + Clone {
        events
            .into_iter()
            .flat_map(move |event| self.update_state_and_dispatch_for_event(&event))
    }

    fn update_state_and_dispatch_for_event(&mut self, event: &Event) -> EventStreamStep {
        match &event.data {
            EventData::NoteOn { data } => {
                self.update_state_and_dispatch_for_note_on(event.sample_offset, data)
            }
            EventData::NoteOff { data } => {
                self.update_state_and_dispatch_for_note_off(event.sample_offset, data)
            }
        }
    }

    fn update_state_and_dispatch_for_note_on(
        &mut self,
        sample_offset: usize,
        data: &NoteData,
    ) -> EventStreamStep {
        let mut open_index = None;
        let mut open_index_order = None;
        let mut old_voice_index = None;
        let mut old_voice_order = None;
        let mut new_voice_order = None;
        for (index, Voice { playing }) in self.voices.iter_mut().enumerate() {
            match (playing, open_index_order) {
                (VoicePlayingState::Idle { order, .. }, None) => {
                    open_index = Some(index);
                    open_index_order = Some(*order);
                }
                (VoicePlayingState::Idle { order, .. }, Some(open_index_order_))
                    if *order < open_index_order_ =>
                {
                    open_index = Some(index);
                    open_index_order = Some(*order);
                }
                (VoicePlayingState::Note { id, .. }, _) if data.id == *id => {
                    return EventStreamStep::new1(
                        index,
                        Event {
                            sample_offset,
                            data: EventData::NoteOn { data: *data },
                        },
                    );
                }
                (VoicePlayingState::Note { order, .. }, _) => {
                    if let Some(old_voice_order_) = old_voice_order {
                        if *order < old_voice_order_ {
                            old_voice_index = Some(index);
                            old_voice_order = Some(*order);
                        }
                    } else {
                        old_voice_index = Some(index);
                        old_voice_order = Some(*order);
                    }
                    if let Some(new_voice_order_) = new_voice_order {
                        if *order > new_voice_order_ {
                            new_voice_order = Some(*order);
                        }
                    } else {
                        new_voice_order = Some(*order);
                    }
                }
                _ => {}
            }
        }

        let mut extra_off = None;
        let open_index = open_index.unwrap_or_else(|| {
            // If we got here, no notes are open - we have to steal one!
            // We always steal the oldest note.

            // Make a synthetic note off event for the oldest note
            if let VoicePlayingState::Note { id, pitch, .. } =
                self.voices[old_voice_index.unwrap()].playing
            {
                extra_off = Some(synthetic_note_off(id, pitch));
            } else {
                panic!("Internal error");
            }
            old_voice_index.unwrap()
        });

        self.voices[open_index].playing = VoicePlayingState::Note {
            id: data.id,
            order: new_voice_order.map_or(0, |x| x + 1),
            pitch: data.pitch,
        };

        if let Some(extra_off) = extra_off {
            EventStreamStep::new2(
                open_index,
                Event {
                    sample_offset,
                    data: extra_off,
                },
                EventData::NoteOn { data: *data },
            )
        } else {
            EventStreamStep::new1(
                open_index,
                Event {
                    sample_offset,
                    data: EventData::NoteOn { data: *data },
                },
            )
        }
    }

    fn update_state_and_dispatch_for_note_off(
        &mut self,
        sample_offset: usize,
        data: &NoteData,
    ) -> EventStreamStep {
        let order = self
            .voices
            .iter()
            .filter_map(|x| {
                if let VoicePlayingState::Idle { order, .. } = x.playing {
                    Some(order)
                } else {
                    None
                }
            })
            .max()
            .map_or(0, |x| x + 1);
        for (index, voice_state) in self.voices.iter_mut().enumerate() {
            match voice_state.playing {
                VoicePlayingState::Note { id, .. } if data.id == id => {
                    voice_state.playing = VoicePlayingState::Idle {
                        order,
                        prev_note_id: Some(id),
                    };
                    return EventStreamStep::new1(
                        index,
                        Event {
                            sample_offset,
                            data: EventData::NoteOff { data: *data },
                        },
                    );
                }
                _ => {}
            }
        }
        EventStreamStep::new0()
    }

    fn compress_idle_order(&mut self, scratch: &mut UpdateScratch<MAX_VOICES>) {
        scratch.buf.clear();
        scratch.buf.extend(
            self.voices
                .iter()
                .filter_map(|voice_state| {
                    if let VoicePlayingState::Idle { order, .. } = voice_state.playing {
                        Some(order)
                    } else {
                        None
                    }
                })
                .enumerate(),
        );
        scratch.buf.sort_by_key(|x| x.1);
        for (o, (_, vo)) in scratch.buf.iter_mut().enumerate() {
            *vo = o;
        }
        scratch.buf.sort_by_key(|x| x.0);
        self.voices
            .iter_mut()
            .filter_map(|voice_state| {
                if let VoicePlayingState::Idle { ref mut order, .. } = voice_state.playing {
                    Some(order)
                } else {
                    None
                }
            })
            .zip(scratch.buf.iter().map(|x| x.1))
            .for_each(|(order, new_order)| *order = new_order);
    }

    fn compress_note_order(&mut self, scratch: &mut UpdateScratch<MAX_VOICES>) {
        scratch.buf.clear();
        scratch.buf.extend(
            self.voices
                .iter_mut()
                .filter_map(|voice_state| {
                    if let VoicePlayingState::Note { ref mut order, .. } = voice_state.playing {
                        Some(*order)
                    } else {
                        None
                    }
                })
                .enumerate(),
        );
        scratch.buf.sort_by_key(|x| x.1);
        for (o, (_, vo)) in scratch.buf.iter_mut().enumerate() {
            *vo = o;
        }
        scratch.buf.sort_by_key(|x| x.0);
        self.voices
            .iter_mut()
            .filter_map(|voice_state| {
                if let VoicePlayingState::Note { ref mut order, .. } = voice_state.playing {
                    Some(order)
                } else {
                    None
                }
            })
            .zip(scratch.buf.iter().map(|x| x.1))
            .for_each(|(order, new_order)| *order = new_order);
    }

    /// Note that the events must be sorted by time!
    pub fn update(
        &mut self,
        events: impl IntoIterator<Item = Event>,
        scratch: &mut UpdateScratch<MAX_VOICES>,
    ) {
        for event in events {
            self.update_state_and_dispatch_for_event(&event);
        }

        // compress orders - this keeps the `order` member bounded between buffers.
        self.compress_idle_order(scratch);
        self.compress_note_order(scratch);
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::{Event, EventData};
    use conformal_component::events::{NoteData, NoteID};

    fn example_note_data(pitch: u8) -> NoteData {
        NoteData {
            id: NoteID::from_pitch(pitch),
            pitch,
            velocity: 1.0,
            tuning: 0.0,
        }
    }

    fn example_note_on(time: usize, pitch: u8) -> Event {
        Event {
            sample_offset: time,
            data: EventData::NoteOn {
                data: example_note_data(pitch),
            },
        }
    }

    fn example_note_off(time: usize, pitch: u8) -> Event {
        Event {
            sample_offset: time,
            data: EventData::NoteOff {
                data: example_note_data(pitch),
            },
        }
    }

    fn expected_note_on(time: usize, pitch: u8) -> Event {
        Event {
            sample_offset: time,
            data: EventData::NoteOn {
                data: example_note_data(pitch),
            },
        }
    }

    fn expected_note_off(time: usize, pitch: u8) -> Event {
        Event {
            sample_offset: time,
            data: EventData::NoteOff {
                data: example_note_data(pitch),
            },
        }
    }

    // Note that we always allow the voice state logic to shuffle the voices!
    fn assert_events_match(expected: Vec<Vec<Event>>, mut actual: Vec<Vec<Event>>) {
        let actual_original = actual.clone();
        assert_eq!(expected.len(), actual.len());
        for events in expected {
            actual.remove(actual.iter().position(|x| x == &events).expect(
                format!("Expected to find {:?} in {:?}", events, actual_original).as_str(),
            ));
        }
    }

    fn gather_events<const MAX_VOICES: usize>(state: &State<MAX_VOICES>, num_voices: usize, events: Vec<Event>) -> Vec<Vec<Event>> {
        (0..num_voices)
            .into_iter()
            .map(|voice_index| {
                state
                    .clone()
                    .dispatch_events(events.iter().cloned())
                    .into_iter()
                    .filter_map(|(index, event)| {
                        if index == voice_index {
                            Some(event)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn two_notes_go_to_two_voices() {
        assert_events_match(
            vec![
                vec![expected_note_on(1, 61), expected_note_off(3, 61)],
                vec![expected_note_on(0, 60), expected_note_off(2, 60)],
            ],
            gather_events(
                &State::<2>::new(),
                2,
                vec![
                    example_note_on(0, 60),
                    example_note_on(1, 61),
                    example_note_off(2, 60),
                    example_note_off(3, 61),
                ],
            ),
        );
    }

    fn cat_events(a: Vec<Vec<Event>>, b: Vec<Vec<Event>>) -> Vec<Vec<Event>> {
        a.into_iter()
            .zip(b)
            .map(|(mut a, b)| {
                a.extend(b);
                a
            })
            .collect()
    }

    #[test]
    fn two_notes_go_to_two_voices_across_buffers() {
        let events_a = vec![example_note_on(0, 60), example_note_on(1, 61)];
        let mut state = State::<2>::new();
        let a = gather_events(&state, 2, events_a.clone());
        state.update(events_a, &mut Default::default());
        let b = gather_events(
            &state,
            2,
            vec![example_note_off(2, 60), example_note_off(3, 61)],
        );
        assert_events_match(
            vec![
                vec![expected_note_on(0, 60), expected_note_off(2, 60)],
                vec![expected_note_on(1, 61), expected_note_off(3, 61)],
            ],
            cat_events(a, b),
        );
    }

    #[test]
    fn new_note_goes_to_longest_off_voice() {
        assert_events_match(
            vec![
                vec![expected_note_on(0, 61), expected_note_off(3, 61)],
                vec![
                    expected_note_on(0, 60),
                    expected_note_off(2, 60),
                    expected_note_on(5, 62),
                ],
            ],
            gather_events(
                &State::<2>::new(),
                2,
                vec![
                    example_note_on(0, 60),
                    example_note_on(0, 61),
                    example_note_off(2, 60),
                    example_note_off(3, 61),
                    example_note_on(5, 62),
                ],
            ),
        );
    }

    #[test]
    fn new_note_goes_to_longest_off_across_buffers() {
        let events_a = vec![
            example_note_on(0, 60),
            example_note_on(0, 61),
            example_note_off(66, 60),
        ];
        let mut state = State::<2>::new();
        let a = gather_events(&state, 2, events_a.clone());
        state.update(events_a, &mut Default::default());
        let b = gather_events(
            &state,
            2,
            vec![example_note_off(0, 61), example_note_on(5, 62)],
        );
        assert_events_match(
            vec![
                vec![
                    expected_note_on(0, 60),
                    expected_note_off(66, 60),
                    expected_note_on(5, 62),
                ],
                vec![expected_note_on(0, 61), expected_note_off(0, 61)],
            ],
            cat_events(a, b),
        );
    }

    #[test]
    fn drops_from_oldest_note() {
        assert_events_match(
            vec![
                vec![expected_note_on(1, 61)],
                vec![
                    expected_note_on(0, 60),
                    expected_note_off(2, 60),
                    expected_note_on(2, 62),
                ],
            ],
            gather_events(
                &State::<2>::new(),
                2,
                vec![
                    example_note_on(0, 60),
                    example_note_on(1, 61),
                    example_note_on(2, 62),
                ],
            ),
        );
    }

    #[test]
    fn drops_from_oldest_note_across_buffers() {
        let events_a = vec![example_note_on(0, 60), example_note_on(1, 61)];
        let mut state = State::<2>::new();
        let a = gather_events(&state, 2, events_a.clone());
        state.update(events_a, &mut Default::default());
        let b = gather_events(&state, 2, vec![example_note_on(0, 62)]);
        assert_events_match(
            vec![
                vec![
                    expected_note_on(0, 60),
                    expected_note_off(0, 60),
                    expected_note_on(0, 62),
                ],
                vec![expected_note_on(1, 61)],
            ],
            cat_events(a, b),
        );
    }

    #[test]
    fn reset_restors_state() {
        let mut state = State::<2>::new();
        state.update(vec![example_note_on(0, 60), example_note_on(1, 61)], &mut Default::default());
        state.reset();
        assert_events_match(
            vec![vec![expected_note_on(0, 62)], vec![expected_note_on(1, 63)]],
            gather_events(
                &state,
                2,
                vec![example_note_on(0, 62), example_note_on(1, 63)],
            ),
        );
    }
}
