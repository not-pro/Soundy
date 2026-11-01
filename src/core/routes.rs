//! Audio routes: which physical output Soundy answers ("always", "headphones
//! only", "speakers only"), classified from the CoreAudio default output device.
//!
//! The objc2 crates do not wrap CoreAudio's property API, so the handful of
//! functions used here are declared against the system framework directly.

use std::os::raw::c_void;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    Headphones,
    Speakers,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Always,
    Headphones,
    Speakers,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Always, Self::Headphones, Self::Speakers];

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Always)
    }

    /// Whether a press may sound on a device of this class. An unknown device
    /// never mutes: silence from a classification bug is worse than a leak.
    pub fn allows(self, class: DeviceClass) -> bool {
        match self {
            Self::Always => true,
            Self::Headphones => class != DeviceClass::Speakers,
            Self::Speakers => class != DeviceClass::Headphones,
        }
    }
}

// Four-character codes and object IDs from CoreAudio/AudioHardware.h.
const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const SCOPE_OUTPUT: u32 = u32::from_be_bytes(*b"outp");
const PROP_DEFAULT_OUTPUT: u32 = u32::from_be_bytes(*b"dOut");
const PROP_TRANSPORT: u32 = u32::from_be_bytes(*b"tran");
const PROP_DATA_SOURCE: u32 = u32::from_be_bytes(*b"ssrc");

const TRANSPORT_BUILT_IN: u32 = u32::from_be_bytes(*b"bltn");
const TRANSPORT_BLUETOOTH: u32 = u32::from_be_bytes(*b"blue");
const TRANSPORT_BLUETOOTH_LE: u32 = u32::from_be_bytes(*b"blea");
const TRANSPORT_USB: u32 = u32::from_be_bytes(*b"usb ");
const TRANSPORT_HDMI: u32 = u32::from_be_bytes(*b"hdmi");
const TRANSPORT_THUNDERBOLT: u32 = u32::from_be_bytes(*b"thun");
const TRANSPORT_DISPLAY_PORT: u32 = u32::from_be_bytes(*b"dprt");
const TRANSPORT_PCI: u32 = u32::from_be_bytes(*b"pci ");
const TRANSPORT_FIREWIRE: u32 = u32::from_be_bytes(*b"fire");
const TRANSPORT_AIRPLAY: u32 = u32::from_be_bytes(*b"airp");
const TRANSPORT_VIRTUAL: u32 = u32::from_be_bytes(*b"virt");

// Runtime data-source IDs (not exposed as public constants, but stable):
// 'hdpn' is the headphone jack, 'ispk' the internal speaker.
const SOURCE_HEADPHONES: u32 = u32::from_be_bytes(*b"hdpn");
const SOURCE_INTERNAL_SPEAKER: u32 = u32::from_be_bytes(*b"ispk");

/// Pure classification rule, split out for tests.
fn classify(transport: Option<u32>, data_source: Option<u32>) -> DeviceClass {
    match transport {
        Some(TRANSPORT_BLUETOOTH | TRANSPORT_BLUETOOTH_LE) => DeviceClass::Headphones,
        Some(TRANSPORT_BUILT_IN) => match data_source {
            Some(SOURCE_HEADPHONES) => DeviceClass::Headphones,
            Some(SOURCE_INTERNAL_SPEAKER) => DeviceClass::Speakers,
            _ => DeviceClass::Unknown,
        },
        Some(
            TRANSPORT_USB
            | TRANSPORT_HDMI
            | TRANSPORT_THUNDERBOLT
            | TRANSPORT_DISPLAY_PORT
            | TRANSPORT_PCI
            | TRANSPORT_FIREWIRE
            | TRANSPORT_AIRPLAY,
        ) => DeviceClass::Speakers,
        Some(TRANSPORT_VIRTUAL) => DeviceClass::Unknown,
        _ => DeviceClass::Unknown,
    }
}

#[repr(C)]
pub struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

const ELEMENT_MAIN: u32 = 0;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: *mut u32,
        data: *mut c_void,
    ) -> i32;
    fn AudioObjectAddPropertyListener(
        object: u32,
        address: *const PropertyAddress,
        listener: extern "C" fn(u32, u32, *const PropertyAddress, *mut c_void),
        client_data: *mut c_void,
    ) -> i32;
    fn AudioObjectRemovePropertyListener(
        object: u32,
        address: *const PropertyAddress,
        listener: extern "C" fn(u32, u32, *const PropertyAddress, *mut c_void),
        client_data: *mut c_void,
    ) -> i32;
}

fn read_property<T>(object: u32, selector: u32, scope: u32) -> Option<T> {
    let address = PropertyAddress {
        selector,
        scope,
        element: ELEMENT_MAIN,
    };
    let mut size = u32::try_from(std::mem::size_of::<T>()).ok()?;
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    // SAFETY: address is a valid stack property address; size matches the out
    // buffer exactly, which CoreAudio fills on success (status 0).
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            value.as_mut_ptr().cast(),
        )
    };
    (status == 0 && size as usize == std::mem::size_of::<T>())
        .then(|| unsafe { value.assume_init() })
}

pub fn default_output_device() -> Option<u32> {
    read_property(
        K_AUDIO_OBJECT_SYSTEM_OBJECT,
        PROP_DEFAULT_OUTPUT,
        SCOPE_GLOBAL,
    )
}

fn device_class(device: u32) -> DeviceClass {
    let transport = read_property::<u32>(device, PROP_TRANSPORT, SCOPE_GLOBAL);
    let data_source = read_property::<u32>(device, PROP_DATA_SOURCE, SCOPE_OUTPUT);
    classify(transport, data_source)
}

pub fn current_output_class() -> DeviceClass {
    default_output_device().map_or(DeviceClass::Unknown, device_class)
}

/// Watches the default output device and fires `listener` (on an arbitrary
/// CoreAudio thread) whenever it changes. The listener must dispatch its work
/// to the main thread itself.
pub struct OutputListener {
    client_data: *mut c_void,
    listener: extern "C" fn(u32, u32, *const PropertyAddress, *mut c_void),
}

impl OutputListener {
    pub fn add(
        client_data: *mut c_void,
        listener: extern "C" fn(u32, u32, *const PropertyAddress, *mut c_void),
    ) -> Option<Self> {
        let address = PropertyAddress {
            selector: PROP_DEFAULT_OUTPUT,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        // SAFETY: address outlives the call; listener is a plain function pointer;
        // client_data is opaque to CoreAudio and must stay valid until remove().
        let status = unsafe {
            AudioObjectAddPropertyListener(
                K_AUDIO_OBJECT_SYSTEM_OBJECT,
                &address,
                listener,
                client_data,
            )
        };
        (status == 0).then_some(Self {
            client_data,
            listener,
        })
    }
}

impl Drop for OutputListener {
    fn drop(&mut self) {
        let address = PropertyAddress {
            selector: PROP_DEFAULT_OUTPUT,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        // SAFETY: mirrors the successful add call with the same listener and data.
        unsafe {
            AudioObjectRemovePropertyListener(
                K_AUDIO_OBJECT_SYSTEM_OBJECT,
                &address,
                self.listener,
                self.client_data,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_covers_the_transports_soundy_cares_about() {
        assert_eq!(
            classify(Some(TRANSPORT_BLUETOOTH), None),
            DeviceClass::Headphones
        );
        assert_eq!(
            classify(Some(TRANSPORT_BLUETOOTH_LE), None),
            DeviceClass::Headphones
        );
        assert_eq!(
            classify(Some(TRANSPORT_BUILT_IN), Some(SOURCE_HEADPHONES)),
            DeviceClass::Headphones
        );
        assert_eq!(
            classify(Some(TRANSPORT_BUILT_IN), Some(SOURCE_INTERNAL_SPEAKER)),
            DeviceClass::Speakers
        );
        assert_eq!(
            classify(Some(TRANSPORT_BUILT_IN), None),
            DeviceClass::Unknown
        );
        assert_eq!(classify(Some(TRANSPORT_USB), None), DeviceClass::Speakers);
        assert_eq!(
            classify(Some(TRANSPORT_AIRPLAY), None),
            DeviceClass::Speakers
        );
        assert_eq!(
            classify(Some(TRANSPORT_VIRTUAL), None),
            DeviceClass::Unknown
        );
        assert_eq!(classify(None, None), DeviceClass::Unknown);
    }

    #[test]
    fn route_modes_gate_only_when_the_class_is_certain() {
        assert!(Mode::Always.allows(DeviceClass::Unknown));
        assert!(Mode::Headphones.allows(DeviceClass::Headphones));
        assert!(!Mode::Headphones.allows(DeviceClass::Speakers));
        assert!(Mode::Headphones.allows(DeviceClass::Unknown));
        assert!(Mode::Speakers.allows(DeviceClass::Speakers));
        assert!(!Mode::Speakers.allows(DeviceClass::Headphones));
        assert!(Mode::Speakers.allows(DeviceClass::Unknown));
    }
}
