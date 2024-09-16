#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    missing_docs,
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

pub use conformal_ui::Size as UiSize;
use core::slice;

/// Contains information about the host.
///
/// You can use this to customize the comonent based on the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostInfo {
    /// The name of the host.
    pub name: String,
}

/// A class ID for a VST3 component.
///
/// This must be _globally_ unique for each class
pub type ClassID = [u8; 16];

/// A component factory that can create a component.
///
/// This can return a specialized component based on information
/// about the current host
#[allow(clippy::module_name_repetitions)]
pub trait ComponentFactory: Clone {
    /// The type of component that this factory creates
    type Component;

    /// Create a component
    fn create(&self, host: &HostInfo) -> Self::Component;
}

impl<C, F: Fn(&HostInfo) -> C + Clone> ComponentFactory for F {
    type Component = C;
    fn create(&self, host_info: &HostInfo) -> C {
        (self)(host_info)
    }
}

/// Information about a VST3 component
#[derive(Debug, Clone, Copy)]
pub struct ClassInfo<'a> {
    /// User-visibile name of the component.
    pub name: &'a str,

    /// Class ID for the processor component.  This is used by the host to identify the VST.
    pub cid: ClassID,

    /// Class ID for the so-called "edit controller" component.  This is arbitrary
    /// but must be unique.
    pub edit_controller_cid: ClassID,

    /// Initial size of the UI in logical pixels
    pub ui_initial_size: UiSize,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum ExtraParameters {
    None,
    SynthControlParameters,
}

#[doc(hidden)]
pub struct ParameterModel {
    pub parameter_infos: Box<dyn Fn(&HostInfo) -> Vec<conformal_component::parameters::Info>>,
    pub extra_parameters: ExtraParameters,
}

#[doc(hidden)]
pub trait ClassCategory {
    fn create_processor(&self, controller_cid: ClassID) -> vst3::ComPtr<IPluginBase>;

    fn info(&self) -> &ClassInfo<'static>;

    fn category_str(&self) -> &'static str;

    fn create_parameter_model(&self) -> ParameterModel;

    fn get_bypass_id(&self) -> Option<&'static str>;
}

/// Information about a synth component
pub struct SynthClass<CF> {
    /// The actual factory.
    pub factory: CF,

    /// Information about the component
    pub info: ClassInfo<'static>,
}

fn create_parameter_model_internal<CF: ComponentFactory + 'static>(
    factory: CF,
    extra_parameters: ExtraParameters,
) -> ParameterModel
where
    CF::Component: Component,
{
    ParameterModel {
        parameter_infos: Box::new(move |host_info| {
            let component = factory.create(host_info);
            component.parameter_infos()
        }),
        extra_parameters,
    }
}

impl<CF: ComponentFactory + 'static> ClassCategory for SynthClass<CF>
where
    CF::Component: Component<Processor: Synth> + 'static,
{
    fn create_processor(&self, controller_cid: ClassID) -> vst3::ComPtr<IPluginBase> {
        vst3::ComWrapper::new(processor::create_synth(
            self.factory.clone(),
            controller_cid,
        ))
        .to_com_ptr::<IPluginBase>()
        .unwrap()
    }

    fn create_parameter_model(&self) -> ParameterModel {
        create_parameter_model_internal(
            self.factory.clone(),
            ExtraParameters::SynthControlParameters,
        )
    }

    fn category_str(&self) -> &'static str {
        "Instrument|Synth"
    }

    fn info(&self) -> &ClassInfo<'static> {
        &self.info
    }

    fn get_bypass_id(&self) -> Option<&'static str> {
        None
    }
}

/// Information about an effect component
pub struct EffectClass<CF> {
    /// The actual factory.
    pub factory: CF,

    /// Information about the component
    pub info: ClassInfo<'static>,

    /// The VST3 category for this effect
    /// See [here](https://steinbergmedia.github.io/vst3_doc/vstinterfaces/group__plugType.html)
    /// for a list of possible categories.
    pub category: &'static str,

    /// All effects must have a bypass parameter. This is the unique ID for that parameter.
    pub bypass_id: &'static str,
}

impl<CF: ComponentFactory<Component: Component<Processor: Effect> + 'static> + 'static>
    ClassCategory for EffectClass<CF>
{
    fn create_processor(&self, controller_cid: ClassID) -> vst3::ComPtr<IPluginBase> {
        vst3::ComWrapper::new(processor::create_effect(
            self.factory.clone(),
            controller_cid,
        ))
        .to_com_ptr::<IPluginBase>()
        .unwrap()
    }

    fn category_str(&self) -> &'static str {
        self.category
    }

    fn info(&self) -> &ClassInfo<'static> {
        &self.info
    }

    fn create_parameter_model(&self) -> ParameterModel {
        create_parameter_model_internal(self.factory.clone(), ExtraParameters::None)
    }

    fn get_bypass_id(&self) -> Option<&'static str> {
        Some(self.bypass_id)
    }
}

/// General global infor about a vst plug-in
#[derive(Debug, Clone, Copy)]
pub struct Info<'a> {
    /// The "vendor" of the plug-in.
    ///
    /// Hosts often present plug-ins grouped by vendor.
    pub vendor: &'a str,

    /// The vendor's URL
    pub url: &'a str,

    /// The vendor's email
    pub email: &'a str,

    /// User-visibile version of components in this factory
    pub version: &'a str,
}

use conformal_component::effect::Effect;
use conformal_component::synth::Synth;
use conformal_component::Component;

use vst3::Steinberg::{IPluginBase, IPluginFactory2, IPluginFactory2Trait};
use vst3::{Class, Steinberg::IPluginFactory};

mod edit_controller;
mod factory;
mod host_info;
mod io;
mod parameters;
mod processor;
mod view;

#[cfg(test)]
mod dummy_host;

#[cfg(test)]
mod fake_ibstream;

#[doc(hidden)]
pub fn _wrap_factory(
    classes: &'static [&'static dyn ClassCategory],
    info: Info<'static>,
) -> impl Class<Interfaces = (IPluginFactory, IPluginFactory2)> + 'static + IPluginFactory2Trait {
    factory::Factory::new(classes, info)
}

fn to_utf16(s: &str, buffer: &mut [i16]) {
    for (i, c) in s.encode_utf16().chain([0]).enumerate() {
        buffer[i] = c as i16;
    }
}

fn from_utf16_ptr(buffer: *const i16, max_size: usize) -> Option<String> {
    let mut len = 0;
    unsafe {
        while *buffer.add(len) != 0 {
            if len >= max_size {
                return None;
            }
            len += 1;
        }
    }
    let utf16_slice = unsafe { slice::from_raw_parts(buffer.cast(), len) };
    String::from_utf16(utf16_slice).ok()
}

fn from_utf16_buffer(buffer: &[i16]) -> Option<String> {
    let mut len = 0;
    for c in buffer {
        if *c == 0 {
            break;
        }
        len += 1;
    }
    let utf16_slice = unsafe { slice::from_raw_parts(buffer.as_ptr().cast(), len) };
    String::from_utf16(utf16_slice).ok()
}

/// Create a VST3-compatible plug-in entry point.
///
/// This is the main entry point for the VST3 Conformal Wrapper, and must
/// be invoked exactly once in each VST3 plug-in binary.
///
/// Note that each VST3 plug-in binary can contain _multiple_ components,
/// so this takes a slice of `EffectClass` and `SynthClass` instances.
///
/// Note that to create a loadable plug-in, you must add this to your
/// `Cargo.toml`:
///
/// ```toml
/// [lib]
/// crate-type = ["cdylib"]
/// ```
///
/// Conformal provides a template project that you can use to get started,
/// using `bun create conformal` script. This will provide a working example
/// of using the VST3 wrapper.
#[macro_export]
macro_rules! wrap_factory {
    ($CLASSES:expr, $INFO:expr) => {
        #[no_mangle]
        #[allow(non_snake_case, clippy::missing_safety_doc, clippy::missing_panics_doc)]
        pub unsafe extern "system" fn GetPluginFactory() -> *mut core::ffi::c_void {
            let factory = conformal_vst_wrapper::_wrap_factory($CLASSES, $INFO);
            vst3::ComWrapper::new(factory)
                .to_com_ptr::<vst3::Steinberg::IPluginFactory>()
                .unwrap()
                .into_raw()
                .cast()
        }

        /// This is required by the API [see here](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/VST+Module+Architecture/Index.html?highlight=GetPluginFactory#module-factory)
        #[cfg(target_os = "macos")]
        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleEntry(_: *mut core::ffi::c_void) -> bool {
            true
        }

        /// This is required by the API [see here](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/VST+Module+Architecture/Index.html?highlight=GetPluginFactory#module-factory)
        #[cfg(target_os = "macos")]
        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "system" fn bundleExit() -> bool {
            true
        }
    };
}
