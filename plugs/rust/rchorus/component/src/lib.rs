#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]

use conformal_component::parameters::{self, InfoRef};
use conformal_component::parameters::{Flags, TypeSpecificInfoRef};
use conformal_component::{Component as ComponentT, ProcessingEnvironment};

const PARAMETERS: [InfoRef<'static, &'static str>; 4] = [
    InfoRef {
        title: "Rate",
        short_title: "Rate",
        unique_id: "rate",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: 0.35,
            valid_range: 0.08..=10.1,
            units: "hz",
        },
    },
    InfoRef {
        title: "Depth",
        short_title: "Depth",
        unique_id: "depth",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: 100.,
            valid_range: 0f32..=100.,
            units: "%",
        },
    },
    InfoRef {
        title: "Mix",
        short_title: "Mix",
        unique_id: "mix",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: 100.,
            valid_range: 0f32..=100.,
            units: "%",
        },
    },
    InfoRef {
        title: "Bypass",
        short_title: "Bypass",
        unique_id: "bypass",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch { default: false },
    },
];

mod anti_aliasing_filter;
mod compander;
mod effect;
mod kernel;
mod lfo;
mod look_behind;
mod modulated_delay;
mod nonlinearity;
mod polyphase_kernel;

#[derive(Clone, Debug, Default)]
pub struct Component {}

impl ComponentT for Component {
    type Processor = effect::Effect;

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&PARAMETERS)
    }

    fn create_processor(&self, env: &ProcessingEnvironment) -> Self::Processor {
        effect::Effect::new(env)
    }
}
