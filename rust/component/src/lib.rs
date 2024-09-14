#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
//    missing_docs,
    rustdoc::private_doc_tests,
    rustdoc::unescaped_backticks,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]
#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

pub mod audio;
pub mod effect;
pub mod events;
pub mod parameters;
pub mod synth;

#[doc(hidden)]
pub use itertools;

/// The mode that the processor will run in.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProcessingMode {
    /// The component is processing audio in realtime.
    Realtime,

    /// The component may not be running in realtime, but should use the same quality settings as `Realtime`.
    Prefetch,

    /// The component is processing audio in offline mode.
    Offline,
}

/// Information about the processing environment that the processor will run in.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingEnvironment {
    /// The sample rate of the audio.
    pub sampling_rate: f32,

    /// The mazimum number of samples that will be passed to each call to `process`.
    ///
    /// Note that fewer samples may be passed to `process` than this.
    pub max_samples_per_process_call: usize,

    /// The channel layout of the audio
    pub channel_layout: audio::ChannelLayout,

    /// The processing mode that the processor will run in.
    pub processing_mode: ProcessingMode,
}

/// The main plug-in abstraction in Conformal.
///
/// [`Component`]s can be wrapped in various plug-in formats
/// for use in audio software.
///
/// [`Component`]s contain information about the parameters of a processor
/// as well as the ability to create a processor.
///
/// Note that this is not intended to be used as an _internal_ interface for audio processors,
/// but rather an _external_ one that can be easily wrapped in common plug-in formats. That is to
/// say, a plug-in should only have one `Component` that represents the whole plug-in - to
/// compose _parts_ of the plug-in you should use a different abstraction.
pub trait Component {
    /// The processor that this component creates.
    type Processor;

    /// Get information about the parameters of this component
    ///
    /// This must return the same value every time it is called.
    fn parameter_infos(&self) -> Vec<parameters::Info> {
        Default::default()
    }

    /// Create the processor that will actually process audio.
    ///
    /// Note any state needed to process audio should be allocated here.
    fn create_processor(&self, environment: &ProcessingEnvironment) -> Self::Processor;
}

/// A base trait for audio processors.
///
/// Most audio processors should additionally implement [`effect::Effect`] or [`synth::Synth`].
pub trait Processor {
    /// Enable or disable processing. Must not allocate or block.
    ///
    /// processing starts off.
    ///
    /// Note that after toggling this on -> off -> on, we must generate the
    /// _exact_ same output as the first time we were turned on - i.e.,
    /// this acts as a reset.
    ///
    /// Note that `process` will only ever be called _after_ `set_processing(true)`
    fn set_processing(&mut self, processing: bool);
}
