//! "MPE Quirks" is a _really_ unfortunate vst3 note expression implementation that is used
//! in several hosts, including Ableton as of 12.0.25. Instead of using the vst3 note expression
//! system, it insteads uses actual MPE messages that are expected to be midi-mapped to parameters
//! in the plugin.
//!
//! We begrudgingly support this, since we want our plug-ins to work with Ableton, even though
//! it means adding _several_ completely unnecessary dummy parameters, and a bunch of extra code.

use conformal_component::{
    parameters::{self, Flags, IdHash, TypeSpecificInfo, hash_id},
    synth::NumericPerNoteExpression,
};

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
                    valid_range: -120.0..=120.0,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Hashes {
    pitch: [IdHash; 15],
    aftertouch: [IdHash; 15],
    timbre: [IdHash; 15],
}

impl std::ops::Index<(NumericPerNoteExpression, i16)> for Hashes {
    type Output = IdHash;

    fn index(&self, (parameter, channel): (NumericPerNoteExpression, i16)) -> &Self::Output {
        let idx = (channel - 1) as usize;
        match parameter {
            NumericPerNoteExpression::PitchBend => &self.pitch[idx],
            NumericPerNoteExpression::Aftertouch => &self.aftertouch[idx],
            NumericPerNoteExpression::Timbre => &self.timbre[idx],
        }
    }
}

impl Default for Hashes {
    fn default() -> Self {
        Hashes {
            pitch: std::array::from_fn(|i| hash_id(&pitch_param_id(i16::try_from(i).unwrap() + 1))),
            aftertouch: std::array::from_fn(|i| {
                hash_id(&aftertouch_param_id(i16::try_from(i).unwrap() + 1))
            }),
            timbre: std::array::from_fn(|i| {
                hash_id(&timbre_param_id(i16::try_from(i).unwrap() + 1))
            }),
        }
    }
}
