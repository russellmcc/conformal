use serde::{Deserialize, Serialize};
use std::{cell::RefCell, ops::Deref, rc};
use vst3::{
    Class, ComPtr, ComRef, ComWrapper,
    Steinberg::{
        IPlugFrame, IPlugView, IPlugViewContentScaleSupport,
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

/// Unscaled size of the UI in logical pixels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

fn from_vst3_size(size: vst3::Steinberg::ViewRect) -> Size {
    Size {
        width: size.right - size.left,
        height: size.bottom - size.top,
    }
}

fn scale_size(size: LogicalSize, scale_factor: f32) -> LogicalSize {
    LogicalSize {
        width: (size.width * scale_factor),
        height: (size.height * scale_factor),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn round_size(size: LogicalSize) -> Size {
    Size {
        width: size.width.round() as i32,
        height: size.height.round() as i32,
    }
}

impl From<Size> for LogicalSize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn from(size: Size) -> Self {
        LogicalSize {
            width: size.width as f32,
            height: size.height as f32,
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn unscale_size(size: Size, scale_factor: f32) -> LogicalSize {
    LogicalSize {
        width: (size.width as f32 / scale_factor),
        height: (size.height as f32 / scale_factor),
    }
}

/// Trait representing persistent storage for the current size of the UI.
///
/// Note that this is a persistance layer but NOT a reactivity layer -
/// while a view is active, it is the source of truth for the size.
pub trait SizePersistance {
    fn set_size(&mut self, size: LogicalSize);
    fn get_size(&self) -> LogicalSize;
}

struct View<S> {
    store: SharedStore<S>,
    /// Note that we only support a single UI per plugin instance.
    ui: Option<Ui<SharedStore<S>>>,

    domain: String,

    resizability: Resizability,

    /// Note we always store the current size _scaled_! This lets us ensure we don't
    /// get rounding errors.
    current_size: Size,
    scale_factor: f32,

    /// When the scale factor changes, the VST3 spec is a bit weird:
    ///
    /// 1) Host notifies us of the new scale factor with `setContentScaleFactor`
    ///    a) We are expected to call `IPlugFrame::resizeView` with the new size
    /// 2) Host is expected to call `IPlugView::onSize` with the new size
    ///
    /// In this scenario, if the host calls getSize after step 1 but before step 2,
    /// it's illegal for us to return the new value.
    ///
    /// That said, we need to behave a bit differently on "true" resizes vs these
    /// scale factor changes:
    ///
    /// A) We shouldn't change size of the wry view, since wry manages scale factor internally.
    /// B) We shouldn't persist the change, since this wasn't a _logical_ resize.
    ///
    /// We implement this by setting this variable in step 1a, and clearing it after
    /// step 2, if we end up receiving a notification for the same size we asked for.
    /// In this case, we skip updating the size of the wry view and the persistance layer.
    pending_scale_factor_change: Option<f32>,

    frame: Option<ComPtr<IPlugFrame>>,

    /// Raw self-pointer to our own `IPlugView` interface, needed for passing to
    /// `IPlugFrame::resizeView`. This is set in `create`.
    plug_view_ptr: *mut IPlugView,
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

impl<S: SizePersistance> SizePersistance for SharedStore<S> {
    fn set_size(&mut self, size: LogicalSize) {
        self.0.borrow_mut().set_size(size);
    }

    fn get_size(&self) -> LogicalSize {
        self.0.borrow().get_size()
    }
}

pub fn create<S: store::Store + SizePersistance + 'static>(
    store: S,
    domain: String,
    resizability: Resizability,
) -> ComPtr<IPlugView> {
    let initial_size = store.get_size();
    let view = SharedView(rc::Rc::new(ViewCell(RefCell::new(View {
        store: SharedStore(rc::Rc::new(RefCell::new(store))),
        ui: Default::default(),
        domain,
        resizability,
        current_size: round_size(initial_size),
        pending_scale_factor_change: None,
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

impl<S: store::Store + 'static> SharedView<S> {
    fn apply_size_change_to_ui(&self, scaled_new_size: Size) -> Result<(), conformal_ui::UiError> {
        let scale_factor = self.borrow().scale_factor;

        // Also alert our web UI that the size has changed.
        if let Some(ui) = self.borrow_mut().ui.as_mut() {
            return ui.set_size(round_size(unscale_size(scaled_new_size, scale_factor)));
        }
        Ok(())
    }
}

impl<S: store::Store + SizePersistance + 'static> IPlugViewTrait for SharedView<S> {
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
                self.borrow_mut().ui =
                    Ui::new(handle, store, rsrc_root, domain.as_str(), current_size).ok();
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
        let view_size = self.borrow().current_size;
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
        // Note that the new size will be scaled by the scale factor!
        let scaled_new_size = from_vst3_size(unsafe { *scaled_new_size_ptr });

        // Special case, if this is simply the result of a scale factor change, we do need to
        // update our internal scaled size, even if we're not resizable.
        let pending_scale_factor_change = self.borrow().pending_scale_factor_change;
        if let Some(pending_scale_factor_change) = pending_scale_factor_change {
            let pending_scaled_new_size =
                scale_size(self.borrow().store.get_size(), pending_scale_factor_change);
            // Note that we allow a margin here since some hosts (like Tracktion waveform)
            // can ignore the size we told it to resize to, and instead use some re-rounded size.
            if (pending_scaled_new_size.width - scaled_new_size.width as f32).abs() <= 1.0
                && (pending_scaled_new_size.height - scaled_new_size.height as f32).abs() <= 1.0
            {
                self.borrow_mut().pending_scale_factor_change = None;
                self.borrow_mut().current_size = scaled_new_size;

                // Note that the web ui scales itself, but it doesn't detect scale factor changes.
                // So, we need to notify it here.
                if (self.apply_size_change_to_ui(scaled_new_size)).is_err() {
                    return vst3::Steinberg::kInternalError;
                }

                return vst3::Steinberg::kResultOk;
            }
        }

        // Otherwise, we got some other update from the host before our pending scale factor change
        // - so just clear it.
        self.borrow_mut().pending_scale_factor_change = None;

        // Nothing to do if we're not resizable.
        if let Resizability::FixedSize = self.borrow().resizability {
            return vst3::Steinberg::kResultOk;
        }

        self.borrow_mut().current_size = scaled_new_size;
        let scale_factor = self.borrow().scale_factor;

        // Also alert our web UI that the size has changed.
        if (self.apply_size_change_to_ui(scaled_new_size)).is_err() {
            return vst3::Steinberg::kInternalError;
        }

        // Also alert our size persistance layer that the size has changed.
        self.borrow_mut()
            .store
            .set_size(unscale_size(scaled_new_size, scale_factor));

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
                let scaled_new_size = from_vst3_size(unsafe { *new_scaled_size_ptr });

                if let Some(unscaled_ui_min_size) = ui_min_size {
                    let scaled_ui_min_size =
                        round_size(scale_size(unscaled_ui_min_size.into(), scale));
                    if scaled_new_size.width < scaled_ui_min_size.width {
                        unsafe {
                            (*new_scaled_size_ptr).right =
                                (*new_scaled_size_ptr).left + scaled_ui_min_size.width;
                        }
                    }
                    if scaled_new_size.height < scaled_ui_min_size.height {
                        unsafe {
                            (*new_scaled_size_ptr).bottom =
                                (*new_scaled_size_ptr).top + scaled_ui_min_size.height;
                        }
                    }
                }
                if let Some(unscaled_ui_max_size) = ui_max_size {
                    let scaled_ui_max_size =
                        round_size(scale_size(unscaled_ui_max_size.into(), scale));
                    if scaled_new_size.width > scaled_ui_max_size.width {
                        unsafe {
                            (*new_scaled_size_ptr).right =
                                (*new_scaled_size_ptr).left + scaled_ui_max_size.width;
                        }
                    }
                    if scaled_new_size.height > scaled_ui_max_size.height {
                        unsafe {
                            (*new_scaled_size_ptr).bottom =
                                (*new_scaled_size_ptr).top + scaled_ui_max_size.height;
                        }
                    }
                }
                vst3::Steinberg::kResultOk
            }
        }
    }
}

impl<S: store::Store + SizePersistance + 'static> IPlugViewContentScaleSupportTrait
    for SharedView<S>
{
    // Note we use windows implementation in tests on all platforms.
    // This is because non-windows implementation is trivial,
    // and as of this writing we only run tests for this module on macOS.
    #[cfg(any(target_os = "windows", test))]
    unsafe fn setContentScaleFactor(&self, factor: ScaleFactor) -> vst3::Steinberg::tresult {
        use vst3::Steinberg::IPlugFrameTrait;

        let unscaled_size = self.borrow().store.get_size();
        self.borrow_mut().scale_factor = factor;
        let new_scaled_size = round_size(scale_size(unscaled_size, factor));
        let frame = self.borrow().frame.clone();
        let plug_view_ptr = self.borrow().plug_view_ptr;
        if let Some(frame) = frame {
            self.borrow_mut().pending_scale_factor_change = Some(factor);
            let mut new_rect = vst3::Steinberg::ViewRect {
                top: 0,
                left: 0,
                right: new_scaled_size.width,
                bottom: new_scaled_size.height,
            };
            unsafe { frame.resizeView(plug_view_ptr, &raw mut new_rect) };
        } else {
            // If we don't have a frame set-up, we can't advertise the new size, so simply
            // update our current size immediately.
            self.borrow_mut().current_size = new_scaled_size;
        }
        vst3::Steinberg::kResultOk
    }

    #[cfg(not(any(target_os = "windows", test)))]
    unsafe fn setContentScaleFactor(&self, _: ScaleFactor) -> vst3::Steinberg::tresult {
        // macOS vst3 is not supposed to send setContentScaleFactor, so if we get it,
        // ignore it.
        //
        // VST docs for this function imply we are supposed to return kResultFalse here when we
        // no-op.
        vst3::Steinberg::kResultFalse
    }
}

impl<S> Class for SharedView<S> {
    type Interfaces = (IPlugView, IPlugViewContentScaleSupport);
}

// Only include tests in test config on macos
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::cell::RefCell;

    use vst3::Steinberg::{
        IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewContentScaleSupport_::ScaleFactor,
        IPlugViewContentScaleSupportTrait, IPlugViewTrait,
    };
    use vst3::{Class, ComPtr, ComWrapper};

    use crate::Resizability;

    use super::{LogicalSize, SizePersistance, create};
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

    impl SizePersistance for DummyStore {
        fn set_size(&mut self, _size: LogicalSize) {}
        fn get_size(&self) -> LogicalSize {
            LogicalSize {
                width: 100.0,
                height: 100.0,
            }
        }
    }

    #[derive(Default)]
    struct MockFrame {
        resized_sizes: RefCell<Vec<(i32, i32)>>,
    }

    impl IPlugFrameTrait for MockFrame {
        unsafe fn resizeView(
            &self,
            _view: *mut IPlugView,
            new_size: *mut vst3::Steinberg::ViewRect,
        ) -> vst3::Steinberg::tresult {
            let new_size = unsafe { *new_size };
            self.resized_sizes.borrow_mut().push((
                new_size.right - new_size.left,
                new_size.bottom - new_size.top,
            ));
            vst3::Steinberg::kResultOk
        }
    }

    impl Class for MockFrame {
        type Interfaces = (IPlugFrame,);
    }

    fn get_size(view: &ComPtr<IPlugView>) -> vst3::Steinberg::ViewRect {
        let mut size = vst3::Steinberg::ViewRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            assert_eq!(view.getSize(&raw mut size), vst3::Steinberg::kResultOk);
        }
        size
    }

    #[test]
    fn nsview_platform_supported() {
        let v = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
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
        let v = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
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
        let v = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
        let nsview = std::ffi::CString::new("NSView").unwrap();
        assert_ne!(
            unsafe { v.attached(std::ptr::null_mut(), nsview.as_ptr()) },
            vst3::Steinberg::kResultOk
        );
    }

    #[test]
    fn set_content_scale_factor_then_on_size_succeeds_when_frame_attached() {
        let view = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
        let frame = ComWrapper::new(MockFrame::default());
        let scale_support = view
            .cast::<vst3::Steinberg::IPlugViewContentScaleSupport>()
            .unwrap();
        unsafe {
            assert_eq!(
                view.setFrame(frame.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                scale_support
                    .as_com_ref()
                    .setContentScaleFactor(2.0f32 as ScaleFactor),
                vst3::Steinberg::kResultOk
            );
        }
        assert_eq!(frame.resized_sizes.borrow().as_slice(), &[(200, 200)]);
        let get_before_size = get_size(&view);
        assert_eq!(get_before_size.right, 100);
        assert_eq!(get_before_size.bottom, 100);
        let mut rect = vst3::Steinberg::ViewRect {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        unsafe {
            assert_eq!(view.onSize(&raw mut rect), vst3::Steinberg::kResultOk);
        }
        let get_after_size = get_size(&view);
        assert_eq!(get_after_size.right, 200);
        assert_eq!(get_after_size.bottom, 200);
    }

    #[test]
    fn set_content_scale_factor_accepts_on_size_within_slush_factor() {
        let view = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
        let frame = ComWrapper::new(MockFrame::default());
        let scale_support = view
            .cast::<vst3::Steinberg::IPlugViewContentScaleSupport>()
            .unwrap();
        unsafe {
            assert_eq!(
                view.setFrame(frame.as_com_ref().unwrap().as_ptr()),
                vst3::Steinberg::kResultOk
            );
            assert_eq!(
                scale_support
                    .as_com_ref()
                    .setContentScaleFactor(2.0f32 as ScaleFactor),
                vst3::Steinberg::kResultOk
            );
        }
        assert_eq!(frame.resized_sizes.borrow().as_slice(), &[(200, 200)]);

        let mut rect = vst3::Steinberg::ViewRect {
            left: 0,
            top: 0,
            right: 201,
            bottom: 199,
        };
        unsafe {
            assert_eq!(view.onSize(&raw mut rect), vst3::Steinberg::kResultOk);
        }

        let size = get_size(&view);
        assert_eq!(size.right, 201);
        assert_eq!(size.bottom, 199);
    }

    #[test]
    fn set_content_scale_factor_immediately_updates_size_without_frame() {
        let view = create(DummyStore {}, "test".to_string(), Resizability::FixedSize);
        let scale_support = view
            .cast::<vst3::Steinberg::IPlugViewContentScaleSupport>()
            .unwrap();
        unsafe {
            assert_eq!(
                scale_support
                    .as_com_ref()
                    .setContentScaleFactor(2.0f32 as ScaleFactor),
                vst3::Steinberg::kResultOk
            );
        }

        let size = get_size(&view);
        assert_eq!(size.right, 200);
        assert_eq!(size.bottom, 200);
    }
}
