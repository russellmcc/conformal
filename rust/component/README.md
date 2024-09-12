This crate defines abstractions for audio processing components.

Users of this crate will generally implement a [Component] and then use another crate (such as `conformal_vst_wrapper`) to wrap the component in a standard audio Plug-in format.
