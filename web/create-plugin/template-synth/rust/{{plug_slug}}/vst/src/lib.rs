use {{plug_slug}}_component::Component;

use conformal_vst_wrapper::{ClassID, ClassInfo, HostInfo, Info, SynthClass};

const CID: ClassID = [
    {{class_id}}
];
const EDIT_CONTROLLER_CID: ClassID = [
    {{edit_class_id}}
];

conformal_vst_wrapper::wrap_factory!(
    &const {
        [&SynthClass {
            info: ClassInfo {
                name: "{{plug_name}}",
                cid: CID,
                edit_controller_cid: EDIT_CONTROLLER_CID,
                ui_initial_size: conformal_vst_wrapper::UiSize {
                    width: 400,
                    height: 400,
                },
            },
            factory: |_: &HostInfo| -> Component { Default::default() },
        }]
    },
    Info {
        vendor: "{{vendor_name}}",
        url: "{{task_marker}} add URL",
        email: "test@example.com",
        version: "1.0.0",
    }
);
