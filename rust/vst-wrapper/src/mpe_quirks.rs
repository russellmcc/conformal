use conformal_component::{
    audio::approx_eq,
    events::{
        self, to_vst_note_channel_for_mpe_quirks, Event, Events, NoteExpression,
        NoteExpressionData, NoteID,
    },
    parameters::{self, hash_id, BufferStates, Flags, IdHash, States, TypeSpecificInfo},
};

use crate::HostInfo;

const MPE_QUIRKS_PREFIX: &str = "_conformal_internal_mpe_quirks";

pub fn aftertouch_param_id(channel_index: i16) -> String {
    format!("{MPE_QUIRKS_PREFIX}_aftertouch_{channel_index}")
}

pub fn pitch_param_id(channel_index: i16) -> String {
    format!("{MPE_QUIRKS_PREFIX}_pitch_{channel_index}")
}

pub fn timbre_param_id(channel_index: i16) -> String {
    format!("{MPE_QUIRKS_PREFIX}_timbre_{channel_index}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    SupportQuirks,
    DoNotSupportQuirks,
}

// "MPE Quirks" is a _really_ unfortunate vst3 note expression implementation that is used
// in several hosts, including Ableton as of 12.0.25. Instead of using the vst3 note expression
// system, it insteads uses actual MPE messages that are expected to be midi-mapped to parameters
// in the plugin.
//
// We begrudgingly support this, since we want our plug-ins to work with Ableton, even though
// it means adding _several_ completely unnecessary dummy parameters, and a bunch of extra code.
pub fn should_support(_: &HostInfo) -> Support {
    // Currently support "mpe quirks" in all hosts. If this implementation of note expression
    // becomes less common, we might want to use only a list of hosts known to use this quirky
    // implementation. There isn't much of a downside to supporting the quirks, since we
    // don't support multi-channel synths anyways. When and if we do, we'll have to reconsider this.
    Support::SupportQuirks
}

pub fn parameters() -> impl Iterator<Item = parameters::Info> + Clone + 'static {
    (1..16).flat_map(|idx| {
        [
            parameters::Info {
                unique_id: aftertouch_param_id(idx),
                title: format!("MPE Quirks Aftertouch {idx}"),
                short_title: format!("MPE After {idx}"),
                flags: Flags { automatable: false },
                type_specific: TypeSpecificInfo::Numeric {
                    default: 0.0,
                    valid_range: 0.0..=1.0,
                    units: None,
                },
            },
            parameters::Info {
                unique_id: pitch_param_id(idx),
                title: format!("MPE Quirks Pitch {idx}"),
                short_title: format!("MPE Pitch {idx}"),
                flags: Flags { automatable: false },
                type_specific: TypeSpecificInfo::Numeric {
                    default: 0.0,
                    valid_range: -1.0..=1.0,
                    units: None,
                },
            },
            parameters::Info {
                unique_id: timbre_param_id(idx),
                title: format!("MPE Quirks Timbre {idx}"),
                short_title: format!("MPE Timbre {idx}"),
                flags: Flags { automatable: false },
                type_specific: TypeSpecificInfo::Numeric {
                    default: 0.0,
                    valid_range: 0.0..=1.0,
                    units: None,
                },
            },
        ]
    })
}

fn update_mpe_quirks_state(ev: &events::Data, quirks_state: &mut State) {
    match ev {
        events::Data::NoteOn { data } => {
            if let c @ 1..=16 = to_vst_note_channel_for_mpe_quirks(data.id) {
                quirks_state.channels[(c - 1) as usize] = Some(Default::default());
            }
        }
        events::Data::NoteOff { data } => {
            if let c @ 1..=16 = to_vst_note_channel_for_mpe_quirks(data.id) {
                quirks_state.channels[(c - 1) as usize] = None;
            }
        }
        events::Data::NoteExpression { .. } => {}
    }
}

pub fn add_mpe_quirk_events_buffer(
    events: impl IntoIterator<Item = Event> + Clone,
    quirks_state: State,
    buffer_states: impl BufferStates,
    buffer_size: usize,
) -> Events<impl IntoIterator<Item = Event> + Clone> {
    // TODO: make work
    Events::new(events, buffer_size).unwrap()
}

pub fn update_mpe_quirk_events_buffer(
    events: impl IntoIterator<Item = Event> + Clone,
    quirks_state: &mut State,
    buffer_states: impl BufferStates,
) {
    // TODO
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parameter {
    Pitch,
    Aftertouch,
    Timbre,
}

fn next(parameter: Parameter, channel: i16) -> Option<(Parameter, i16)> {
    match parameter {
        Parameter::Pitch => Some((Parameter::Aftertouch, channel)),
        Parameter::Aftertouch => Some((Parameter::Timbre, channel)),
        Parameter::Timbre => {
            if channel + 1 >= 16 {
                None
            } else {
                Some((Parameter::Pitch, channel + 1))
            }
        }
    }
}

fn note_expression_data(parameter: Parameter, channel: i16, value: f32) -> NoteExpressionData {
    NoteExpressionData {
        id: NoteID::from_channel_for_mpe_quirks(channel),
        expression: match parameter {
            Parameter::Pitch => NoteExpression::PitchBend(value),
            Parameter::Aftertouch => NoteExpression::Aftertouch(value),
            Parameter::Timbre => NoteExpression::Timbre(value),
        },
    }
}

#[derive(Debug, Clone)]
enum NoAudioIterState<E, S> {
    IteratingEvents {
        events: E,
        params: S,
    },
    IteratingParams {
        params: S,
        ch: i16,
        parameter: Parameter,
    },
    Done,
}

#[derive(Debug, Clone)]
struct NoAudioIter<E, S> {
    iter_state: NoAudioIterState<E, S>,
    state: State,
}

impl<E: Iterator<Item = events::Data>, S: States> NoAudioIter<E, S> {
    fn new(events: E, quirks_state: State, params: S) -> Self {
        NoAudioIter {
            iter_state: NoAudioIterState::IteratingEvents { events, params },
            state: quirks_state,
        }
    }
}

impl<E: Iterator<Item = events::Data>, S: States> Iterator for NoAudioIter<E, S> {
    type Item = Option<events::Data>;

    fn next(&mut self) -> Option<Self::Item> {
        let (ret, next_iter_state) =
            match std::mem::replace(&mut self.iter_state, NoAudioIterState::Done) {
                NoAudioIterState::IteratingEvents { mut events, params } => {
                    let ret = events.next();
                    if let Some(data) = ret {
                        update_mpe_quirks_state(&data, &mut self.state);
                        (
                            Some(Some(data)),
                            NoAudioIterState::IteratingEvents { events, params },
                        )
                    } else {
                        (
                            Some(None),
                            NoAudioIterState::IteratingParams {
                                params,
                                ch: 1,
                                parameter: Parameter::Pitch,
                            },
                        )
                    }
                }
                NoAudioIterState::IteratingParams {
                    params,
                    parameter,
                    ch,
                } => {
                    let next_iter_state = |params: S| {
                        if let Some((parameter, ch)) = next(parameter, ch) {
                            NoAudioIterState::IteratingParams {
                                params,
                                parameter,
                                ch,
                            }
                        } else {
                            NoAudioIterState::Done
                        }
                    };
                    if let Some(mut channel_state) = self.state.channels[(ch - 1) as usize] {
                        let hash = self.state.hashes[(parameter, ch)];
                        if let Some(value) = params.numeric_by_hash(hash) {
                            if approx_eq(value, channel_state[parameter], 1e-6) {
                                (Some(None), next_iter_state(params))
                            } else {
                                channel_state[parameter] = value;
                                (
                                    Some(Some(events::Data::NoteExpression {
                                        data: note_expression_data(parameter, ch, value),
                                    })),
                                    next_iter_state(params),
                                )
                            }
                        } else {
                            (Some(None), next_iter_state(params))
                        }
                    } else {
                        (Some(None), next_iter_state(params))
                    }
                }
                NoAudioIterState::Done => (None, NoAudioIterState::Done),
            };
        self.iter_state = next_iter_state;
        ret
    }
}

pub fn add_mpe_quirk_events_no_audio(
    events: impl Iterator<Item = events::Data> + Clone,
    quirks_state: State,
    buffer_states: impl States + Clone,
) -> impl Iterator<Item = events::Data> + Clone {
    NoAudioIter::new(events.into_iter(), quirks_state, buffer_states).flatten()
}

pub fn update_mpe_quirk_events_no_audio(
    events: impl IntoIterator<Item = events::Data>,
    quirks_state: &mut State,
    buffer_states: impl States,
) {
    // TODO
}

#[derive(Debug, Clone, PartialEq)]
struct Hashes {
    pitch: [IdHash; 15],
    aftertouch: [IdHash; 15],
    timbre: [IdHash; 15],
}

impl std::ops::Index<(Parameter, i16)> for Hashes {
    type Output = IdHash;

    fn index(&self, (parameter, channel): (Parameter, i16)) -> &Self::Output {
        let idx = (channel - 1) as usize;
        match parameter {
            Parameter::Pitch => &self.pitch[idx],
            Parameter::Aftertouch => &self.aftertouch[idx],
            Parameter::Timbre => &self.timbre[idx],
        }
    }
}

impl Default for Hashes {
    fn default() -> Self {
        let mut pitch = [hash_id("dummy"); 15];
        let mut aftertouch = pitch;
        let mut timbre = pitch;

        for i in 0i16..15i16 {
            pitch[i as usize] = hash_id(&pitch_param_id(i + 1));
            aftertouch[i as usize] = hash_id(&aftertouch_param_id(i + 1));
            timbre[i as usize] = hash_id(&timbre_param_id(i + 1));
        }

        Hashes {
            pitch,
            aftertouch,
            timbre,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct ChannelState {
    pitch_bend: f32,
    aftertouch: f32,
    timbre: f32,
}

impl std::ops::Index<Parameter> for ChannelState {
    type Output = f32;

    fn index(&self, parameter: Parameter) -> &Self::Output {
        match parameter {
            Parameter::Pitch => &self.pitch_bend,
            Parameter::Aftertouch => &self.aftertouch,
            Parameter::Timbre => &self.timbre,
        }
    }
}

impl std::ops::IndexMut<Parameter> for ChannelState {
    fn index_mut(&mut self, parameter: Parameter) -> &mut Self::Output {
        match parameter {
            Parameter::Pitch => &mut self.pitch_bend,
            Parameter::Aftertouch => &mut self.aftertouch,
            Parameter::Timbre => &mut self.timbre,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    channels: [Option<ChannelState>; 15],

    hashes: Hashes,
}
