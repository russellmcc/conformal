use vst3::{
    ComRef,
    Steinberg::Vst::{IEventList, IEventListTrait},
};

use conformal_component::{
    events::{Data, Event, NoteData, NoteID, NoteIDInternals},
    synth::NumericPerNoteExpression,
};

use crate::mpe;

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

unsafe fn convert_event(event: &vst3::Steinberg::Vst::Event) -> Option<Event> {
    unsafe {
        if event.sampleOffset < 0 {
            return None;
        }
        match u32::from(event.r#type) {
            vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent => {
                let pitch = u8::try_from(event.__field0.noteOn.pitch).ok()?;
                let channel = event.__field0.noteOn.channel;
                Some(Event {
                    sample_offset: event.sampleOffset as usize,
                    data: Data::NoteOn {
                        data: NoteData {
                            pitch,
                            tuning: event.__field0.noteOn.tuning,
                            velocity: event.__field0.noteOn.velocity,
                            id: if channel != 0 {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDFromChannelID(channel),
                                }
                            } else if event.__field0.noteOn.noteId == -1 {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDFromPitch(pitch),
                                }
                            } else {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDWithID(
                                        event.__field0.noteOn.noteId,
                                    ),
                                }
                            },
                        },
                    },
                })
            }
            vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent => {
                let pitch = u8::try_from(event.__field0.noteOff.pitch).ok()?;
                let channel = event.__field0.noteOff.channel;
                Some(Event {
                    sample_offset: event.sampleOffset as usize,
                    data: Data::NoteOff {
                        data: NoteData {
                            pitch,
                            tuning: event.__field0.noteOff.tuning,
                            velocity: event.__field0.noteOff.velocity,
                            id: if channel != 0 {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDFromChannelID(channel),
                                }
                            } else if event.__field0.noteOff.noteId == -1 {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDFromPitch(pitch),
                                }
                            } else {
                                NoteID {
                                    internals: NoteIDInternals::NoteIDWithID(
                                        event.__field0.noteOff.noteId,
                                    ),
                                }
                            },
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
        match u32::from(event.r#type) {
            vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOnEvent => {
                let channel = event.__field0.noteOn.channel;
                let note_id = event.__field0.noteOn.noteId;
                if note_id == -1 || channel != 0 {
                    return None;
                }
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::On { note_id },
                })
            }
            vst3::Steinberg::Vst::Event_::EventTypes_::kNoteOffEvent => {
                let channel = event.__field0.noteOff.channel;
                let note_id = event.__field0.noteOff.noteId;
                if note_id == -1 || channel != 0 {
                    return None;
                }
                Some(mpe::NoteEvent {
                    sample_offset: event.sampleOffset as usize,
                    data: mpe::NoteEventData::Off { note_id },
                })
            }
            vst3::Steinberg::Vst::Event_::EventTypes_::kNoteExpressionValueEvent => {
                let note_id = event.__field0.noteExpressionValue.noteId;
                if note_id == -1 {
                    return None;
                }
                let expression = match event.__field0.noteExpressionValue.typeId {
                    vst3::Steinberg::Vst::NoteExpressionTypeIDs_::kTuningTypeID => {
                        NumericPerNoteExpression::PitchBend
                    }
                    super::NOTE_EXPRESSION_TIMBRE_TYPE_ID => NumericPerNoteExpression::Timbre,
                    super::NOTE_EXPRESSION_AFTERTOUCH_TYPE_ID => {
                        NumericPerNoteExpression::Aftertouch
                    }
                    _ => return None,
                };
                let value = match expression {
                    NumericPerNoteExpression::PitchBend => {
                        (event.__field0.noteExpressionValue.value as f32 - 0.5) * 240.0
                    }
                    NumericPerNoteExpression::Timbre => {
                        event.__field0.noteExpressionValue.value as f32
                    }
                    NumericPerNoteExpression::Aftertouch => {
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
