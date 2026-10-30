use objc2::{
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSBundle, NSError, NSObject, NSString, NSURL};

// Sparkle is loaded from the signed application bundle, so ordinary Cargo builds
// and unit tests do not require a separately installed framework.
pub struct Updater {
    controller: Retained<AnyObject>,
}

impl Updater {
    pub fn start(delegate: &NSObject, test_feed: Option<&str>) -> Result<Self, String> {
        let frameworks = NSBundle::mainBundle()
            .privateFrameworksPath()
            .ok_or("Soundy must run from its application bundle to check for updates")?;
        let path = NSString::from_str(&format!("{frameworks}/Sparkle.framework"));
        let framework = NSBundle::bundleWithPath(&path).ok_or("Sparkle.framework is missing")?;
        // SAFETY: Only the Sparkle framework embedded in our signed app is loaded.
        // These selectors and signatures are from the pinned Sparkle 2 headers.
        unsafe {
            framework.loadAndReturnError().map_err(|e| e.to_string())?;
            let class = AnyClass::get(c"SPUStandardUpdaterController")
                .ok_or("Sparkle updater class is unavailable")?;
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            let controller: Retained<AnyObject> = msg_send![allocated,
                initWithStartingUpdater: false,
                updaterDelegate: delegate,
                userDriverDelegate: Option::<&AnyObject>::None,
            ];
            let updater: Retained<AnyObject> = msg_send![&*controller, updater];
            if let Some(feed) = test_feed {
                let url = NSURL::URLWithString(&NSString::from_str(feed))
                    .ok_or("Invalid smoke-test feed URL")?;
                let _: () = msg_send![&*updater, setFeedURL: &*url];
            }
            let result: Result<(), Retained<NSError>> = msg_send![&*updater, startUpdater: _];
            result.map_err(|e| e.to_string())?;
            // This information-only check runs on EVERY normal launch. It does
            // not display Sparkle's update/permission dialogs or download a ZIP.
            let _: () = msg_send![&*updater, checkForUpdateInformation];
            Ok(Self { controller })
        }
    }

    pub fn install(&self) {
        // SAFETY: This is Sparkle's standard user-initiated check/install action.
        // It is called only after a click on Soundy's available-update menu item.
        unsafe {
            let _: () = msg_send![&*self.controller, checkForUpdates: Option::<&AnyObject>::None];
        }
    }
}
