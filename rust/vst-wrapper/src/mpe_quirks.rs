use conformal_component::parameters::{self, Flags, TypeSpecificInfo};

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
pub enum SupportMpeQuirks {
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
pub fn should_support(_: &HostInfo) -> SupportMpeQuirks {
    // Currently support "mpe quirks" in all hosts. If this implementation of note expression
    // becomes less common, we might want to use only a list of hosts known to use this quirky
    // implementation. There isn't much of a downside to supporting the quirks, since we
    // don't support multi-channel synths anyways. When and if we do, we'll have to reconsider this.
    SupportMpeQuirks::SupportQuirks
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
