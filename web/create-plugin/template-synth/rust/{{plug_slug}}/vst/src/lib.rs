use {{plug_slug}}_component::Component;

use conformal_vst_wrapper::{ClassID, ClassInfoBuilder, HostInfo, Info, SynthClass};

const CID: ClassID = [
    {{class_id}}
];
const EDIT_CONTROLLER_CID: ClassID = [
    {{edit_class_id}}
];

conformal_vst_wrapper::wrap_factory!(
    &const {
        [&SynthClass {
            info: ClassInfoBuilder::new(
                "{{plug_name}}",
                CID,
                EDIT_CONTROLLER_CID,
                conformal_vst_wrapper::UiSize {
                    width: 400,
                    height: 400,
                },
            )
            .build(),
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
