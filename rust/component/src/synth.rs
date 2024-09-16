//! Abstractions for processors that generate audio.

use crate::{
    audio::BufferMut,
    events::{self, Event, Events},
    parameters::{self, BufferStates, Flags, InfoRef, TypeSpecificInfoRef},
    Processor,
};

/// The parameter ID of the pitch bend parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const PITCH_BEND_PARAMETER: &str = "pitch_bend";

/// The parameter ID of the mod wheel parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const MOD_WHEEL_PARAMETER: &str = "mod_wheel";

/// The parameter ID of the expression pedal parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const EXPRESSION_PARAMETER: &str = "expression_pedal";

/// The parameter ID of the sustain pedal parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const SUSTAIN_PARAMETER: &str = "sustain_pedal";

/// The parameter ID of the aftertouch parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const AFTERTOUCH_PARAMETER: &str = "aftertouch";

/// Parameter info for the pitch bend parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const PITCH_BEND_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Pitch Bend",
    short_title: "Bend",
    unique_id: PITCH_BEND_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: -1.0..=1.0,
        units: None,
    },
};

/// Parameter info for the mod wheel parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const MOD_WHEEL_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Mod Wheel",
    short_title: "Mod",
    unique_id: MOD_WHEEL_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: None,
    },
};

/// Parameter info for the expression pedal parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const EXPRESSION_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Expression",
    short_title: "Expr",
    unique_id: EXPRESSION_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: None,
    },
};

/// Parameter info for the sustain pedal parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const SUSTAIN_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Sustain Pedal",
    short_title: "Sus",
    unique_id: SUSTAIN_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Switch { default: false },
};

/// Parameter info for the aftertouch parameter. See [`CONTROLLER_PARAMETERS`] for more.
pub const AFTERTOUCH_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Aftertouch",
    short_title: "Aftertouch",
    unique_id: AFTERTOUCH_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: None,
    },
};

/// This represents a set of "controller parameters" that are common to
/// all synths.
///
/// These parameters will not appear in audio software as
/// automatable parameters, but they will be filled in with the current
/// value of the corresponding controllers.
///
/// Note that synths will receive these regardless of what they returned
/// from `crate::Component::parameter_infos`.
pub const CONTROLLER_PARAMETERS: [InfoRef<'static, &'static str>; 5] = [
    PITCH_BEND_INFO,
    MOD_WHEEL_INFO,
    EXPRESSION_INFO,
    SUSTAIN_INFO,
    AFTERTOUCH_INFO,
];

/// A trait for synthesizers
///
/// A synthesizer is a processor that creates audio from a series of _events_,
/// such as Note On, or Note Off.
pub trait Synth: Processor {
    /// Handle parameter changes and events without processing any data.
    /// Must not allocate or block.
    ///
    /// Note that `parameters` will include [`CONTROLLER_PARAMETERS`] related to controller state
    /// (e.g. pitch bend, mod wheel, etc.) above, in addition to all the parameters
    /// returned by `crate::Component::parameter_infos`.
    fn handle_events<E: IntoIterator<Item = events::Data> + Clone, P: parameters::States>(
        &mut self,
        events: E,
        parameters: P,
    );

    /// Process a buffer of events into a buffer of audio. Must not allocate or block.
    ///
    /// Note that `events` will be sorted by `sample_offset`
    ///
    /// `output` will be received in an undetermined state and must
    /// be filled with audio by the processor during this call.
    ///
    /// Note that `parameters` will include [`CONTROLLER_PARAMETERS`] related to controller state
    /// (e.g. pitch bend, mod wheel, etc.) above, in addition to all the parameters
    /// returned by `crate::Component::parameter_infos`.
    ///
    /// In order to consume the parameters, you can use the [`crate::pzip`] macro
    /// to convert the parameters into an iterator of tuples that represent
    /// the state of the parameters at each sample.
    ///
    /// The sample rate of the audio was provided in `environment.sampling_rate`
    /// in the call to `crate::Component::create_processor`.
    ///
    /// Note that it's guaranteed that `output` will be no longer than
    /// `environment.max_samples_per_process_call` provided in the call to
    /// `crate::Component::create_processor`.
    fn process<E: IntoIterator<Item = Event> + Clone, P: BufferStates, O: BufferMut>(
        &mut self,
        events: Events<E>,
        parameters: P,
        output: &mut O,
    );
}
