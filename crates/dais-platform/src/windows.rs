//! Windows monitor management via `Win32` API.
#![allow(unsafe_code)]

use dais_core::monitor::{MonitorInfo, MonitorManager};
use std::mem;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::core::{BOOL, PCWSTR};

/// Windows implementation of monitor enumeration.
pub struct WindowsMonitorManager;

impl WindowsMonitorManager {
    /// Create a Windows monitor manager.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsMonitorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorManager for WindowsMonitorManager {
    #[allow(unsafe_code)]
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        let mut monitors = Vec::new();

        // SAFETY: `monitors` lives for the duration of the callback enumeration.
        // We pass a raw pointer via LPARAM and reconstruct it in the callback.
        let monitors_ptr: *mut Vec<MonitorInfo> = &raw mut monitors;
        unsafe {
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(monitor_enum_callback),
                LPARAM(monitors_ptr as isize),
            );
        }

        monitors
    }
}

/// Callback invoked by `EnumDisplayMonitors` for each connected monitor.
///
/// # Safety
///
/// `lparam` must point to a valid, mutable `Vec<MonitorInfo>`.
#[allow(clippy::cast_sign_loss)]
unsafe extern "system" fn monitor_enum_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // SAFETY: Caller guarantees lparam points to our Vec.
    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };

    // SAFETY: MONITORINFOEXW is a plain-data struct, zeroing is valid.
    let mut info: MONITORINFOEXW = unsafe { mem::zeroed() };
    #[allow(clippy::cast_possible_truncation)]
    let cb_size = mem::size_of::<MONITORINFOEXW>() as u32;
    info.monitorInfo.cbSize = cb_size;

    // SAFETY: info is properly initialized with cbSize set.
    if !unsafe { GetMonitorInfoW(hmonitor, (&raw mut info).cast()) }.as_bool() {
        return BOOL(1); // skip this monitor, continue enumeration
    }

    let rc = info.monitorInfo.rcMonitor;
    let rc_work = info.monitorInfo.rcWork;
    let width = (rc.right - rc.left) as u32;
    let height = (rc.bottom - rc.top) as u32;
    let work_width = (rc_work.right - rc_work.left) as u32;
    let work_height = (rc_work.bottom - rc_work.top) as u32;
    let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY

    // Get effective DPI
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    // SAFETY: dpi_x and dpi_y are valid mutable u32s.
    let _ =
        unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    let scale_factor = f64::from(dpi_x) / 96.0;

    // Device name from MONITORINFOEXW (null-terminated UTF-16)
    let device_name = wstr_to_string(&info.szDevice);

    // Try to resolve a friendly/human-readable name
    // SAFETY: szDevice is a valid null-terminated UTF-16 buffer from GetMonitorInfoW.
    let friendly_name = unsafe { get_friendly_name(&info.szDevice) };
    let name = friendly_name.unwrap_or_else(|| device_name.clone());

    let id = format!("{device_name}@{}x{}", rc.left, rc.top);

    monitors.push(MonitorInfo {
        id,
        name,
        position: (rc.left, rc.top),
        size: (width, height),
        work_area: (rc_work.left, rc_work.top, work_width, work_height),
        scale_factor,
        is_primary,
    });

    BOOL(1) // continue enumeration
}

/// Query `EnumDisplayDevicesW` for a human-readable monitor name.
///
/// # Safety
///
/// `device_name` must be a valid null-terminated UTF-16 string in a `[u16; 32]` buffer.
unsafe fn get_friendly_name(device_name: &[u16; 32]) -> Option<String> {
    // SAFETY: DISPLAY_DEVICEW is a plain-data struct, zeroing is valid.
    let mut display_device: DISPLAY_DEVICEW = unsafe { mem::zeroed() };
    #[allow(clippy::cast_possible_truncation)]
    let cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;
    display_device.cb = cb;

    // SAFETY: device_name is a valid null-terminated UTF-16 buffer.
    let device_name_pcwstr = PCWSTR::from_raw(device_name.as_ptr());
    // SAFETY: display_device is properly initialized with cb set.
    if !unsafe { EnumDisplayDevicesW(device_name_pcwstr, 0, &raw mut display_device, 1) }.as_bool()
    {
        return None;
    }

    let name = wstr_to_string(&display_device.DeviceString);
    if name.is_empty() { None } else { Some(name) }
}

/// Convert a fixed-size null-terminated UTF-16 buffer to a `String`.
fn wstr_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_at_least_one_monitor() {
        let mgr = WindowsMonitorManager::new();
        let monitors = mgr.available_monitors();
        assert!(!monitors.is_empty(), "should detect at least one monitor");
    }

    #[test]
    fn has_one_primary() {
        let mgr = WindowsMonitorManager::new();
        let monitors = mgr.available_monitors();
        let primary_count = monitors.iter().filter(|m| m.is_primary).count();
        assert_eq!(primary_count, 1, "exactly one monitor should be primary");
    }

    #[test]
    fn monitor_has_nonzero_size() {
        let mgr = WindowsMonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(m.size.0 > 0, "monitor width should be > 0: {}", m.name);
            assert!(m.size.1 > 0, "monitor height should be > 0: {}", m.name);
        }
    }

    #[test]
    fn scale_factor_is_reasonable() {
        let mgr = WindowsMonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(
                (0.5..=4.0).contains(&m.scale_factor),
                "scale factor {} out of expected range for {}",
                m.scale_factor,
                m.name
            );
        }
    }

    #[test]
    fn primary_monitor_helper_works() {
        let mgr = WindowsMonitorManager::new();
        let primary = mgr.primary_monitor();
        assert!(primary.is_some(), "primary_monitor() should return Some");
        assert!(primary.unwrap().is_primary);
    }

    #[test]
    fn logical_size_computation() {
        let info = MonitorInfo {
            id: "test".into(),
            name: "Test".into(),
            position: (0, 0),
            size: (3840, 2160),
            work_area: (0, 0, 3840, 2080),
            scale_factor: 2.0,
            is_primary: true,
        };
        let (lw, lh) = info.logical_size();
        assert!((lw - 1920.0).abs() < f64::EPSILON);
        assert!((lh - 1080.0).abs() < f64::EPSILON);
    }
}
