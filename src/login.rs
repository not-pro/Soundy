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
mod tests {
    use super::*;

    #[test]
    #[ignore = "run from a signed test app bundle with a unique bundle identifier"]
    fn login_item_registers_and_unregisters() {
        assert!(
            matches!(status(), 0 | 3),
            "the test bundle must start unregistered"
        );
        let registration = set_enabled(true);
        let registered_status = status();
        // Unregister before asserting so a failed check leaves no enabled test item.
        set_enabled(false).expect("unregister the test login item");
        registration.expect("register the signed test bundle");
        assert!(matches!(registered_status, ENABLED | REQUIRES_APPROVAL));
        assert_eq!(status(), 0);
    }
}
