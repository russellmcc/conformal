use super::{Data, Event, Events, NoteData, NoteID};

static EXAMPLE_NOTE: NoteData = NoteData {
    id: NoteID::from_pitch(60),
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
