This crate defines abstractions for audio processing components.

Users of this crate will generally implement a [`Component`] that can create either an [`effect::Effect`] or a [`synth::Synth`] and then use a Conformal wrapper crate (currently `conformal_vst_wrapper`) to wrap the component in a standard audio Plug-in format.

This crate contains:

 - Definitions for the traits [`Component`]s must implement
 - Definitions for traits that Conformal wrappers will implement to provide data for the [`Component`] to consume. (e.g., [`parameters::BufferStates`], [`audio::Buffer`])
 - Simple implementatations of traits normally implemented by Conformal wrappers, to make testing easier and to provide a simple way to use [`Component`]s outside of a Conformal wrapper. (e.g., [`audio::BufferData`], [`parameters::ConstantBufferStates`])
 - Utilities to make some of these traits either to work with (e.g., [`pzip`]).
