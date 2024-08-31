use vst3::Steinberg::IPlugViewTrait;

use super::create;
use conformal_component::parameters;
struct DummyStore;

impl conformal_component::parameters::store::Store for DummyStore {
    fn get(&self, _unique_id: &str) -> Option<parameters::Value> {
        None
    }

    fn set_listener(&mut self, _listener: std::rc::Weak<dyn parameters::store::Listener>) {}

    fn set(
        &mut self,
        _unique_id: &str,
        _value: parameters::Value,
    ) -> Result<(), parameters::store::SetError> {
        Ok(())
    }

    fn set_grabbed(
        &mut self,
        _unique_id: &str,
        _grabbed: bool,
    ) -> Result<(), parameters::store::SetGrabbedError> {
        Ok(())
    }

    fn get_info(&self, _unique_id: &str) -> Option<parameters::Info> {
        None
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
