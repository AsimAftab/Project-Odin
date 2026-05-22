use anyhow::{Context, Result};
use sysinfo::System;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowPos, HWND_TOP, SM_CXSCREEN, SM_CYSCREEN, SWP_NOZORDER, SWP_SHOWWINDOW,
};

use crate::asgard::profile::{LayoutPreset, WindowLayout};

struct WindowSearch {
    target_exe: String,
    found_hwnd: Option<HWND>,
    sys: System,
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

pub fn apply_layout(exe_name: &str, layout: &WindowLayout) -> Result<()> {
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

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    let (x, y, w, h) = match layout {
        WindowLayout::Bounds {
            x,
            y,
            width,
            height,
        } => (*x, *y, *width, *height),
        WindowLayout::Preset(preset) => match preset {
            LayoutPreset::SnapLeft => (0, 0, screen_w / 2, screen_h),
            LayoutPreset::SnapRight => (screen_w / 2, 0, screen_w / 2, screen_h),
            LayoutPreset::TopHalf => (0, 0, screen_w, screen_h / 2),
            LayoutPreset::BottomHalf => (0, screen_h / 2, screen_w, screen_h / 2),
            LayoutPreset::Quadrant1 => (screen_w / 2, 0, screen_w / 2, screen_h / 2),
            LayoutPreset::Quadrant2 => (0, 0, screen_w / 2, screen_h / 2),
            LayoutPreset::Quadrant3 => (0, screen_h / 2, screen_w / 2, screen_h / 2),
            LayoutPreset::Quadrant4 => (screen_w / 2, screen_h / 2, screen_w / 2, screen_h / 2),
        },
    };

    unsafe {
        SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW)
            .with_context(|| "failed to set window position")?;
    }

    Ok(())
}
