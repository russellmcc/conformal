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

use self::state::State;
use component::{
    audio::{channels_mut, BufferMut},
    events::{Data, Event},
    parameters, ProcessingEnvironment,
};

use util::slice_ops::{add_in_place, mul_constant_in_place};

// Optimization opportunity - allow `Voice` to indicate that not all output
// was filled. This will let us skip rendering until a voice is playing
// and also skip mixing silence.

pub trait Voice {
    type SharedData<'a>: Clone;
    fn new(max_samples_per_process_call: usize, sampling_rate: f32) -> Self;
    fn handle_event(&mut self, event: &Data);
    fn render_audio(
        &mut self,
        events: impl IntoIterator<Item = Event>,
        params: &impl parameters::BufferStates,
        data: Self::SharedData<'_>,
        output: &mut [f32],
    );
    #[must_use]
    fn quiescent(&self) -> bool;
    fn skip_samples(&mut self, _num_samples: usize) {}
    fn reset(&mut self);
}

pub struct Poly<V> {
    voices: Vec<V>,
    state: State,
    voice_scratch_buffer: Vec<f32>,
}

impl<V: std::fmt::Debug> std::fmt::Debug for Poly<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Poly")
            .field("voices", &self.voices)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

mod state;

impl<V: Voice> Poly<V> {
    #[must_use]
    pub fn new(environment: &ProcessingEnvironment, max_voices: usize) -> Self {
        let voices = std::iter::repeat_with(|| {
            V::new(
                environment.max_samples_per_process_call,
                environment.sampling_rate,
            )
        })
        .take(max_voices)
        .collect();
        let state = State::new(max_voices);

        Self {
            voices,
            state,
            voice_scratch_buffer: vec![0f32; environment.max_samples_per_process_call],
        }
    }

    pub fn handle_events(&mut self, events: impl IntoIterator<Item = Data> + Clone) {
        for (v, ev) in self
            .state
            .clone()
            .dispatch_events(events.clone().into_iter().map(|data| Event {
                sample_offset: 0,
                data,
            }))
        {
            self.voices[v].handle_event(&ev.data);
        }

        self.state.update(events.into_iter().map(|data| Event {
            sample_offset: 0,
            data,
        }));
    }

    pub fn render_audio(
        &mut self,
        events: impl IntoIterator<Item = Event> + Clone,
        params: &impl parameters::BufferStates,
        shared_data: &V::SharedData<'_>,
        output: &mut impl BufferMut,
    ) {
        let buffer_size = output.num_frames();
        #[allow(clippy::cast_precision_loss)]
        let voice_scale = 1f32 / self.voices.len() as f32;
        let mut cleared = false;
        for (index, voice) in self.voices.iter_mut().enumerate() {
            let voice_events = || {
                self.state
                    .clone()
                    .dispatch_events(events.clone())
                    .into_iter()
                    .filter_map(|(i, event)| if i == index { Some(event) } else { None })
            };
            if voice_events().next().is_none() && voice.quiescent() {
                voice.skip_samples(buffer_size);
                continue;
            }
            voice.render_audio(
                voice_events(),
                params,
                shared_data.clone(),
                &mut self.voice_scratch_buffer[0..output.num_frames()],
            );
            mul_constant_in_place(voice_scale, &mut self.voice_scratch_buffer);
            if cleared {
                for channel_mut in channels_mut(output) {
                    add_in_place(&self.voice_scratch_buffer[0..buffer_size], channel_mut);
                }
            } else {
                for channel_mut in channels_mut(output) {
                    channel_mut.copy_from_slice(&self.voice_scratch_buffer[0..buffer_size]);
                }
                cleared = true;
            }
        }
        if !cleared {
            for channel_mut in channels_mut(output) {
                channel_mut.fill(0f32);
            }
        }
        self.state.update(events);
    }

    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.state.reset();
    }
}
