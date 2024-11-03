use super::State;
use crate::{Event, EventData};
use conformal_component::events::{self as events, NoteData, NoteID};

fn example_note_data(pitch: u8) -> NoteData {
    NoteData {
        id: NoteID::from_pitch(pitch),
        pitch,
        velocity: 1.0,
        tuning: 0.0,
    }
}

fn example_note_on(time: usize, pitch: u8) -> events::Event {
    events::Event {
        sample_offset: time,
        data: events::Data::NoteOn {
            data: example_note_data(pitch),
        },
    }
}

fn example_note_off(time: usize, pitch: u8) -> events::Event {
    events::Event {
        sample_offset: time,
        data: events::Data::NoteOff {
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
        actual.remove(
            actual
                .iter()
                .position(|x| x == &events)
                .expect(format!("Expected to find {:?} in {:?}", events, actual_original).as_str()),
        );
    }
}

fn gather_events(
    state: &State,
    num_voices: usize,
    events: Vec<events::Event>,
) -> Vec<Vec<crate::Event>> {
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
            &State::new(2),
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
        .zip(b.into_iter())
        .map(|(mut a, b)| {
            a.extend(b);
            a
        })
        .collect()
}

#[test]
fn two_notes_go_to_two_voices_across_buffers() {
    let events_a = vec![example_note_on(0, 60), example_note_on(1, 61)];
    let mut state = State::new(2);
    let a = gather_events(&state, 2, events_a.clone());
    state.update(events_a);
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
            &State::new(2),
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
    let mut state = State::new(2);
    let a = gather_events(&state, 2, events_a.clone());
    state.update(events_a);
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
            &State::new(2),
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
    let mut state = State::new(2);
    let a = gather_events(&state, 2, events_a.clone());
    state.update(events_a);
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
    let mut state = State::new(2);
    state.update(vec![example_note_on(0, 60), example_note_on(1, 61)]);
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
