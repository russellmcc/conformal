use component::events::{Data, Event, NoteData, NoteID};

#[derive(Clone, Debug, PartialEq)]
enum VoiceState {
    Idle { order: usize },
    Note { order: usize, id: NoteID, pitch: u8 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    voices: Vec<VoiceState>,

    voices_compress_order_scratch: Vec<(usize, usize)>,
}

struct MaybeDoubleEvent {
    voice: usize,
    first: Option<Event>,
    second: Option<Event>,
}

impl Iterator for MaybeDoubleEvent {
    type Item = (usize, Event);

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.first.take();
        let second = self.second.take();
        match (first, second) {
            (Some(first), Some(second)) => {
                self.first = Some(second);
                self.second = None;
                Some((self.voice, first))
            }
            (Some(first), None) => {
                self.first = None;
                Some((self.voice, first))
            }
            (None, Some(_)) => {
                panic!("Invariant violation")
            }
            (None, None) => None,
        }
    }
}

impl MaybeDoubleEvent {
    fn new1(voice: usize, first: Event) -> Self {
        Self {
            voice,
            first: Some(first),
            second: None,
        }
    }
    fn new2(voice: usize, first: Event, second: Event) -> Self {
        Self {
            voice,
            first: Some(first),
            second: Some(second),
        }
    }
    fn new0() -> Self {
        Self {
            voice: 0,
            first: None,
            second: None,
        }
    }
}

fn synthetic_note_off(sample_offset: usize, id: NoteID, pitch: u8) -> Event {
    Event {
        sample_offset,
        data: Data::NoteOff {
            data: NoteData {
                channel: 0,
                id,
                pitch,
                velocity: 1.0,
                tuning: 0.0,
            },
        },
    }
}

impl State {
    pub fn new(max_voices: usize) -> Self {
        assert!(max_voices > 0);
        Self {
            voices: (0..max_voices)
                .map(|i| VoiceState::Idle { order: i })
                .collect(),
            voices_compress_order_scratch: Vec::with_capacity(max_voices),
        }
    }

    pub fn reset(&mut self) {
        let num_voices = self.voices.len();
        self.voices.clear();
        self.voices
            .extend((0..num_voices).map(|i| VoiceState::Idle { order: i }));
    }

    /// Note that the events must be sorted by time!
    pub fn dispatch_events(
        mut self,
        events: impl IntoIterator<Item = Event>,
    ) -> impl IntoIterator<Item = (usize, Event)> {
        events
            .into_iter()
            .flat_map(move |event| self.update_single_and_dispatch(event))
    }

    fn update_single_and_dispatch(&mut self, event: Event) -> MaybeDoubleEvent {
        match event.data {
            Data::NoteOn {
                data: NoteData {
                    id: note_id, pitch, ..
                },
            } => {
                let mut open_index = None;
                let mut open_index_order = None;
                let mut old_voice_index = None;
                let mut old_voice_order = None;
                let mut new_voice_order = None;
                for (index, voice_state) in self.voices.iter().enumerate() {
                    match (voice_state, open_index_order) {
                        (VoiceState::Idle { order }, None) => {
                            open_index = Some(index);
                            open_index_order = Some(*order);
                        }
                        (VoiceState::Idle { order }, Some(open_index_order_))
                            if order < &open_index_order_ =>
                        {
                            open_index = Some(index);
                            open_index_order = Some(*order);
                        }
                        (VoiceState::Note { id, .. }, _) if note_id == *id => {
                            return MaybeDoubleEvent::new1(index, event);
                        }
                        (VoiceState::Note { order, .. }, _) => {
                            if let Some(old_voice_order_) = old_voice_order {
                                if order < &old_voice_order_ {
                                    old_voice_index = Some(index);
                                    old_voice_order = Some(*order);
                                }
                            } else {
                                old_voice_index = Some(index);
                                old_voice_order = Some(*order);
                            }
                            if let Some(new_voice_order_) = new_voice_order {
                                if order > &new_voice_order_ {
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
                    if let VoiceState::Note { id, pitch, .. } =
                        self.voices[old_voice_index.unwrap()]
                    {
                        extra_off = Some(synthetic_note_off(event.sample_offset, id, pitch));
                    } else {
                        panic!("Internal error");
                    }
                    old_voice_index.unwrap()
                });

                self.voices[open_index] = VoiceState::Note {
                    id: note_id,
                    order: new_voice_order.map_or(0, |x| x + 1),
                    pitch,
                };

                if let Some(extra_off) = extra_off {
                    MaybeDoubleEvent::new2(open_index, extra_off, event)
                } else {
                    MaybeDoubleEvent::new1(open_index, event)
                }
            }
            Data::NoteOff {
                data: NoteData { id: note_id, .. },
            } => {
                let order = self
                    .voices
                    .iter()
                    .filter_map(|x| {
                        if let VoiceState::Idle { order } = x {
                            Some(order)
                        } else {
                            None
                        }
                    })
                    .max()
                    .map_or(0, |x| x + 1);
                for (index, voice_state) in self.voices.iter_mut().enumerate() {
                    match voice_state.clone() {
                        VoiceState::Note { id, .. } if note_id == id => {
                            *voice_state = VoiceState::Idle { order };
                            return MaybeDoubleEvent::new1(index, event);
                        }
                        _ => {}
                    }
                }
                MaybeDoubleEvent::new0()
            }
        }
    }

    fn compress_idle_order(&mut self) {
        self.voices_compress_order_scratch.clear();
        self.voices_compress_order_scratch.extend(
            self.voices
                .iter()
                .filter_map(|voice_state| {
                    if let VoiceState::Idle { order } = voice_state {
                        Some(*order)
                    } else {
                        None
                    }
                })
                .enumerate(),
        );
        self.voices_compress_order_scratch.sort_by_key(|x| x.1);
        for (o, (_, vo)) in self.voices_compress_order_scratch.iter_mut().enumerate() {
            *vo = o;
        }
        self.voices_compress_order_scratch.sort_by_key(|x| x.0);
        self.voices
            .iter_mut()
            .filter_map(|voice_state| {
                if let VoiceState::Idle { order } = voice_state {
                    Some(order)
                } else {
                    None
                }
            })
            .zip(self.voices_compress_order_scratch.iter().map(|x| x.1))
            .for_each(|(order, new_order)| *order = new_order);
    }

    fn compress_note_order(&mut self) {
        self.voices_compress_order_scratch.clear();
        self.voices_compress_order_scratch.extend(
            self.voices
                .iter()
                .filter_map(|voice_state| {
                    if let VoiceState::Note { order, .. } = voice_state {
                        Some(*order)
                    } else {
                        None
                    }
                })
                .enumerate(),
        );
        self.voices_compress_order_scratch.sort_by_key(|x| x.1);
        for (o, (_, vo)) in self.voices_compress_order_scratch.iter_mut().enumerate() {
            *vo = o;
        }
        self.voices_compress_order_scratch.sort_by_key(|x| x.0);
        self.voices
            .iter_mut()
            .filter_map(|voice_state| {
                if let VoiceState::Note { order, .. } = voice_state {
                    Some(order)
                } else {
                    None
                }
            })
            .zip(self.voices_compress_order_scratch.iter().map(|x| x.1))
            .for_each(|(order, new_order)| *order = new_order);
    }

    /// Note that the events must be sorted by time!
    pub fn update(&mut self, events: impl IntoIterator<Item = Event>) {
        for event in events {
            self.update_single_and_dispatch(event);
        }

        // compress orders - this keeps the `order` member bounded between buffers.
        self.compress_idle_order();
        self.compress_note_order();
    }
}

#[cfg(test)]
mod tests;
