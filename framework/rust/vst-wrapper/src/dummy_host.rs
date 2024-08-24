use vst3::{
    Class,
    Steinberg::Vst::{IHostApplication, IHostApplicationTrait},
};

#[derive(Default)]
pub struct Host {}

impl IHostApplicationTrait for Host {
    unsafe fn getName(
        &self,
        name: *mut vst3::Steinberg::Vst::String128,
    ) -> vst3::Steinberg::tresult {
        unsafe { super::to_utf16("Dummy Host", &mut (*name)) };
        vst3::Steinberg::kResultOk
    }

    unsafe fn createInstance(
        &self,
        _cid: *mut vst3::Steinberg::TUID,
        _iid: *mut vst3::Steinberg::TUID,
        _obj: *mut *mut std::ffi::c_void,
    ) -> vst3::Steinberg::tresult {
        unimplemented!()
    }
}

impl Class for Host {
    type Interfaces = (IHostApplication,);
}
