use vst3::{
    ComRef,
    Steinberg::Vst::{IEventList, IEventListTrait},
};

use conformal_component::{
    events::{Data, Event, NoteData, NoteID},
    synth::NumericPerNoteExpression,
};

use crate::{mpe, u32_to_enum};

unsafe fn get_event(
    event_list: ComRef<'_, IEventList>,
    index: i32,
) -> Option<vst3::Steinberg::Vst::Event> {
    unsafe {
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
        let result = event_list.getEvent(index, &raw mut event);
        if result != vst3::Steinberg::kResultOk {
            return None;
        }
        Some(event)
    }
}

fn get_note_id(pitch: u8, channel: i16, note_id: i32) -> NoteID {
    if note_id != -1 {
        return NoteID::from_id(note_id);
    }
    if channel != 0 {
        NoteID::from_channel_id(channel)
    } else {
        NoteID::from_pitch(pitch)
    }
}

fn get_mpe_note_id(note_id: i32) -> Option<i32> {
    if note_id == -1 { None } else { Some(note_id) }
}

unsafe fn convert_event(event: &vst3::Steinberg::Vst::Event) -> Option<Event> {
    unsafe {
        if event.sampleOffset < 0 {
            return None;
        }
        match u32_to_enum(u32::from(event.r#type)) {
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent) => {
                let pitch = u8::try_from(event.__field0.noteOn.pitch).ok()?;
                let channel = event.__field0.noteOn.channel;
                Some(Event {
                    sample_offset: event.sampleOffset as usize,
                    data: Data::NoteOn {
                        data: NoteData {
                            pitch,
                            tuning: event.__field0.noteOn.tuning,
                            velocity: event.__field0.noteOn.velocity,
                            id: get_note_id(pitch, channel, event.__field0.noteOn.noteId),
                        },
                    },
                })
            }
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent) => {
                let pitch = u8::try_from(event.__field0.noteOff.pitch).ok()?;
                let channel = event.__field0.noteOff.channel;
                Some(Event {
                    sample_offset: event.sampleOffset as usize,
                    data: Data::NoteOff {
                        data: NoteData {
                            pitch,
                            tuning: event.__field0.noteOff.tuning,
                            velocity: event.__field0.noteOff.velocity,
                            id: get_note_id(pitch, channel, event.__field0.noteOff.noteId),
                        },
                    },
                })
            }
            _ => None,
        }
    }
}

unsafe fn convert_mpe_event(event: &vst3::Steinberg::Vst::Event) -> Option<mpe::NoteEvent> {
    unsafe {
        if event.sampleOffset < 0 {
            return None;
        }
        match u32_to_enum(u32::from(event.r#type)) {
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent) => {
                let note_id = event.__field0.noteOn.noteId;
                let note_id = get_mpe_note_id(note_id)?;
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::On { note_id },
                })
            }
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent) => {
                let note_id = event.__field0.noteOff.noteId;
                let note_id = get_mpe_note_id(note_id)?;
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::Off { note_id },
                })
            }
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kPolyPressureEvent) => {
                let note_id = get_mpe_note_id(event.__field0.polyPressure.noteId)?;
                let pressure = event.__field0.polyPressure.pressure;
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::ExpressionChange {
                        note_id,
                        expression: NumericPerNoteExpression::Aftertouch,
                        value: pressure,
                    },
                })
            }
            Ok(vst3::Steinberg::Vst::Event_::EventTypes_::kNoteExpressionValueEvent) => {
                let note_id = event.__field0.noteExpressionValue.noteId;
                let note_id = get_mpe_note_id(note_id)?;
                let expression = match event.__field0.noteExpressionValue.typeId {
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID => {
                        NumericPerNoteExpression::PitchBend
                    }
                    super::NOTE_EXPRESSION_TIMBRE_TYPE_ID => NumericPerNoteExpression::Timbre,
                    _ => return None,
                };
                #[allow(clippy::cast_possible_truncation)]
                let value = match expression {
                    NumericPerNoteExpression::PitchBend => {
                        (event.__field0.noteExpressionValue.value as f32 - 0.5) * 240.0
                    }
                    NumericPerNoteExpression::Timbre | NumericPerNoteExpression::Aftertouch => {
                        event.__field0.noteExpressionValue.value as f32
                    }
                };
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::ExpressionChange {
                        note_id,
                        expression,
                        value,
                    },
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use conformal_component::events::{Data, NoteData, NoteIDInternals};

    use super::*;

    fn note_on_event(channel: i16, note_id: i32, pitch: i16) -> vst3::Steinberg::Vst::Event {
        vst3::Steinberg::Vst::Event {
            busIndex: 0,
            sampleOffset: 0,
            ppqPosition: 0.0,
            flags: 0,
            r#type: vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent as u16,
            __field0: vst3::Steinberg::Vst::Event__type0 {
                noteOn: vst3::Steinberg::Vst::NoteOnEvent {
                    channel,
                    pitch,
                    tuning: 0.0,
                    velocity: 0.5,
                    length: 0,
                    noteId: note_id,
                },
            },
        }
    }

    fn note_off_event(channel: i16, note_id: i32, pitch: i16) -> vst3::Steinberg::Vst::Event {
        vst3::Steinberg::Vst::Event {
            busIndex: 0,
            sampleOffset: 0,
            ppqPosition: 0.0,
            flags: 0,
            r#type: vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent as u16,
            __field0: vst3::Steinberg::Vst::Event__type0 {
                noteOff: vst3::Steinberg::Vst::NoteOffEvent {
                    channel,
                    pitch,
                    tuning: 0.0,
                    velocity: 0.5,
                    noteId: note_id,
                },
            },
        }
    }

    #[test]
    fn note_ids_prefer_vst_note_id_over_channel() {
        let event = note_on_event(3, 42, 64);
        let converted = unsafe { convert_event(&event) }.unwrap();
        assert_eq!(
            converted,
            Event {
                sample_offset: 0,
                data: Data::NoteOn {
                    data: NoteData {
                        id: NoteID::from_id(42),
                        pitch: 64,
                        velocity: 0.5,
                        tuning: 0.0,
                    },
                },
            }
        );
    }

    #[test]
    fn official_mpe_note_on_accepts_nonzero_channel_when_note_id_present() {
        let event = note_on_event(7, 42, 64);
        let converted = unsafe { convert_mpe_event(&event) }.unwrap();
        match converted.data {
            mpe::NoteEventData::On { note_id } => assert_eq!(note_id, 42),
            _ => panic!("expected note on"),
        }
    }

    #[test]
    fn official_mpe_note_off_accepts_nonzero_channel_when_note_id_present() {
        let event = note_off_event(7, 42, 64);
        let converted = unsafe { convert_mpe_event(&event) }.unwrap();
        match converted.data {
            mpe::NoteEventData::Off { note_id } => assert_eq!(note_id, 42),
            _ => panic!("expected note off"),
        }
    }

    #[test]
    fn quirks_note_without_note_id_still_uses_channel_id() {
        let event = note_on_event(7, -1, 64);
        let converted = unsafe { convert_event(&event) }.unwrap();
        match converted.data {
            Data::NoteOn { data } => {
                assert_eq!(data.id.internals, NoteIDInternals::NoteIDFromChannelID(7));
            }
            _ => panic!("expected note on"),
        }
        assert!(unsafe { convert_mpe_event(&event) }.is_none());
    }
}

pub unsafe fn event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> impl Iterator<Item = Event> + Clone {
    unsafe {
        (0..event_list.getEventCount()).filter_map(move |i| -> Option<Event> {
            get_event(event_list, i)
                .as_ref()
                .and_then(|x| convert_event(x))
        })
    }
}

pub unsafe fn all_zero_event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> Option<impl Iterator<Item = Data> + Clone> {
    unsafe {
        let i = (0..event_list.getEventCount()).filter_map(move |i| -> Option<Event> {
            get_event(event_list, i)
                .as_ref()
                .and_then(|x| convert_event(x))
        });
        if i.clone().any(|x| x.sample_offset != 0) {
            None
        } else {
            Some(i.map(|x| x.data))
        }
    }
}

pub unsafe fn mpe_event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> impl Iterator<Item = mpe::NoteEvent> + Clone {
    unsafe {
        (0..event_list.getEventCount()).filter_map(move |i| -> Option<mpe::NoteEvent> {
            get_event(event_list, i)
                .as_ref()
                .and_then(|x| convert_mpe_event(x))
        })
    }
}

pub unsafe fn all_zero_mpe_event_iterator(
    event_list: ComRef<'_, IEventList>,
) -> Option<impl Iterator<Item = mpe::NoteEventData> + Clone> {
    unsafe {
        let i = (0..event_list.getEventCount()).filter_map(move |i| -> Option<mpe::NoteEvent> {
            get_event(event_list, i)
                .as_ref()
                .and_then(|x| convert_mpe_event(x))
        });
        if i.clone().any(|x| x.sample_offset != 0) {
            None
        } else {
            Some(i.map(|x| x.data))
        }
    }
}
