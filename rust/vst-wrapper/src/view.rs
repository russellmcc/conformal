use std::{cell::RefCell, ops::Deref, rc};
use vst3::{
    Class, ComPtr, ComRef, ComWrapper,
    Steinberg::{
        IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewContentScaleSupport,
        IPlugViewContentScaleSupport_::ScaleFactor, IPlugViewContentScaleSupportTrait,
        IPlugViewTrait,
    },
};

use conformal_component::parameters;
use conformal_core::parameters::store;
use conformal_ui::{self, Size, Ui, raw_window_handle};

use crate::Resizability;

struct SharedStore<S>(rc::Rc<RefCell<S>>);

impl<S> Clone for SharedStore<S> {
    fn clone(&self) -> Self {
        SharedStore(self.0.clone())
    }
}

#[derive(Debug, Clone, Copy)]
struct SizeFloat {
    width: f32,
    height: f32,
}

impl SizeFloat {
    #[allow(clippy::cast_precision_loss)]
    fn from_size(size: Size) -> Self {
        SizeFloat {
            width: size.width as f32,
            height: size.height as f32,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn from_scaled_vst3_size(size: vst3::Steinberg::ViewRect, scale_factor: f32) -> Self {
        SizeFloat {
            width: (size.right - size.left) as f32 / scale_factor,
            height: (size.bottom - size.top) as f32 / scale_factor,
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn to_size(self) -> Size {
        Size {
            width: self.width as i32,
            height: self.height as i32,
        }
    }
}

struct View<S> {
    store: SharedStore<S>,
    /// Note that we only support a single UI per plugin instance.
    ui: Option<Ui<SharedStore<S>>>,

    domain: String,

    resizability: Resizability,
    /// Note we always store the current size _unscaled_!
    current_size: SizeFloat,

    scale_factor: f32,

    frame: Option<ComPtr<IPlugFrame>>,

    /// Raw self-pointer to our own `IPlugView` interface, needed for passing to
    /// `IPlugFrame::resizeView`. This is set in `create`.
    plug_view_ptr: *mut IPlugView,
}

impl<S> View<S> {
    #[allow(clippy::cast_possible_truncation)]
    fn get_size(&self) -> Size {
        let scale = self.scale_factor;
        Size {
            width: (self.current_size.width * scale).round() as i32,
            height: (self.current_size.height * scale).round() as i32,
        }
    }
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
    resizability: Resizability,
) -> ComPtr<IPlugView> {
    let view = SharedView(rc::Rc::new(ViewCell(RefCell::new(View {
        store: SharedStore(rc::Rc::new(RefCell::new(store))),
        ui: Default::default(),
        domain,
        resizability,
        current_size: SizeFloat::from_size(initial_size),
        scale_factor: 1.0,
        frame: None,
        plug_view_ptr: std::ptr::null_mut(),
    }))));
    let view_as_listener: rc::Rc<dyn store::Listener> = view.clone().0;
    view.borrow_mut()
        .store
        .0
        .borrow_mut()
        .set_listener(rc::Rc::downgrade(&view_as_listener));
    let wrapper = ComWrapper::new(view);
    let plug_view_ptr = wrapper.as_com_ref::<IPlugView>().unwrap().as_ptr();
    wrapper.borrow_mut().plug_view_ptr = plug_view_ptr;
    wrapper.to_com_ptr().unwrap()
}

enum VST3PlatformType {
    #[cfg(target_os = "macos")]
    NSView,

    #[cfg(target_os = "windows")]
    Hwnd,
}

#[cfg(target_os = "macos")]
unsafe fn app_kit_handle(
    raw_ns_view: std::ptr::NonNull<std::ffi::c_void>,
) -> raw_window_handle::AppKitWindowHandle {
    raw_window_handle::AppKitWindowHandle::new(raw_ns_view)
}

#[cfg(target_os = "windows")]
unsafe fn hwnd(
    raw_hwnd: std::ptr::NonNull<std::ffi::c_void>,
) -> raw_window_handle::Win32WindowHandle {
    use std::num::NonZero;

    // Safety: we know the numeric representation will be non-zero since we know that the pointer representation is non-null.
    raw_window_handle::Win32WindowHandle::new(unsafe {
        NonZero::new_unchecked(raw_hwnd.as_ptr() as isize)
    })
}

#[allow(deprecated)]
unsafe fn to_window_handle(
    platform_type: &VST3PlatformType,
    handle: std::ptr::NonNull<std::ffi::c_void>,
) -> raw_window_handle::RawWindowHandle {
    match platform_type {
        #[cfg(target_os = "macos")]
        VST3PlatformType::NSView => unsafe {
            raw_window_handle::RawWindowHandle::from(app_kit_handle(handle))
        },
        #[cfg(target_os = "windows")]
        VST3PlatformType::Hwnd => unsafe { raw_window_handle::RawWindowHandle::from(hwnd(handle)) },
    }
}

impl VST3PlatformType {
    fn from_vst3_str(s: vst3::Steinberg::FIDString) -> Option<VST3PlatformType> {
        match unsafe { std::ffi::CStr::from_ptr(s).to_str() } {
            #[cfg(target_os = "macos")]
            Ok("NSView") => Some(VST3PlatformType::NSView),
            #[cfg(target_os = "windows")]
            Ok("HWND") => Some(VST3PlatformType::Hwnd),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
fn get_rsrc_root_or_panic() -> std::path::PathBuf {
    use conformal_core::mac_bundle_utils::get_current_bundle_info;

    get_current_bundle_info()
        .map(|info| info.resource_path)
        .expect("Could not find bundle resources")
}

#[cfg(target_os = "windows")]
fn get_rsrc_root_or_panic() -> std::path::PathBuf {
    let dll_path = process_path::get_dylib_path().expect("Could not find path to DLL");

    // VST3 bundles have structures like this:
    // - MyPlugin.vst3
    //   - Contents
    //     - Resources
    //       - <Resources go here>
    //     - <target name> (e.g., `x86_64-win`)
    //        - MyPlugin.vst3 <- this is the DLL

    let contents_path = dll_path
        .parent()
        .and_then(|p| p.parent())
        .expect("Could not find Contents directory");
    let resources_path = contents_path.join("Resources");
    assert!(
        resources_path.exists(),
        "Resources directory does not exist This indicates a corrupt VST3 bundle."
    );
    resources_path
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
                let current_size = self.borrow().current_size;
                let rsrc_root = get_rsrc_root_or_panic().join("web-ui");
                self.borrow_mut().ui = Ui::new(
                    handle,
                    store,
                    rsrc_root,
                    domain.as_str(),
                    current_size.to_size(),
                )
                .ok();
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
        let view_size = self.borrow().get_size();
        unsafe {
            (*size).top = 0;
            (*size).left = 0;
            (*size).right = view_size.width;
            (*size).bottom = view_size.height;
            vst3::Steinberg::kResultOk
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    unsafe fn onSize(
        &self,
        scaled_new_size_ptr: *mut vst3::Steinberg::ViewRect,
    ) -> vst3::Steinberg::tresult {
        // Nothing to do if we're not resizable.
        if let Resizability::FixedSize = self.borrow().resizability {
            return vst3::Steinberg::kResultOk;
        }

        // Note that the new size will be scaled by the scale factor!
        let scaled_new_size = unsafe { *scaled_new_size_ptr };
        let new_size =
            SizeFloat::from_scaled_vst3_size(scaled_new_size, self.borrow().scale_factor);
        self.borrow_mut().current_size = new_size;

        // Also alert our web UI that the size has changed.
        if let Some(ui) = self.borrow_mut().ui.as_mut()
            && ui.set_size(new_size.to_size()).is_err()
        {
            return vst3::Steinberg::kInternalError;
        }

        vst3::Steinberg::kResultOk
    }

    unsafe fn onFocus(&self, _state: vst3::Steinberg::TBool) -> vst3::Steinberg::tresult {
        // Unclear if this has a use, maybe this should focus our OS window?
        // In the steinberg SDK this does nothing.
        vst3::Steinberg::kResultFalse
    }

    unsafe fn setFrame(&self, frame: *mut vst3::Steinberg::IPlugFrame) -> vst3::Steinberg::tresult {
        self.borrow_mut().frame =
            (unsafe { ComRef::from_raw(frame) }).map(|frame| frame.to_com_ptr());
        vst3::Steinberg::kResultOk
    }

    unsafe fn canResize(&self) -> vst3::Steinberg::tresult {
        match self.borrow().resizability {
            Resizability::FixedSize => vst3::Steinberg::kResultFalse,
            Resizability::Resizable { .. } => vst3::Steinberg::kResultTrue,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    unsafe fn checkSizeConstraint(
        &self,
        new_scaled_size_ptr: *mut vst3::Steinberg::ViewRect,
    ) -> vst3::Steinberg::tresult {
        match self.borrow().resizability {
            Resizability::FixedSize => vst3::Steinberg::kResultFalse,
            Resizability::Resizable {
                ui_min_size,
                ui_max_size,
            } => {
                let scale = self.borrow().scale_factor;
                let new_size =
                    SizeFloat::from_scaled_vst3_size(unsafe { *new_scaled_size_ptr }, scale);

                if let Some(ui_min_size) = ui_min_size {
                    if new_size.width < ui_min_size.width as f32 {
                        unsafe {
                            (*new_scaled_size_ptr).right = ((*new_scaled_size_ptr).left as f32
                                + (ui_min_size.width as f32) * scale)
                                as i32;
                        }
                    }
                    if new_size.height < ui_min_size.height as f32 {
                        unsafe {
                            (*new_scaled_size_ptr).bottom = ((*new_scaled_size_ptr).top as f32
                                + (ui_min_size.height as f32) * scale)
                                as i32;
                        }
                    }
                }
                if let Some(ui_max_size) = ui_max_size {
                    if new_size.width > ui_max_size.width as f32 {
                        unsafe {
                            (*new_scaled_size_ptr).right = ((*new_scaled_size_ptr).left as f32
                                + (ui_max_size.width as f32) * scale)
                                as i32;
                        }
                    }
                    if new_size.height > ui_max_size.height as f32 {
                        unsafe {
                            (*new_scaled_size_ptr).bottom = ((*new_scaled_size_ptr).top as f32
                                + (ui_max_size.height as f32) * scale)
                                as i32;
                        }
                    }
                }
                vst3::Steinberg::kResultOk
            }
        }
    }
}

impl<S: store::Store + 'static> IPlugViewContentScaleSupportTrait for SharedView<S> {
    unsafe fn setContentScaleFactor(&self, factor: ScaleFactor) -> vst3::Steinberg::tresult {
        self.borrow_mut().scale_factor = factor;
        if let Some(frame) = self.borrow().frame.as_ref() {
            let new_size = self.borrow().get_size();
            let mut new_rect = vst3::Steinberg::ViewRect {
                top: 0,
                left: 0,
                right: new_size.width,
                bottom: new_size.height,
            };
            unsafe { frame.resizeView(self.borrow().plug_view_ptr, &raw mut new_rect) };
        }
        vst3::Steinberg::kResultOk
    }
}

impl<S> Class for SharedView<S> {
    type Interfaces = (IPlugView, IPlugViewContentScaleSupport);
}

// Only include tests in test config on macos
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use vst3::Steinberg::IPlugViewTrait;

    use crate::Resizability;

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
            Resizability::FixedSize,
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
            Resizability::FixedSize,
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
            Resizability::FixedSize,
        );
        let nsview = std::ffi::CString::new("NSView").unwrap();
        assert_ne!(
            unsafe { v.attached(std::ptr::null_mut(), nsview.as_ptr()) },
            vst3::Steinberg::kResultOk
        );
    }
}
