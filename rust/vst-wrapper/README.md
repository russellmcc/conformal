This crate contains code to wrap a [`conformal_component::Component`] in a [VST3](https://steinbergmedia.github.io/vst3_dev_portal/pages/index.html)-compatible plug-in.

The main entry point is the [`wrap_factory`] macro, which should be invoked exactly once for each plug-in binary.
