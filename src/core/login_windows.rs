//! Windows launch-at-login: one value under the per-user `Run` key, which is
//! what Task Manager's Startup tab shows and what macOS calls a login item.

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};

use windows::core::{PCWSTR, w};

pub const ENABLED: isize = 1;
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("Soundy");

pub fn status() -> isize {
    // A value pointing at another copy (moved or reinstalled) does not start this
    // app, so it is not reported as enabled and gets rewritten on the next launch.
    match (value(), command()) {
        (Some(saved), Some(ours)) if saved.eq_ignore_ascii_case(&ours) => ENABLED,
        _ => 0,
    }
}

/// The `Run` value Soundy writes: the quoted path of this executable.
fn command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\"", exe.display()))
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let key = open(KEY_SET_VALUE | KEY_QUERY_VALUE)?;
    let result = unsafe {
        if enabled {
            let Some(command) = command() else {
                return Err("Soundy cannot resolve its own path".into());
            };
            let bytes: Vec<u8> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
            RegSetValueExW(key, VALUE, Some(0), REG_SZ, Some(&bytes))
        } else {
            RegDeleteValueW(key, VALUE)
        }
    };
    // SAFETY: The handle came from RegOpenKeyExW in this function and is closed once.
    unsafe {
        let _ = RegCloseKey(key);
    };
    match result {
        ERROR_SUCCESS => Ok(()),
        // Removing a value that is not there already leaves us in the wanted state.
        error if !enabled && error == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND => Ok(()),
        error => Err(format!("cannot update the Run key: {error:?}")),
    }
}

fn value() -> Option<String> {
    let key = open(KEY_QUERY_VALUE).ok()?;
    let mut kind = REG_SZ;
    let mut buffer = [0u16; 512];
    let mut size = (buffer.len() * size_of::<u16>()) as u32;
    // SAFETY: `buffer` and `size` are valid for the call; the value is a REG_SZ
    // string, so the bytes are UTF-16 in host order.
    let result = unsafe {
        RegQueryValueExW(
            key,
            VALUE,
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    // SAFETY: The handle came from RegOpenKeyExW in this function and is closed once.
    unsafe {
        let _ = RegCloseKey(key);
    };
    if result != ERROR_SUCCESS {
        return None;
    }
    let units = (size as usize / size_of::<u16>()).min(buffer.len());
    let text: String = String::from_utf16_lossy(&buffer[..units]);
    Some(text.trim_end_matches('\0').to_owned())
}

fn open(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Result<HKEY, String> {
    let mut key = HKEY::default();
    // SAFETY: The key exists on every Windows install and `key` is a valid out pointer.
    let result = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), access, &mut key) };
    if result == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(format!("cannot open the Run key: {result:?}"))
    }
}
