use std::{cell::RefCell, ops::Deref, rc};
use vst3::{
    Class, ComPtr, ComWrapper,
    Steinberg::{IPlugView, IPlugViewTrait},
};

use conformal_component::parameters;
use conformal_core::parameters::store;
use conformal_ui::{self, Size, Ui, raw_window_handle};

struct SharedStore<S>(rc::Rc<RefCell<S>>);

impl<S> Clone for SharedStore<S> {
    fn clone(&self) -> Self {
        SharedStore(self.0.clone())
    }
}

struct View<S> {
    store: SharedStore<S>,
    /// Note that we only support a single UI per plugin instance.
    ui: Option<Ui<SharedStore<S>>>,

    domain: String,

    initial_size: Size,
}

struct ViewCell<S>(RefCell<View<S>>);

impl<S: store::Store + 'static> store::Listener for ViewCell<S> {
    fn parameter_changed(&self, unique_id: &str, value: &parameters::Value) {
        if let Some(ui) = self.0.borrow_mut().ui.as_mut() {
            ui.update_parameter(unique_id, value);
        }
    }

    fn ui_state_changed(&self, state: &[u8]) {
        if let Some(ui) = self.0.borrow_mut().ui.as_mut() {
            ui.update_ui_state(state);
        }
    }
}

struct SharedView<S>(rc::Rc<ViewCell<S>>);

impl<S> Clone for SharedView<S> {
    fn clone(&self) -> Self {
        SharedView(self.0.clone())
    }
}

impl<S> Deref for SharedView<S> {
    type Target = RefCell<View<S>>;

    fn deref(&self) -> &Self::Target {
        &self.0.0
    }
}

impl<S: store::Store> conformal_ui::ParameterStore for SharedStore<S> {
    fn get(&self, unique_id: &str) -> Option<parameters::Value> {
        self.0.borrow().get(unique_id)
    }

    fn set(&mut self, unique_id: &str, value: parameters::Value) -> Result<(), store::SetError> {
        self.0.borrow_mut().set(unique_id, value)
    }

    fn set_grabbed(
        &mut self,
        unique_id: &str,
        grabbed: bool,
    ) -> Result<(), store::SetGrabbedError> {
        self.0.borrow_mut().set_grabbed(unique_id, grabbed)
    }

    fn get_info(&self, unique_id: &str) -> Option<parameters::Info> {
        self.0.borrow().get_info(unique_id)
    }

    fn get_ui_state(&self) -> Vec<u8> {
        self.0.borrow().get_ui_state()
    }

    fn set_ui_state(&mut self, state: &[u8]) {
        self.0.borrow_mut().set_ui_state(state);
    }
}

pub fn create<S: store::Store + 'static>(
    store: S,
    domain: String,
    initial_size: Size,
) -> ComPtr<IPlugView> {
    let view = SharedView(rc::Rc::new(ViewCell(RefCell::new(View {
        store: SharedStore(rc::Rc::new(RefCell::new(store))),
        ui: Default::default(),
        domain,
        initial_size,
    }))));
    let view_as_listener: rc::Rc<dyn store::Listener> = view.clone().0;
    view.borrow_mut()
        .store
        .0
        .borrow_mut()
        .set_listener(rc::Rc::downgrade(&view_as_listener));
    ComWrapper::new(view).to_com_ptr().unwrap()
}

enum VST3PlatformType {
    #[cfg(target_os = "macos")]
    NSView,
}

#[cfg(target_os = "macos")]
unsafe fn app_kit_handle(
    raw_ns_view: std::ptr::NonNull<std::ffi::c_void>,
) -> raw_window_handle::AppKitWindowHandle {
    raw_window_handle::AppKitWindowHandle::new(raw_ns_view)
}

#[allow(deprecated)]
unsafe fn to_window_handle(
    platform_type: &VST3PlatformType,
    handle: std::ptr::NonNull<std::ffi::c_void>,
) -> raw_window_handle::RawWindowHandle {
    unsafe {
        match platform_type {
            #[cfg(target_os = "macos")]
            VST3PlatformType::NSView => {
                raw_window_handle::RawWindowHandle::from(app_kit_handle(handle))
            }
        }
    }
}

impl VST3PlatformType {
    fn from_vst3_str(s: vst3::Steinberg::FIDString) -> Option<VST3PlatformType> {
        match unsafe { std::ffi::CStr::from_ptr(s).to_str() } {
            #[cfg(target_os = "macos")]
            Ok("NSView") => Some(VST3PlatformType::NSView),
            _ => None,
        }
    }
}

impl<S: store::Store + 'static> IPlugViewTrait for SharedView<S> {
    unsafe fn isPlatformTypeSupported(
        &self,
        platform_type: vst3::Steinberg::FIDString,
    ) -> vst3::Steinberg::tresult {
        match VST3PlatformType::from_vst3_str(platform_type) {
            Some(_) => vst3::Steinberg::kResultTrue,
            None => vst3::Steinberg::kResultFalse,
        }
    }

    unsafe fn attached(
        &self,
        parent: *mut std::ffi::c_void,
        platform_type: vst3::Steinberg::FIDString,
    ) -> vst3::Steinberg::tresult {
        unsafe {
            if let (Some(platform_type), Some(parent)) = (
                VST3PlatformType::from_vst3_str(platform_type),
                std::ptr::NonNull::new(parent),
            ) {
                let handle = to_window_handle(&platform_type, parent);
                let store = self.borrow().store.clone();
                let domain = self.borrow().domain.clone();
                let initial_size = self.borrow().initial_size;
                self.borrow_mut().ui = Ui::new(handle, store, domain.as_str(), initial_size).ok();
                return vst3::Steinberg::kResultOk;
            }
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn removed(&self) -> vst3::Steinberg::tresult {
        self.borrow_mut().ui = None;
        vst3::Steinberg::kResultOk
    }

    unsafe fn onWheel(&self, _distance: f32) -> vst3::Steinberg::tresult {
        // Note that we currently don't support handling events sent from the host
        // (see #27)
        vst3::Steinberg::kResultFalse
    }

    unsafe fn onKeyDown(
        &self,
        _key: vst3::Steinberg::char16,
        _key_code: vst3::Steinberg::int16,
        _modifiers: vst3::Steinberg::int16,
    ) -> vst3::Steinberg::tresult {
        // Note that we currently don't support handling events sent from the host
        // (see #27)
        vst3::Steinberg::kResultFalse
    }

    unsafe fn onKeyUp(
        &self,
        _key: vst3::Steinberg::char16,
        _key_code: vst3::Steinberg::int16,
        _modifiers: vst3::Steinberg::int16,
    ) -> vst3::Steinberg::tresult {
        // Note that we currently don't support handling events sent from the host
        // (see #27)
        vst3::Steinberg::kResultFalse
    }

    unsafe fn getSize(&self, size: *mut vst3::Steinberg::ViewRect) -> vst3::Steinberg::tresult {
        unsafe {
            (*size).top = 0;
            (*size).left = 0;
            (*size).right = self.borrow().initial_size.width;
            (*size).bottom = self.borrow().initial_size.height;
            vst3::Steinberg::kResultOk
        }
    }

    unsafe fn onSize(&self, _new_size: *mut vst3::Steinberg::ViewRect) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kNotImplemented
    }

    unsafe fn onFocus(&self, _state: vst3::Steinberg::TBool) -> vst3::Steinberg::tresult {
        // Unclear if this has a use, maybe this should focus our OS window?
        // In the steinberg SDK this does nothing.
        vst3::Steinberg::kResultFalse
    }

    unsafe fn setFrame(
        &self,
        _frame: *mut vst3::Steinberg::IPlugFrame,
    ) -> vst3::Steinberg::tresult {
        // This is the hook-up for allowing the UI to resize itself. We don't suppor this (#28)
        // So we don't store it.
        vst3::Steinberg::kResultOk
    }

    unsafe fn canResize(&self) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kResultFalse
    }

    unsafe fn checkSizeConstraint(
        &self,
        _rect: *mut vst3::Steinberg::ViewRect,
    ) -> vst3::Steinberg::tresult {
        vst3::Steinberg::kNotImplemented
    }
}

impl<S> Class for SharedView<S> {
    type Interfaces = (IPlugView,);
}

// Only include tests in test config on macos
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use vst3::Steinberg::IPlugViewTrait;

    use super::create;
    use conformal_component::parameters;
    use conformal_core::parameters::store;
    struct DummyStore;

    impl store::Store for DummyStore {
        fn get(&self, _unique_id: &str) -> Option<parameters::Value> {
            None
        }

        fn set_listener(&mut self, _listener: std::rc::Weak<dyn store::Listener>) {}

        fn set(
            &mut self,
            _unique_id: &str,
            _value: parameters::Value,
        ) -> Result<(), store::SetError> {
            Ok(())
        }

        fn set_grabbed(
            &mut self,
            _unique_id: &str,
            _grabbed: bool,
        ) -> Result<(), store::SetGrabbedError> {
            Ok(())
        }

        fn get_info(&self, _unique_id: &str) -> Option<parameters::Info> {
            None
        }

        fn set_ui_state(&mut self, _state: &[u8]) {}

        fn get_ui_state(&self) -> Vec<u8> {
            vec![]
        }
    }

    #[test]
    fn nsview_platform_supported() {
        let v = create(
            DummyStore {},
            "test".to_string(),
            conformal_ui::Size {
                width: 100,
                height: 100,
            },
        );
        let nsview = std::ffi::CString::new("NSView").unwrap();
        unsafe {
            assert_eq!(
                v.isPlatformTypeSupported(nsview.as_ptr()),
                vst3::Steinberg::kResultTrue
            );
        }
    }

    #[test]
    fn bananas_platform_not_supported() {
        let v = create(
            DummyStore {},
            "test".to_string(),
            conformal_ui::Size {
                width: 100,
                height: 100,
            },
        );
        // Maybe some day, we will support bananas...
        let nsview = std::ffi::CString::new("Bananas").unwrap();
        unsafe {
            assert_eq!(
                v.isPlatformTypeSupported(nsview.as_ptr()),
                vst3::Steinberg::kResultFalse
            );
        }
    }

    #[test]
    fn defends_against_null_parent() {
        let v = create(
            DummyStore {},
            "test".to_string(),
            conformal_ui::Size {
                width: 100,
                height: 100,
            },
        );
        let nsview = std::ffi::CString::new("NSView").unwrap();
        assert_ne!(
            unsafe { v.attached(std::ptr::null_mut(), nsview.as_ptr()) },
            vst3::Steinberg::kResultOk
        );
    }
}
