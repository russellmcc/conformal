use vst3::{
    ComRef,
    Steinberg::Vst::{IEventList, IEventListTrait},
};

use conformal_component::events::{Data, Event, NoteData, NoteID};

unsafe fn get_event(
    event_list: ComRef<'_, IEventList>,
    index: i32,
) -> Option<vst3::Steinberg::Vst::Event> {
    let mut event = vst3::Steinberg::Vst::Event {
        busIndex: 0,
        sampleOffset: 0,
        ppqPosition: 0.0,
        flags: 0,
        r#type: 0,
        __field0: vst3::Steinberg::Vst::Event__type0 {
            noteOn: vst3::Steinberg::Vst::NoteOnEvent {
                channel: 0,
                pitch: 0,
                tuning: 0.0,
                velocity: 0.0,
                length: 0,
                noteId: 0,
            },
        },
    };
    let result = event_list.getEvent(index, &mut event);
    if result != vst3::Steinberg::kResultOk {
        return None;
    }
    Some(event)
}

unsafe fn convert_event(event: &vst3::Steinberg::Vst::Event) -> Option<Event> {
    if event.sampleOffset < 0 {
        return None;
    }
    match u32::from(event.r#type) {
        vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent => {
            let channel = u8::try_from(event.__field0.noteOn.channel).ok()?;
            let pitch = u8::try_from(event.__field0.noteOn.pitch).ok()?;
            Some(Event {
                sample_offset: event.sampleOffset as usize,
                data: Data::NoteOn {
                    data: NoteData {
                        channel,
                        pitch,
                        tuning: event.__field0.noteOn.tuning,
                        velocity: event.__field0.noteOn.velocity,
                        id: if event.__field0.noteOn.noteId == -1 {
                            NoteID::from_pitch(pitch)
                        } else {
                            NoteID::from_id(event.__field0.noteOn.noteId)
                        },
                    },
                },
            })
        }
        vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent => {
            let channel = u8::try_from(event.__field0.noteOn.channel).ok()?;
            let pitch = u8::try_from(event.__field0.noteOn.pitch).ok()?;
            Some(Event {
                sample_offset: event.sampleOffset as usize,
                data: Data::NoteOff {
                    data: NoteData {
                        channel,
                        pitch,
                        tuning: event.__field0.noteOff.tuning,
                        velocity: event.__field0.noteOff.velocity,
                        id: if event.__field0.noteOff.noteId == -1 {
                            NoteID::from_pitch(pitch)
                        } else {
                            NoteID::from_id(event.__field0.noteOff.noteId)
                        },
                    },
                },
            })
        }
        _ => None,
    }
}

pub unsafe fn event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> impl Iterator<Item = Event> + '_ + Clone {
    (0..event_list.getEventCount()).filter_map(move |i| -> Option<Event> {
        get_event(event_list, i)
            .as_ref()
            .and_then(|x| unsafe { convert_event(x) })
    })
}

pub unsafe fn all_zero_event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> Option<impl Iterator<Item = Data> + Clone + '_> {
    let i = (0..event_list.getEventCount()).filter_map(move |i| -> Option<Event> {
        get_event(event_list, i)
            .as_ref()
            .and_then(|x| unsafe { convert_event(x) })
    });
    if i.clone().any(|x| x.sample_offset != 0) {
        None
    } else {
        Some(i.map(|x| x.data))
    }
}
