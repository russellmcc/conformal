use crate::HostInfo;
use vst3::Steinberg::Vst::IHostApplicationTrait;

use super::from_utf16_buffer;

fn get_name(host: &dyn IHostApplicationTrait) -> Option<String> {
    let mut name_buffer = [0i16; 128];
    let res = unsafe { host.getName(&raw mut name_buffer) };
    if res != vst3::Steinberg::kResultOk {
        return None;
    }

    from_utf16_buffer(&name_buffer)
}

/// Extract the host info from `IHostApplication`.  Note that this is
/// potentially re-entrant!
pub fn get(host: &dyn IHostApplicationTrait) -> Option<HostInfo> {
    let name = get_name(host)?;
    Some(HostInfo { name })
}
