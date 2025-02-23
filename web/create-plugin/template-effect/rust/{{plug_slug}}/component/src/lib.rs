use conformal_component::audio::{Buffer, BufferMut, channels, channels_mut};
use conformal_component::effect::Effect as EffectTrait;
use conformal_component::parameters::{self, BufferStates, Flags, InfoRef, TypeSpecificInfoRef};
use conformal_component::pzip;
use conformal_component::{Component as ComponentTrait, ProcessingEnvironment, Processor};

const PARAMETERS: [InfoRef<'static, &'static str>; 2] = [
    InfoRef {
        title: "Bypass",
        short_title: "Bypass",
        unique_id: "bypass",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Switch { default: false },
    },
    InfoRef {
        title: "Gain",
        short_title: "Gain",
        unique_id: "gain",
        flags: Flags { automatable: true },
        type_specific: TypeSpecificInfoRef::Numeric {
            default: 100.,
            valid_range: 0f32..=100.,
            units: Some("%"),
        },
    },
];

#[derive(Clone, Debug, Default)]
pub struct Component {}

#[derive(Clone, Debug, Default)]
pub struct Effect {}

impl Processor for Effect {
    fn set_processing(&mut self, _processing: bool) {}
}

impl EffectTrait for Effect {
    fn handle_parameters<P: parameters::States>(&mut self, _: P) {}
    fn process<P: BufferStates, I: Buffer, O: BufferMut>(
        &mut self,
        parameters: P,
        input: &I,
        output: &mut O,
    ) {
        for (input_channel, output_channel) in channels(input).zip(channels_mut(output)) {
            for ((input_sample, output_sample), (gain, bypass)) in input_channel
                .iter()
                .zip(output_channel.iter_mut())
                .zip(pzip!(parameters[numeric "gain", switch "bypass"]))
            {
                *output_sample = *input_sample * (if bypass { 1.0 } else { gain / 100.0 });
            }
        }
    }
}

impl ComponentTrait for Component {
    type Processor = Effect;

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&PARAMETERS)
    }

    fn create_processor(&self, _env: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }
}
