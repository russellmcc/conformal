use super::edit_controller;
use crate::{ClassCategory, enum_to_u32};
use crate::{ClassID, Info};
use vst3::Class;
use vst3::ComWrapper;
use vst3::Steinberg::IPluginFactoryTrait;
use vst3::Steinberg::{FIDString, IPluginFactory2};
use vst3::Steinberg::{IPluginBase, tresult};
use vst3::Steinberg::{IPluginFactory, IPluginFactory2Trait};
use vst3::com_scrape_types::Unknown;

pub struct Factory {
    classes: &'static [&'static dyn ClassCategory],
    info: Info<'static>,
}

#[cfg(test)]
#[path = "factory.tests.rs"]
mod tests;

const EC_TAG: &str = "EC";

impl Factory {
    pub fn new(classes: &'static [&'static dyn ClassCategory], info: Info<'static>) -> Factory {
        assert!(info.vendor.len() < vst3::Steinberg::PFactoryInfo_::kNameSize as usize);
        assert!(info.url.len() < vst3::Steinberg::PFactoryInfo_::kURLSize as usize);
        assert!(info.email.len() < vst3::Steinberg::PFactoryInfo_::kEmailSize as usize);
        assert!(info.version.len() < vst3::Steinberg::PClassInfo2_::kVersionSize as usize);

        for class in classes {
            assert!(
                class.info().name.len()
                    < vst3::Steinberg::PClassInfo_::kNameSize as usize - EC_TAG.len()
            );
        }
        Factory { classes, info }
    }
}

fn to_cstr<'a, T: Iterator<Item = &'a mut i8>>(s: &str, it: T) {
    let s = s.as_bytes();
    for (i, c) in it.enumerate() {
        if i < s.len() {
            *c = s[i] as i8;
        } else {
            *c = 0;
        }
    }
}

unsafe fn compare_fid(a: FIDString, b: ClassID) -> bool {
    unsafe {
        for i in 0..16 {
            if *a.offset(i) != b[i as usize] as i8 {
                return false;
            }
        }
        true
    }
}

unsafe fn to_iid(iid: FIDString) -> [u8; 16] {
    unsafe {
        let mut ret = [0; 16];
        for i in 0isize..16 {
            ret[i as usize] = *iid.offset(i) as u8;
        }
        ret
    }
}

impl IPluginFactoryTrait for Factory {
    unsafe fn countClasses(&self) -> vst3::Steinberg::int32 {
        (self.classes.len() * 2).try_into().unwrap()
    }

    unsafe fn createInstance(
        &self,
        class_id: FIDString,
        interface_id: FIDString,
        obj: *mut *mut ::std::ffi::c_void,
    ) -> tresult {
        unsafe {
            for class in self.classes {
                if compare_fid(class_id, class.info().edit_controller_cid) {
                    let com_ptr = ComWrapper::new(edit_controller::create(
                        class.create_parameter_model(),
                        class.info().ui_initial_size,
                        class.get_kind(),
                    ))
                    .to_com_ptr::<IPluginBase>()
                    .unwrap();

                    if let Some(i) =
                        IPluginBase::query_interface(com_ptr.as_ptr(), &to_iid(interface_id))
                    {
                        *obj = i;
                        return vst3::Steinberg::kResultOk;
                    }
                    return vst3::Steinberg::kNoInterface;
                }
                if compare_fid(class_id, class.info().cid) {
                    let com_ptr = class.create_processor(class.info().edit_controller_cid);

                    if let Some(i) =
                        IPluginBase::query_interface(com_ptr.as_ptr(), &to_iid(interface_id))
                    {
                        *obj = i;
                        return vst3::Steinberg::kResultOk;
                    }
                    return vst3::Steinberg::kNoInterface;
                }
            }
            vst3::Steinberg::kInvalidArgument
        }
    }

    unsafe fn getClassInfo(
        &self,
        index: vst3::Steinberg::int32,
        info: *mut vst3::Steinberg::PClassInfo,
    ) -> tresult {
        unsafe {
            if let Some(class) = &self.classes.get(index as usize / 2) {
                let is_ec = index % 2 == 1;

                (*info).cardinality =
                    vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as i32;

                if is_ec {
                    (*info)
                        .cid
                        .iter_mut()
                        .zip(class.info().edit_controller_cid.iter())
                        .for_each(|(a, b)| *a = *b as i8);
                    to_cstr("Component Controller Class", (*info).category.iter_mut());
                    to_cstr(class.info().name, (*info).name.iter_mut());
                    to_cstr(
                        EC_TAG,
                        (*info).name.iter_mut().skip(class.info().name.len()),
                    );
                } else {
                    (*info)
                        .cid
                        .iter_mut()
                        .zip(class.info().cid.iter())
                        .for_each(|(a, b)| *a = *b as i8);
                    to_cstr("Audio Module Class", (*info).category.iter_mut());
                    to_cstr(class.info().name, (*info).name.iter_mut());
                }
                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }

    unsafe fn getFactoryInfo(&self, info: *mut vst3::Steinberg::PFactoryInfo) -> tresult {
        unsafe {
            to_cstr(self.info.vendor, (*info).vendor.iter_mut());
            to_cstr(self.info.url, (*info).url.iter_mut());
            to_cstr(self.info.email, (*info).email.iter_mut());
            (*info).flags = vst3::Steinberg::PFactoryInfo_::FactoryFlags_::kUnicode as i32;
            vst3::Steinberg::kResultOk
        }
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(
        &self,
        index: vst3::Steinberg::int32,
        info: *mut vst3::Steinberg::PClassInfo2,
    ) -> tresult {
        unsafe {
            if let Some(class) = &self.classes.get(index as usize / 2) {
                let is_ec = index % 2 == 1;
                (*info).cardinality =
                    vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as i32;

                if is_ec {
                    (*info)
                        .cid
                        .iter_mut()
                        .zip(class.info().edit_controller_cid.iter())
                        .for_each(|(a, b)| *a = *b as i8);
                    to_cstr("Component Controller Class", (*info).category.iter_mut());
                    to_cstr(class.info().name, (*info).name.iter_mut());
                    to_cstr(
                        EC_TAG,
                        (*info).name.iter_mut().skip(class.info().name.len()),
                    );
                } else {
                    (*info)
                        .cid
                        .iter_mut()
                        .zip(class.info().cid.iter())
                        .for_each(|(a, b)| *a = *b as i8);
                    to_cstr("Audio Module Class", (*info).category.iter_mut());
                    to_cstr(class.info().name, (*info).name.iter_mut());
                }
                to_cstr(class.category_str(), (*info).subCategories.iter_mut());
                (*info).classFlags =
                    enum_to_u32(vst3::Steinberg::Vst::ComponentFlags_::kDistributable).unwrap();
                to_cstr(
                    std::ffi::CStr::from_ptr(vst3::Steinberg::Vst::SDKVersionString)
                        .to_str()
                        .unwrap(),
                    (*info).sdkVersion.iter_mut(),
                );
                to_cstr(self.info.version, (*info).version.iter_mut());
                to_cstr(self.info.vendor, (*info).vendor.iter_mut());
                vst3::Steinberg::kResultOk
            } else {
                vst3::Steinberg::kInvalidArgument
            }
        }
    }
}

impl Class for Factory {
    type Interfaces = (IPluginFactory, IPluginFactory2);
}
