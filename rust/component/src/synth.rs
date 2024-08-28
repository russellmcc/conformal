use crate::{
    events::{self, Event, Events},
    parameters::{self, BufferStates, Flags, InfoRef, TypeSpecificInfoRef},
    Processor,
};
use audio::BufferMut;

pub const PITCH_BEND_PARAMETER: &str = "pitch_bend";
pub const MOD_WHEEL_PARAMETER: &str = "mod_wheel";
pub const EXPRESSION_PARAMETER: &str = "expression_pedal";
pub const SUSTAIN_PARAMETER: &str = "sustain_pedal";
pub const AFTERTOUCH_PARAMETER: &str = "aftertouch";

pub const PITCH_BEND_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Pitch Bend",
    short_title: "Bend",
    unique_id: PITCH_BEND_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: -1.0..=1.0,
        units: "",
    },
};

pub const MOD_WHEEL_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Mod Wheel",
    short_title: "Mod",
    unique_id: MOD_WHEEL_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: "",
    },
};

pub const EXPRESSION_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Expression",
    short_title: "Expr",
    unique_id: EXPRESSION_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: "",
    },
};

pub const SUSTAIN_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Sustain Pedal",
    short_title: "Sus",
    unique_id: SUSTAIN_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Switch { default: false },
};

pub const AFTERTOUCH_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Aftertouch",
    short_title: "Aftertouch",
    unique_id: AFTERTOUCH_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: "",
    },
};

pub const CONTROLLER_PARAMETERS: [InfoRef<'static, &'static str>; 5] = [
    PITCH_BEND_INFO,
    MOD_WHEEL_INFO,
    EXPRESSION_INFO,
    SUSTAIN_INFO,
    AFTERTOUCH_INFO,
];

pub trait Synth: Processor {
    /// Handle parameter changes and events without processing any data.
    /// Must not allocate or block.
    ///
    /// Note that this will be called any time events come in without audio,
    /// or when parameters are changed without audio.
    ///
    /// Note that `parameters` will include parameters related to controller state
    /// (e.g. pitch bend, mod wheel, etc.) above.
    fn handle_events<E: IntoIterator<Item = events::Data> + Clone, P: parameters::States>(
        &mut self,
        events: E,
        parameters: P,
    );

    /// Process a buffer of events into a buffer of audio. Must not allocate or block.
    ///
    /// Note that `events` will be sorted by `sample_offset`
    ///
    /// Note that `parameters` will include parameters related to controller state
    /// (e.g. pitch bend, mod wheel, etc.) above.
    fn process<E: IntoIterator<Item = Event> + Clone, P: BufferStates, O: BufferMut>(
        &mut self,
        events: Events<E>,
        parameters: P,
        output: &mut O,
    );
}
