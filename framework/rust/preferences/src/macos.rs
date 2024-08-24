use objc::{runtime::Object, sel, sel_impl};
use std::ffi::{CStr, CString};
use std::str;

struct Store {
    domain: String,
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

unsafe fn autoreleased_nsstring(s: &str) -> *mut Object {
    let nsstring = objc::class!(NSString);
    let cstring = CString::new(s).expect("Strings cannot contain null bytes");
    objc::msg_send![nsstring, stringWithUTF8String: cstring.as_ptr()]
}

unsafe fn with_user_defaults<F: FnOnce(*mut Object)>(f: F, domain: &str) {
    let user_defaults_class = objc::class!(NSUserDefaults);
    let nsstring = objc::class!(NSString);

    let domain_string: *mut Object =
        objc::msg_send![nsstring, stringWithUTF8String: domain.as_ptr()];
    let user_defaults_alloc: *mut Object = objc::msg_send![user_defaults_class, alloc];
    let user_defaults: *mut Object =
        objc::msg_send![user_defaults_alloc, initWithSuiteName: domain_string];
    f(user_defaults);
    let _: () = objc::msg_send![user_defaults, release];
}

impl super::OSStore for Store {
    fn get(&self, unique_id: &str) -> Option<super::Value> {
        unsafe {
            let mut ret = None;
            with_user_defaults(
                |user_defaults| {
                    let key = autoreleased_nsstring(unique_id);
                    let value: *mut Object = objc::msg_send![user_defaults, stringForKey: key];
                    if value.is_null() {
                        return;
                    }

                    if let Ok(s) = CStr::from_ptr(objc::msg_send!(value, UTF8String)).to_str() {
                        if let Ok(v) = serde_json::from_str(s) {
                            ret = Some(v);
                        }
                    }
                },
                self.domain.as_str(),
            );
            ret
        }
    }

    fn set(&mut self, unique_id: &str, value: super::Value) {
        unsafe {
            with_user_defaults(
                |user_defaults| {
                    if let Ok(s) = serde_json::to_string(&value) {
                        let key = autoreleased_nsstring(unique_id);
                        let value = autoreleased_nsstring(&s);
                        let _: () = objc::msg_send![user_defaults, setObject: value forKey: key];
                    }
                },
                self.domain.as_str(),
            );
        }
    }

    #[cfg(all(test, not(miri)))]
    fn reset(&mut self) {
        unsafe {
            with_user_defaults(
                |user_defaults| {
                    let dictionary: *mut Object =
                        objc::msg_send![user_defaults, dictionaryRepresentation];
                    let count: std::ffi::c_ulong = objc::msg_send![dictionary, count];
                    let keys: *mut Object = objc::msg_send![dictionary, allKeys];

                    for i in 0..count {
                        let key: *mut Object = objc::msg_send![keys, objectAtIndex: i];
                        let _: () = objc::msg_send![user_defaults, removeObjectForKey: key];
                    }
                },
                self.domain.as_str(),
            );
        };
    }
}

pub fn create_os_store(domain: &str) -> impl super::OSStore {
    Store {
        domain: domain.to_string(),
    }
}
