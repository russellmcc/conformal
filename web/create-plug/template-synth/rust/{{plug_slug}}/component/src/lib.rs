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

use conformal_component::audio::BufferMut;
use conformal_component::events::{self, Event, Events};
use conformal_component::parameters::{self, BufferStates, Flags, InfoRef, TypeSpecificInfoRef};
use conformal_component::synth::Synth as SynthTrait;
use conformal_component::{Component as ComponentTrait, ProcessingEnvironment, Processor};

const PARAMETERS: [InfoRef<'static, &'static str>; 1] = [InfoRef {
    title: "Gain",
    short_title: "Gain",
    unique_id: "gain",
    flags: Flags { automatable: true },
    type_specific: TypeSpecificInfoRef::Numeric {
        default: 100.,
        valid_range: 0f32..=100.,
        units: Some("%"),
    },
}];

#[derive(Clone, Debug, Default)]
pub struct Component {}

#[derive(Clone, Debug, Default)]
pub struct Synth {}

impl Processor for Synth {
    fn set_processing(&mut self, _processing: bool) {}
}

impl SynthTrait for Synth {
    fn handle_events<E: Iterator<Item = events::Data> + Clone, P: parameters::States>(
        &mut self,
        _events: E,
        _parameters: P,
    ) {
    }

    fn process<E: Iterator<Item = Event> + Clone, P: BufferStates, O: BufferMut>(
        &mut self,
        _events: Events<E>,
        _parameters: P,
        _output: &mut O,
    ) {
    }
}

impl ComponentTrait for Component {
    type Processor = Synth;

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&PARAMETERS)
    }

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }
}
