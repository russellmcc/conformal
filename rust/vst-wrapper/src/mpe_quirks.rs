use std::iter::Peekable;

use conformal_component::{
    audio::approx_eq,
    events::{
        self, Events, NoteExpression, NoteExpressionData, NoteID,
        to_vst_note_channel_for_mpe_quirks,
    },
    parameters::{self, BufferStates, Flags, IdHash, States, TypeSpecificInfo, hash_id},
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
                    valid_range: -48.0..=48.0,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parameter {
    Pitch,
    Aftertouch,
    Timbre,
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
enum EventData {
    Event(events::Data),
    ParamChange {
        parameter: Parameter,
        channel: i16,
        value: f32,
    },
}

#[derive(Debug, Clone)]
struct Event {
    sample_offset: usize,
    data: EventData,
}

#[derive(Clone)]
struct InterleavedEventIter<A: Iterator<Item: Clone> + Clone, B: Iterator<Item: Clone> + Clone> {
    a: Peekable<A>,
    b: Peekable<B>,
}

impl<A: Iterator<Item = Event> + Clone, B: Iterator<Item = Event> + Clone> Iterator
    for InterleavedEventIter<A, B>
{
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.a.peek(), self.b.peek()) {
            (Some(a), Some(b)) => {
                if a.sample_offset <= b.sample_offset {
                    self.a.next()
                } else {
                    self.b.next()
                }
            }
            (Some(_), None) => self.a.next(),
            (None, Some(_)) => self.b.next(),
            (None, None) => None,
        }
    }
}

fn interleave_events<
    'a,
    A: Iterator<Item = Event> + Clone + 'a,
    B: Iterator<Item = Event> + Clone + 'a,
>(
    a: A,
    b: B,
) -> impl Iterator<Item = Event> + Clone + use<'a, A, B> {
    InterleavedEventIter {
        a: a.peekable(),
        b: b.peekable(),
    }
}

#[derive(Clone)]
struct ChannelInterleavedEventIter<I: Iterator<Item: Clone> + Clone> {
    channels: [Peekable<I>; 15],
}

impl<I: Iterator<Item = Event> + Clone> Iterator for ChannelInterleavedEventIter<I> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        (0..15)
            .map(|c_idx| self.channels[c_idx].peek().map(|e| e.sample_offset))
            .enumerate()
            .filter_map(|(i, sample_offset)| sample_offset.map(|sample_offset| (i, sample_offset)))
            .min_by_key(|(_, sample_offset)| *sample_offset)
            .and_then(|(i, _)| self.channels[i].next())
    }
}

fn interleave_events_for_channel(
    channels: [impl Iterator<Item = Event> + Clone; 15],
) -> impl Iterator<Item = Event> + Clone {
    ChannelInterleavedEventIter {
        channels: channels.map(std::iter::Iterator::peekable),
    }
}

fn update_state_for_event(ev: &events::Data, quirks_state: &mut State) {
    match ev {
        events::Data::NoteOn { data } => {
            if let c @ 1..=16 = to_vst_note_channel_for_mpe_quirks(data.id) {
                quirks_state.channels[(c - 1) as usize] = Default::default();
            }
        }
        events::Data::NoteOff { .. } | events::Data::NoteExpression { .. } => {}
    }
}

fn update_state_for_param_and_get_event(
    param: Parameter,
    channel: i16,
    value: f32,
    quirks_state: &mut State,
) -> Option<events::Data> {
    let mut channel_state = quirks_state.channels[(channel - 1) as usize];
    if approx_eq(channel_state[param], value, 1e-6) {
        return None;
    }
    channel_state[param] = value;
    Some(events::Data::NoteExpression {
        data: note_expression_data(param, channel, value),
    })
}

fn update_state_and_get_event(ev: EventData, quirks_state: &mut State) -> Option<events::Data> {
    match ev {
        EventData::Event(ev) => {
            update_state_for_event(&ev, quirks_state);
            Some(ev)
        }
        EventData::ParamChange {
            parameter,
            channel,
            value,
        } => update_state_for_param_and_get_event(parameter, channel, value, quirks_state),
    }
}

fn with_mpe_events(
    events: impl Iterator<Item = Event> + Clone,
    mut quirks_state: State,
) -> impl Iterator<Item = events::Event> + Clone {
    events.filter_map(move |e| {
        update_state_and_get_event(e.data, &mut quirks_state).map(|data| events::Event {
            sample_offset: e.sample_offset,
            data,
        })
    })
}

fn update_for_events(events: impl Iterator<Item = Event>, quirks_state: &mut State) {
    for e in events {
        update_state_and_get_event(e.data, quirks_state);
    }
}

fn param_event_iters_no_audio<S: States>(
    parameter: Parameter,
    hashes: &[IdHash; 15],
    buffer_states: &S,
) -> impl Iterator<Item = Event> + Clone + use<S> {
    let mut i = 0;
    interleave_events_for_channel(hashes.map(move |hash| {
        let c = i;
        i += 1;
        buffer_states
            .numeric_by_hash(hash)
            .into_iter()
            .map(move |v| Event {
                sample_offset: 0,
                data: EventData::ParamChange {
                    parameter,
                    channel: c + 1,
                    value: v,
                },
            })
    }))
}

fn param_event_iters<'a, S: BufferStates + Clone>(
    parameter: Parameter,
    hashes: &[IdHash; 15],
    buffer_states: &'a S,
) -> impl Iterator<Item = Event> + Clone + use<'a, S> {
    let mut i = 0;
    interleave_events_for_channel(hashes.map(move |hash| {
        let c = i;
        i += 1;
        buffer_states
            .numeric_by_hash(hash)
            .into_iter()
            .flat_map(move |numeric| match numeric {
                parameters::NumericBufferState::Constant(v) => {
                    itertools::Either::Left(std::iter::once(Event {
                        sample_offset: 0,
                        data: EventData::ParamChange {
                            parameter,
                            channel: c + 1,
                            value: v,
                        },
                    }))
                }
                parameters::NumericBufferState::PiecewiseLinear(piecewise_linear_curve) => {
                    itertools::Either::Right(piecewise_linear_curve.into_iter().map(move |point| {
                        Event {
                            sample_offset: point.sample_offset,
                            data: EventData::ParamChange {
                                parameter,
                                channel: c + 1,
                                value: point.value,
                            },
                        }
                    }))
                }
            })
    }))
}

fn all_param_event_iters_no_audio<
    'a,
    I: Iterator<Item = events::Data> + Clone + 'a,
    S: States + Clone,
>(
    events: I,
    hashes: &Hashes,
    buffer_states: &'a S,
) -> impl Iterator<Item = Event> + Clone + use<'a, I, S> {
    let pitch = param_event_iters_no_audio(Parameter::Pitch, &hashes.pitch, buffer_states);
    let aftertouch =
        param_event_iters_no_audio(Parameter::Aftertouch, &hashes.aftertouch, buffer_states);
    let timbre = param_event_iters_no_audio(Parameter::Timbre, &hashes.timbre, buffer_states);
    interleave_events(
        events.map(|e| Event {
            sample_offset: 0,
            data: EventData::Event(e),
        }),
        interleave_events(pitch, interleave_events(aftertouch, timbre)),
    )
}

fn all_param_event_iters_audio<
    'a,
    I: Iterator<Item = events::Event> + Clone + 'a,
    S: BufferStates + Clone,
>(
    events: I,
    hashes: &Hashes,
    buffer_states: &'a S,
) -> impl Iterator<Item = Event> + Clone + use<'a, I, S> {
    let pitch = param_event_iters(Parameter::Pitch, &hashes.pitch, buffer_states);
    let aftertouch = param_event_iters(Parameter::Aftertouch, &hashes.aftertouch, buffer_states);
    let timbre = param_event_iters(Parameter::Timbre, &hashes.timbre, buffer_states);
    interleave_events(
        events.map(|e| Event {
            sample_offset: e.sample_offset,
            data: EventData::Event(e.data),
        }),
        interleave_events(pitch, interleave_events(aftertouch, timbre)),
    )
}

pub fn add_mpe_quirk_events_no_audio<
    'a,
    I: Iterator<Item = events::Data> + Clone + 'a,
    S: States + Clone,
>(
    events: I,
    quirks_state: State,
    buffer_states: &'a S,
) -> impl Iterator<Item = events::Data> + Clone + use<'a, I, S> {
    with_mpe_events(
        all_param_event_iters_no_audio(events, &quirks_state.hashes, buffer_states),
        quirks_state,
    )
    .map(|e| e.data)
}

pub fn update_mpe_quirk_events_no_audio(
    events: impl Iterator<Item = events::Data> + Clone,
    quirks_state: &mut State,
    buffer_states: &(impl States + Clone),
) {
    update_for_events(
        all_param_event_iters_no_audio(events, &quirks_state.hashes, buffer_states),
        quirks_state,
    );
}

pub fn add_mpe_quirk_events_buffer<
    'a,
    E: Iterator<Item = events::Event> + Clone + 'a,
    S: BufferStates + Clone,
>(
    events: E,
    quirks_state: State,
    buffer_states: &'a S,
    buffer_size: usize,
) -> Events<impl Iterator<Item = events::Event> + Clone + use<'a, E, S>> {
    events::Events::new(
        with_mpe_events(
            all_param_event_iters_audio(events.into_iter(), &quirks_state.hashes, buffer_states),
            quirks_state,
        ),
        buffer_size,
    )
    .unwrap()
}

pub fn update_mpe_quirk_events_buffer(
    events: impl Iterator<Item = events::Event> + Clone,
    quirks_state: &mut State,
    buffer_states: &(impl BufferStates + Clone),
) {
    update_for_events(
        all_param_event_iters_audio(events.into_iter(), &quirks_state.hashes, buffer_states),
        quirks_state,
    );
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
    channels: [ChannelState; 15],

    hashes: Hashes,
}
