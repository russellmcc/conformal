// Generally we _expect_ truncation here, so allow it.
#![allow(clippy::cast_possible_truncation)]

use conformal_component::parameters::UNIQUE_ID_INTERNAL_PREFIX;
use conformal_component::parameters::{Flags, InfoRef, TypeSpecificInfoRef};
use conformal_component::synth::{NumericGlobalExpression, SwitchGlobalExpression};
use const_format::formatcp;

pub(crate) fn convert_numeric(value: f64, valid_range: &std::ops::RangeInclusive<f32>) -> f32 {
    (value as f32).clamp(0.0, 1.0) * (valid_range.end() - valid_range.start()) + valid_range.start()
}

pub(crate) fn normalize_numeric(value: f32, valid_range: &std::ops::RangeInclusive<f32>) -> f64 {
    ((value.clamp(*valid_range.start(), *valid_range.end()) - valid_range.start())
        / (valid_range.end() - valid_range.start()))
    .into()
}

// Generally we _expect_ truncation here, so allow it.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn convert_enum(value: f64, count: u32) -> u32 {
    ((value.clamp(0.0, 1.0) * (f64::from(count))).floor() as u32).min(count - 1)
}

pub(crate) fn normalize_enum(value: u32, count: u32) -> f64 {
    (f64::from(value.clamp(0, count - 1))) / (f64::from(count - 1))
}

pub(crate) fn convert_switch(value: f64) -> bool {
    value > 0.5
}

pub(crate) fn normalize_switch(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

pub(crate) fn should_include_parameter_in_snapshot(id: &str) -> bool {
    !id.starts_with(UNIQUE_ID_INTERNAL_PREFIX)
}

/// In VST3 format, the way to get _global_ expression controllers is to
/// define non-automatable VST3 parameters and then midi-map them to the
/// expression controller. At that point, we'll receive controller changes
/// as param changes to the mapped parameters.
///
/// We handle this by defining internal VST3 parameters that map to these
/// controllers. Note that these parameters do live in our normal parameter
/// store, but are filtered out by [`should_include_parameter_in_snapshot`].
pub(crate) const PITCH_BEND_PARAMETER: &str = formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}pitch_bend");
pub(crate) const MOD_WHEEL_PARAMETER: &str = formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}mod_wheel");
pub(crate) const EXPRESSION_PEDAL_PARAMETER: &str =
    formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}expression_pedal");
pub(crate) const AFTERTOUCH_PARAMETER: &str = formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}aftertouch");
pub(crate) const TIMBRE_PARAMETER: &str = formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}timbre");
pub(crate) const SUSTAIN_PARAMETER: &str = formatcp!("{UNIQUE_ID_INTERNAL_PREFIX}sustain");

pub(crate) fn parameter_id_for_numeric_global_expression(
    expression: NumericGlobalExpression,
) -> &'static str {
    match expression {
        NumericGlobalExpression::PitchBend => PITCH_BEND_PARAMETER,
        NumericGlobalExpression::ModWheel => MOD_WHEEL_PARAMETER,
        NumericGlobalExpression::ExpressionPedal => EXPRESSION_PEDAL_PARAMETER,
        NumericGlobalExpression::Aftertouch => AFTERTOUCH_PARAMETER,
        NumericGlobalExpression::Timbre => TIMBRE_PARAMETER,
    }
}

pub(crate) fn parameter_id_for_switch_global_expression(
    expression: SwitchGlobalExpression,
) -> &'static str {
    match expression {
        SwitchGlobalExpression::SustainPedal => SUSTAIN_PARAMETER,
    }
}

const PITCH_BEND_INFO: InfoRef<'static, &'static str> = InfoRef {
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

const MOD_WHEEL_INFO: InfoRef<'static, &'static str> = InfoRef {
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

const EXPRESSION_PEDAL_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Expression Pedal",
    short_title: "Expression",
    unique_id: EXPRESSION_PEDAL_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: None,
    },
};

const AFTERTOUCH_INFO: InfoRef<'static, &'static str> = InfoRef {
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

const TIMBRE_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Timbre",
    short_title: "Timbre",
    unique_id: TIMBRE_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 0.0,
        valid_range: 0.0..=1.0,
        units: None,
    },
};

const SUSTAIN_INFO: InfoRef<'static, &'static str> = InfoRef {
    title: "Sustain",
    short_title: "Sustain",
    unique_id: SUSTAIN_PARAMETER,
    flags: Flags { automatable: false },
    type_specific: TypeSpecificInfoRef::Switch { default: false },
};

pub(crate) const CONTROLLER_PARAMETERS: [InfoRef<'static, &'static str>; 6] = [
    PITCH_BEND_INFO,
    MOD_WHEEL_INFO,
    EXPRESSION_PEDAL_INFO,
    AFTERTOUCH_INFO,
    TIMBRE_INFO,
    SUSTAIN_INFO,
];
