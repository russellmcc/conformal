use super::Factory;
use crate::{ClassInfo, Info};
use crate::{HostInfo, SynthClass};
use conformal_component::audio::BufferMut;
use conformal_component::events::{Data, Event, Events};
use conformal_component::parameters::{BufferStates, States};
use conformal_component::synth::Synth;
use conformal_component::{Component, ProcessingEnvironment, Processor};
use vst3::Steinberg::Vst::{IComponent, IComponentTrait};
use vst3::Steinberg::{IPluginFactory2Trait, IPluginFactoryTrait};
use vst3::{ComPtr, Interface};

struct DummyComponent {}

#[derive(Default)]
struct DummySynth {}

impl Processor for DummySynth {
    fn set_processing(&mut self, _processing: bool) {
        unimplemented!()
    }
}

impl Synth for DummySynth {
    fn handle_events<E: IntoIterator<Item = Data>, P: States>(
        &mut self,
        _events: E,
        _parameters: P,
    ) {
        unimplemented!()
    }

    fn process<E: IntoIterator<Item = Event>, P: BufferStates, O: BufferMut>(
        &mut self,
        _events: Events<E>,
        _parameters: P,
        _output: &mut O,
    ) {
        unimplemented!()
    }
}

impl Component for DummyComponent {
    type Processor = DummySynth;

    fn create_processor(&self, _: &ProcessingEnvironment) -> Self::Processor {
        Default::default()
    }
}

#[test]
#[should_panic]
fn test_too_long_vendor() {
    let _ = Factory::new(
        &[],
        Info {
            vendor: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
}

#[test]
#[should_panic]
fn test_too_long_url() {
    let _ = Factory::new(
        &[],
        Info {
            vendor: "test",
            url: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
}

#[test]
#[should_panic]
fn test_too_long_email() {
    let _ = Factory::new(
        &[],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            version: "1.0.0",
        },
    );
}

#[test]
#[should_panic]
fn test_too_long_class_name() {
    let wrapper = Factory::new(
        &[&SynthClass {
            info: ClassInfo {
                name: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                cid: [4; 16],
                edit_controller_cid: [5; 16],
                ui_initial_size: crate::UiSize {
                    width: 800,
                    height: 400,
                },
            },
            factory: |_: &HostInfo| DummyComponent {},
        }],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let _class_count = unsafe { wrapper.countClasses() };
}

fn from_cstr<'a, T: IntoIterator<Item = &'a i8>>(t: T) -> String {
    t.into_iter()
        .map(|x| -> char { (*x as u8) as char })
        .filter(|x| -> bool { *x != '\0' })
        .collect::<String>()
}

#[test]
fn basic_info_test() {
    let vendor = "test";
    let url = "https://example.com";
    let email = "awesome@example.com";
    let version = "1.0.0";
    let wrapper = Factory::new(
        &[],
        Info {
            vendor,
            url,
            email,
            version,
        },
    );
    let mut info = vst3::Steinberg::PFactoryInfo {
        vendor: [66; 64],
        url: [66; 256],
        email: [66; 128],
        flags: 99,
    };
    unsafe {
        wrapper.getFactoryInfo(&mut info);
    }
    assert_eq!(from_cstr(&info.vendor).as_str(), vendor);
    assert_eq!(from_cstr(&info.url).as_str(), url);
    assert_eq!(from_cstr(&info.email).as_str(), email);
    assert_eq!(
        info.flags,
        vst3::Steinberg::PFactoryInfo_::FactoryFlags_::kUnicode as i32
    );
}

#[test]
fn basic_class_info_test() {
    const NAME: &str = "test";
    const CID: [u8; 16] = [4; 16];
    const EDIT_CONTROLLER_CID: [u8; 16] = [5; 16];
    let wrapper = Factory::new(
        &[&SynthClass {
            info: ClassInfo {
                name: NAME,
                cid: CID,
                edit_controller_cid: EDIT_CONTROLLER_CID,
                ui_initial_size: crate::UiSize {
                    width: 800,
                    height: 400,
                },
            },
            factory: &|_: &HostInfo| DummyComponent {},
        }],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let class_count = unsafe { wrapper.countClasses() };
    assert_eq!(class_count, 2);
    let mut info = vst3::Steinberg::PClassInfo {
        category: [66; 32],
        name: [66; 64],
        cardinality: 0i32,
        cid: [0; 16],
    };
    unsafe {
        assert_eq!(
            wrapper.getClassInfo(0, &mut info),
            vst3::Steinberg::kResultOk
        );
    }
    assert_eq!(from_cstr(&info.category).as_str(), "Audio Module Class");
    assert_eq!(from_cstr(&info.name).as_str(), NAME);
    assert_eq!(
        info.cardinality,
        vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as i32
    );
    assert!(info.cid.iter().zip(CID.iter()).all(|(a, b)| *a == *b as i8));
    unsafe {
        assert_eq!(
            wrapper.getClassInfo(1, &mut info),
            vst3::Steinberg::kResultOk
        );
    }
    assert_eq!(
        from_cstr(&info.category).as_str(),
        "Component Controller Class"
    );

    assert_eq!(
        from_cstr(&info.name).as_str(),
        (NAME.to_string() + "EC").as_str()
    );

    assert!(
        info.cid
            .iter()
            .zip(EDIT_CONTROLLER_CID.iter())
            .all(|(a, b)| *a == *b as i8)
    );
}

#[test]
fn defends_against_bad_index() {
    let wrapper = Factory::new(
        &[],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let result = unsafe { wrapper.getClassInfo(0, std::ptr::null_mut()) };
    assert_ne!(result, vst3::Steinberg::kResultOk);
}

#[test]
fn bad_class_id_gives_error() {
    const NAME: &str = "test";
    const CID: [u8; 16] = [4; 16];
    let wrapper = Factory::new(
        &[&SynthClass {
            info: ClassInfo {
                name: NAME,
                cid: CID,
                edit_controller_cid: [5; 16],
                ui_initial_size: crate::UiSize {
                    width: 800,
                    height: 400,
                },
            },
            factory: &|_: &HostInfo| DummyComponent {},
        }],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let cid: [std::ffi::c_char; 16] = [0; 16];
    let result =
        unsafe { wrapper.createInstance(cid.as_ptr(), cid.as_ptr(), std::ptr::null_mut()) };
    assert_ne!(result, vst3::Steinberg::kResultOk);
}

#[test]
fn get_edit_controller_class_id() {
    const NAME: &str = "test";
    const CID: [u8; 16] = [4; 16];
    const EDIT_CONTROLLER_CID: [u8; 16] = [5; 16];
    let wrapper = Factory::new(
        &[&SynthClass {
            info: ClassInfo {
                name: NAME,
                cid: CID,
                edit_controller_cid: EDIT_CONTROLLER_CID,
                ui_initial_size: crate::UiSize {
                    width: 800,
                    height: 400,
                },
            },
            factory: &|_: &HostInfo| DummyComponent {},
        }],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let cid: [std::ffi::c_char; 16] = [4; 16];
    let mut raw_result: *mut std::ffi::c_void = std::ptr::null_mut();
    let result = unsafe {
        wrapper.createInstance(
            cid.as_ptr(),
            IComponent::IID.as_ptr().cast(),
            &mut raw_result,
        )
    };
    assert_eq!(result, vst3::Steinberg::kResultOk);
    let result: ComPtr<IComponent> = unsafe { ComPtr::from_raw(raw_result.cast()) }.unwrap();
    let mut result_edit_controller_cid = [0i8; 16];
    assert_eq!(
        unsafe { result.getControllerClassId(&mut result_edit_controller_cid) },
        vst3::Steinberg::kResultOk
    );
    assert!(
        EDIT_CONTROLLER_CID
            .iter()
            .zip(result_edit_controller_cid.iter())
            .all(|(a, b)| *a == *b as u8)
    );
}

#[test]
fn defends_against_bad_index_class_info_2() {
    let wrapper = Factory::new(
        &[],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "1.0.0",
        },
    );
    let result = unsafe { wrapper.getClassInfo2(0, std::ptr::null_mut()) };
    assert_ne!(result, vst3::Steinberg::kResultOk);
}

#[test]
#[should_panic]
fn defends_against_too_long_version() {
    let _ = Factory::new(
        &[],
        Info {
            vendor: "test",
            url: "https://example.com",
            email: "awesome@example.com",
            version: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
    );
}

#[test]
fn basic_class_info_2_test() {
    const NAME: &str = "test";
    const CID: [u8; 16] = [4; 16];
    const EDIT_CONTROLLER_CID: [u8; 16] = [5; 16];
    const VENDOR: &str = "test";
    const VERSION: &str = "1.0.0";
    let wrapper = Factory::new(
        &[&SynthClass {
            info: ClassInfo {
                name: NAME,
                cid: CID,
                edit_controller_cid: EDIT_CONTROLLER_CID,
                ui_initial_size: crate::UiSize {
                    width: 800,
                    height: 400,
                },
            },
            factory: &|_: &HostInfo| DummyComponent {},
        }],
        Info {
            vendor: VENDOR,
            url: "https://example.com",
            email: "awesome@example.com",
            version: VERSION,
        },
    );
    let class_count = unsafe { wrapper.countClasses() };
    assert_eq!(class_count, 2);
    let mut info = vst3::Steinberg::PClassInfo2 {
        cid: [0; 16],
        cardinality: 0,
        category: [0; 32],
        name: [0; 64],
        classFlags: 0,
        subCategories: [0; 128],
        vendor: [0; 64],
        version: [0; 64],
        sdkVersion: [0; 64],
    };
    unsafe {
        assert_eq!(
            wrapper.getClassInfo2(0, &mut info),
            vst3::Steinberg::kResultOk
        );
    }
    assert_eq!(from_cstr(&info.category).as_str(), "Audio Module Class");
    assert_eq!(from_cstr(&info.subCategories).as_str(), "Instrument|Synth");
    assert_eq!(from_cstr(&info.name).as_str(), NAME);
    assert_eq!(
        info.cardinality,
        vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as i32
    );
    unsafe {
        assert_eq!(
            from_cstr(&info.sdkVersion).as_str(),
            std::ffi::CStr::from_ptr(vst3::Steinberg::Vst::SDKVersionString)
                .to_str()
                .unwrap()
        );
    }
    assert!(info.cid.iter().zip(CID.iter()).all(|(a, b)| *a == *b as i8));
    assert!(from_cstr(&info.vendor).as_str() == VENDOR);
    assert!(from_cstr(&info.version).as_str() == VERSION);

    unsafe {
        assert_eq!(
            wrapper.getClassInfo2(1, &mut info),
            vst3::Steinberg::kResultOk
        );
    }
    assert!(
        info.cid
            .iter()
            .zip(EDIT_CONTROLLER_CID.iter())
            .all(|(a, b)| *a == *b as i8)
    );
    assert_eq!(
        from_cstr(&info.category).as_str(),
        "Component Controller Class"
    );
    assert_eq!(
        from_cstr(&info.name).as_str(),
        (NAME.to_string() + "EC").as_str()
    );
}
