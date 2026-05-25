use anyhow::{Context, Result};
use sysinfo::System;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowPos, HWND_TOP, SM_CXSCREEN, SM_CYSCREEN, SWP_NOZORDER, SWP_SHOWWINDOW,
};

use crate::asgard::profile::{LayoutPreset, WindowLayout};

const MONITORINFOF_PRIMARY: u32 = 0x0000_0001;

fn enable_per_monitor_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

struct WindowSearch {
    target_exe: String,
    found_hwnd: Option<HWND>,
    sys: System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl WorkArea {
    fn width(self) -> i32 {
        self.right - self.left
    }

    fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayMonitor {
    pub index: u32,
    pub name: String,
    pub device_name: String,
    pub is_primary: bool,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorWorkArea {
    work_area: WorkArea,
    name: String,
    device_name: String,
    is_primary: bool,
}

unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);

    // Only care about visible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1); // continue
    }

    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return BOOL(1);
    }

    search.sys.refresh_processes();
    if let Some(process) = search.sys.process(sysinfo::Pid::from_u32(pid)) {
        let exe_name = process.name().to_string().to_lowercase();
        let target_lower = search.target_exe.to_lowercase();
        let target_with_ext = format!("{}.exe", target_lower);

        if exe_name == target_lower || exe_name == target_with_ext {
            search.found_hwnd = Some(hwnd);
            return BOOL(0); // stop enumerating
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        if len > 0 {
            let title = String::from_utf16_lossy(&title_buf[..len as usize]).to_lowercase();
            if title.contains(&target_lower) {
                search.found_hwnd = Some(hwnd);
                return BOOL(0); // stop enumerating
            }
        }
    }

    BOOL(1) // continue
}

unsafe extern "system" fn enum_monitor_proc(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorWorkArea>);
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
        let device_name = utf16_z_to_string(&info.szDevice);
        let name = monitor_display_name(&info.szDevice).unwrap_or_else(|| device_name.clone());
        monitors.push(MonitorWorkArea {
            work_area: WorkArea {
                left: info.monitorInfo.rcWork.left,
                top: info.monitorInfo.rcWork.top,
                right: info.monitorInfo.rcWork.right,
                bottom: info.monitorInfo.rcWork.bottom,
            },
            name,
            device_name,
            is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
    }

    BOOL(1)
}

fn monitor_display_name(device_name: &[u16; 32]) -> Option<String> {
    let mut display: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    display.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let ok = unsafe {
        EnumDisplayDevicesW(
            PCWSTR(device_name.as_ptr()),
            0,
            &mut display as *mut DISPLAY_DEVICEW,
            0,
        )
        .as_bool()
    };
    if !ok {
        return None;
    }

    let name = utf16_z_to_string(&display.DeviceString);
    if name.trim().is_empty() {
        None
    } else {
        Some(name)
    }
}

fn utf16_z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

pub fn list_display_monitors() -> Result<Vec<DisplayMonitor>> {
    enable_per_monitor_dpi_awareness();
    let monitors = ordered_monitors()?;
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(i, monitor)| DisplayMonitor {
            index: (i + 1) as u32,
            name: monitor.name.clone(),
            device_name: monitor.device_name.clone(),
            is_primary: monitor.is_primary,
            left: monitor.work_area.left,
            top: monitor.work_area.top,
            width: monitor.work_area.width(),
            height: monitor.work_area.height(),
        })
        .collect())
}

fn ordered_monitors() -> Result<Vec<MonitorWorkArea>> {
    let mut monitors = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            HDC(0),
            None,
            Some(enum_monitor_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        )
        .ok()
        .with_context(|| "failed to enumerate display monitors")?;
    }

    if monitors.is_empty() {
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        monitors.push(MonitorWorkArea {
            work_area: WorkArea {
                left: 0,
                top: 0,
                right: screen_w,
                bottom: screen_h,
            },
            name: "Primary display".to_string(),
            device_name: String::new(),
            is_primary: true,
        });
    }

    monitors.sort_by_key(|monitor| {
        (
            display_device_number(&monitor.device_name).unwrap_or(u32::MAX),
            monitor.work_area.left,
            monitor.work_area.top,
        )
    });
    Ok(monitors)
}

fn display_device_number(device_name: &str) -> Option<u32> {
    device_name
        .rsplit_once("DISPLAY")
        .and_then(|(_, n)| n.parse::<u32>().ok())
}

pub fn apply_layout(exe_name: &str, layout: &WindowLayout) -> Result<()> {
    enable_per_monitor_dpi_awareness();
    let mut search = WindowSearch {
        target_exe: exe_name.to_string(),
        found_hwnd: None,
        sys: System::new_all(),
    };

    unsafe {
        EnumWindows(
            Some(enum_window_proc),
            LPARAM(&mut search as *mut _ as isize),
        )
        .ok();
    }

    let hwnd = match search.found_hwnd {
        Some(h) => h,
        None => anyhow::bail!(
            "could not find visible window for executable '{}'",
            exe_name
        ),
    };

    let monitors = ordered_monitors()?;
    let (x, y, w, h) = resolve_layout(layout, &monitors);

    unsafe {
        SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW)
            .with_context(|| "failed to set window position")?;
    }

    Ok(())
}

fn resolve_layout(layout: &WindowLayout, monitors: &[MonitorWorkArea]) -> (i32, i32, i32, i32) {
    match layout {
        WindowLayout::Bounds {
            x,
            y,
            width,
            height,
        } => (*x, *y, *width, *height),
        WindowLayout::Preset(preset) => {
            let monitor = selected_monitor(monitors, 1);
            resolve_preset(*preset, monitor.work_area)
        }
        WindowLayout::TargetedPreset { preset, monitor } => {
            let monitor = selected_monitor(monitors, *monitor);
            resolve_preset(*preset, monitor.work_area)
        }
    }
}

fn selected_monitor(monitors: &[MonitorWorkArea], monitor_index: u32) -> MonitorWorkArea {
    let index = monitor_index.max(1) as usize - 1;
    monitors
        .get(index)
        .cloned()
        .or_else(|| monitors.first().cloned())
        .unwrap_or(MonitorWorkArea {
            work_area: WorkArea {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            name: "Primary display".to_string(),
            device_name: String::new(),
            is_primary: true,
        })
}

fn resolve_preset(preset: LayoutPreset, area: WorkArea) -> (i32, i32, i32, i32) {
    let x = area.left;
    let y = area.top;
    let w = area.width();
    let h = area.height();
    let half_w = w / 2;
    let half_h = h / 2;

    match preset {
        LayoutPreset::SnapLeft => (x, y, half_w, h),
        LayoutPreset::SnapRight => (x + half_w, y, w - half_w, h),
        LayoutPreset::TopHalf => (x, y, w, half_h),
        LayoutPreset::BottomHalf => (x, y + half_h, w, h - half_h),
        LayoutPreset::Quadrant1 => (x + half_w, y, w - half_w, half_h),
        LayoutPreset::Quadrant2 => (x, y, half_w, half_h),
        LayoutPreset::Quadrant3 => (x, y + half_h, half_w, h - half_h),
        LayoutPreset::Quadrant4 => (x + half_w, y + half_h, w - half_w, h - half_h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitors() -> Vec<MonitorWorkArea> {
        vec![
            MonitorWorkArea {
                work_area: WorkArea {
                    left: 1920,
                    top: 0,
                    right: 4480,
                    bottom: 1440,
                },
                name: "External display".to_string(),
                device_name: r"\\.\DISPLAY1".to_string(),
                is_primary: false,
            },
            MonitorWorkArea {
                work_area: WorkArea {
                    left: 0,
                    top: 40,
                    right: 1920,
                    bottom: 1080,
                },
                name: "Primary display".to_string(),
                device_name: r"\\.\DISPLAY2".to_string(),
                is_primary: true,
            },
        ]
    }

    #[test]
    fn legacy_preset_uses_windows_display_one_work_area() {
        let layout = WindowLayout::Preset(LayoutPreset::SnapRight);
        assert_eq!(resolve_layout(&layout, &monitors()), (3200, 0, 1280, 1440));
    }

    #[test]
    fn targeted_preset_uses_selected_monitor_work_area() {
        let layout = WindowLayout::TargetedPreset {
            preset: LayoutPreset::SnapLeft,
            monitor: 2,
        };
        assert_eq!(resolve_layout(&layout, &monitors()), (0, 40, 960, 1040));
    }

    #[test]
    fn targeted_preset_out_of_range_falls_back_to_display_one() {
        let layout = WindowLayout::TargetedPreset {
            preset: LayoutPreset::TopHalf,
            monitor: 99,
        };
        assert_eq!(resolve_layout(&layout, &monitors()), (1920, 0, 2560, 720));
    }

    #[test]
    fn raw_bounds_are_absolute_virtual_screen_coordinates() {
        let layout = WindowLayout::Bounds {
            x: 1920,
            y: 40,
            width: 960,
            height: 1000,
        };
        assert_eq!(resolve_layout(&layout, &monitors()), (1920, 40, 960, 1000));
    }

    #[test]
    fn display_device_number_reads_windows_display_suffix() {
        assert_eq!(display_device_number(r"\\.\DISPLAY1"), Some(1));
        assert_eq!(display_device_number(r"\\.\DISPLAY12"), Some(12));
        assert_eq!(display_device_number("Generic PnP Monitor"), None);
    }
}
