use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::NSError;

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

pub const ENABLED: isize = 1;
pub const REQUIRES_APPROVAL: isize = 2;

fn service() -> Retained<AnyObject> {
    // SAFETY: ServiceManagement is linked above; mainAppService returns a non-null
    // SMAppService on macOS 13+, Soundy's minimum supported version.
    unsafe { msg_send![class!(SMAppService), mainAppService] }
}

pub fn status() -> isize {
    // SAFETY: SMAppService.status is an NSInteger property with no side effects.
    unsafe { msg_send![&*service(), status] }
}

pub fn set_enabled(enabled: bool) -> Result<(), Retained<NSError>> {
    // SAFETY: Both selectors take NSError** and return BOOL. msg_send! manages
    // the autoreleased error and converts failure to an owned NSError.
    unsafe {
        if enabled {
            msg_send![&*service(), registerAndReturnError: _]
        } else {
            msg_send![&*service(), unregisterAndReturnError: _]
        }
    }
}

pub fn open_settings() {
    // SAFETY: This class method opens the native Login Items settings on macOS 13+.
    unsafe { msg_send![class!(SMAppService), openSystemSettingsLoginItems] }
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod tests;
