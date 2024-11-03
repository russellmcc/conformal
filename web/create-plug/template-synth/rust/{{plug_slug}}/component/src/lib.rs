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
use conformal_component::events::{self, Event, Events, NoteData};
use conformal_component::parameters::{self, BufferStates, Flags, InfoRef, TypeSpecificInfoRef};
use conformal_component::synth::Synth as SynthTrait;
use conformal_component::{pzip, Component as ComponentTrait, ProcessingEnvironment, Processor};
use conformal_poly::{self, EventData, Poly, Voice as VoiceTrait};
use itertools::izip;

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

/// Converts a MIDI pitch to a phase increment
fn increment(midi_pitch: f32, sampling_rate: f32) -> f32 {
    440f32 * 2.0f32.powf((midi_pitch - 69f32) / 12f32) / sampling_rate
}

#[derive(Clone, Debug, Default)]
struct Voice {
    pitch: Option<f32>,
    phase: f32,
    sampling_rate: f32,
}

#[derive(Default, Debug, Clone)]
pub struct SharedData {}

#[derive(Debug)]
pub struct Synth {
    poly: Poly<Voice>,
}

const PITCH_BEND_WIDTH: f32 = 2.;

impl Processor for Synth {
    fn set_processing(&mut self, _processing: bool) {}
}

impl SynthTrait for Synth {
    fn handle_events<E: Iterator<Item = events::Data> + Clone, P: parameters::States>(
        &mut self,
        events: E,
        _parameters: P,
    ) {
        self.poly.handle_events(events);
    }

    fn process<E: Iterator<Item = Event> + Clone, P: BufferStates, O: BufferMut>(
        &mut self,
        events: Events<E>,
        parameters: P,
        output: &mut O,
    ) {
        self.poly
            .render_audio(events.into_iter(), &parameters, &Default::default(), output);
    }
}

impl VoiceTrait for Voice {
    type SharedData<'a> = SharedData;

    fn new(_max_samples_per_process_call: usize, sampling_rate: f32) -> Self {
        Self {
            pitch: None,
            phase: 0.,
            sampling_rate,
        }
    }

    fn handle_event(&mut self, event: &conformal_poly::EventData) {
        match event {
            EventData::NoteOn {
                data: NoteData { pitch, .. },
            } => {
                self.pitch = Some(f32::from(*pitch));
            }
            EventData::NoteOff { .. } => {
                self.pitch = None;
                self.phase = 0.;
            }
        }
    }

    fn render_audio(
        &mut self,
        events: impl IntoIterator<Item = conformal_poly::Event>,
        params: &impl parameters::BufferStates,
        note_expressions: conformal_poly::NoteExpressionCurve<
            impl Iterator<Item = conformal_poly::NoteExpressionPoint> + Clone,
        >,
        _data: Self::SharedData<'_>,
        output: &mut [f32],
    ) {
        let mut events = events.into_iter().peekable();
        for ((index, sample), (gain, global_pitch_bend), expression) in izip!(
            output.iter_mut().enumerate(),
            pzip!(params[numeric "gain", numeric "pitch_bend"]),
            note_expressions.iter_by_sample(),
        ) {
            while let Some(conformal_poly::Event {
                sample_offset,
                data,
            }) = events.peek()
            {
                if sample_offset > &index {
                    break;
                }
                self.handle_event(data);
                events.next();
            }
            if let Some(pitch) = self.pitch {
                let total_pitch_bend = global_pitch_bend * PITCH_BEND_WIDTH + expression.pitch_bend;
                let adjusted_pitch = pitch + total_pitch_bend;
                let increment = increment(adjusted_pitch, self.sampling_rate);
                *sample = (self.phase * std::f32::consts::TAU).sin() * gain / 100.;
                // Update the phase and wrap it to [0, 1)
                self.phase += increment;
                self.phase -= self.phase.floor();
            }
        }
    }

    fn quiescent(&self) -> bool {
        self.pitch.is_none()
    }

    fn reset(&mut self) {
        self.pitch = None;
        self.phase = 0.;
    }
}

impl ComponentTrait for Component {
    type Processor = Synth;

    fn parameter_infos(&self) -> Vec<parameters::Info> {
        parameters::to_infos(&PARAMETERS)
    }

    fn create_processor(&self, env: &ProcessingEnvironment) -> Self::Processor {
        Synth {
            poly: Poly::new(env, 8),
        }
    }
}
