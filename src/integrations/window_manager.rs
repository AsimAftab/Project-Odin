use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use sysinfo::System;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, SetWindowPos, ShowWindow, HWND_TOP, SM_CXSCREEN, SM_CYSCREEN, SWP_NOZORDER,
    SWP_SHOWWINDOW, SW_RESTORE, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED,
};

use crate::asgard::profile::{LayoutPreset, WindowLayout, WindowState};
use crate::integrations::launcher::is_shell_command;

const MONITORINFOF_PRIMARY: u32 = 0x0000_0001;

fn enable_per_monitor_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
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

/// One layout target the matcher is looking for. `app_index` is the caller's
/// index for the app (used to slot results in [`assign_candidates`]).
#[derive(Debug, Clone)]
pub struct LayoutTarget {
    pub app_index: usize,
    pub targets: Vec<String>,
    pub owned_pid: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MatchContext {
    /// HWNDs that existed before we launched anything.
    pub prelaunch: HashSet<isize>,
    /// HWNDs already assigned to some app; never matched again.
    pub claimed: HashSet<isize>,
    /// Our own process id — our windows are never candidates.
    pub own_pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCandidate {
    pub hwnd: isize,
    pub score: u8,
    pub app_index: usize,
}

fn is_cloaked(hwnd: HWND) -> bool {
    // Windows 10/11 UWP apps often have "cloaked" windows that are marked as
    // visible but aren't actually drawn on screen (e.g. splash screens or
    // virtual desktop shadows). We must skip these, otherwise we might snap a
    // hidden window and think we succeeded.
    let mut cloaked: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    }
}

unsafe extern "system" fn snapshot_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let set = &mut *(lparam.0 as *mut HashSet<isize>);
    if IsWindowVisible(hwnd).as_bool() && !is_cloaked(hwnd) {
        set.insert(hwnd.0);
    }
    BOOL(1)
}

/// All visible, non-cloaked top-level HWNDs right now. Taken before launching
/// so the matcher can tell freshly created windows from pre-existing ones.
pub fn snapshot_visible_windows() -> HashSet<isize> {
    let mut set: HashSet<isize> = HashSet::new();
    unsafe {
        let _ = EnumWindows(
            Some(snapshot_window_proc),
            LPARAM(&mut set as *mut _ as isize),
        );
    }
    set
}

struct EnumeratedWindow {
    hwnd: isize,
    pid: u32,
    title: String,
}

unsafe extern "system" fn collect_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<EnumeratedWindow>);

    if !IsWindowVisible(hwnd).as_bool() || is_cloaked(hwnd) {
        return BOOL(1);
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return BOOL(1);
    }

    let mut title_buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut title_buf);
    let title = if len > 0 {
        String::from_utf16_lossy(&title_buf[..len as usize])
    } else {
        String::new()
    };

    out.push(EnumeratedWindow {
        hwnd: hwnd.0,
        pid,
        title,
    });
    BOOL(1)
}

fn build_parent_map(sys: &System) -> HashMap<u32, u32> {
    sys.processes()
        .iter()
        .filter_map(|(pid, proc_)| proc_.parent().map(|pp| (pid.as_u32(), pp.as_u32())))
        .collect()
}

/// True if `pid`'s parent chain reaches `root` within 10 hops. Cycle-safe via
/// the hop cap.
pub fn is_descendant(pid: u32, root: u32, parents: &HashMap<u32, u32>) -> bool {
    let mut cur = pid;
    for _ in 0..10 {
        match parents.get(&cur) {
            Some(&parent) => {
                if parent == root {
                    return true;
                }
                if parent == cur {
                    return false;
                }
                cur = parent;
            }
            None => return false,
        }
    }
    false
}

/// Score a (window, target) pair. Higher is better; `None` means no match.
/// Pre-existing windows never match by title — only by exe name (weakly).
pub fn score_candidate(
    exact_pid: bool,
    descendant: bool,
    is_new: bool,
    name_match: bool,
    title_match: bool,
) -> Option<u8> {
    if exact_pid {
        Some(100)
    } else if descendant {
        Some(90)
    } else if is_new && name_match {
        Some(70)
    } else if is_new && title_match {
        Some(50)
    } else if !is_new && name_match {
        Some(30)
    } else {
        None
    }
}

/// Single EnumWindows pass scoring every (window, target) pair.
pub fn find_candidates(
    sys: &System,
    targets: &[LayoutTarget],
    ctx: &MatchContext,
) -> Vec<WindowCandidate> {
    let mut windows: Vec<EnumeratedWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_windows_proc),
            LPARAM(&mut windows as *mut _ as isize),
        );
    }

    let parents = build_parent_map(sys);
    let mut candidates = Vec::new();

    for window in &windows {
        if window.pid == ctx.own_pid || ctx.claimed.contains(&window.hwnd) {
            continue;
        }
        let is_new = !ctx.prelaunch.contains(&window.hwnd);
        let (exe_name, exe_stem) = process_names(sys, window.pid);
        let title_lower = window.title.to_lowercase();

        for target in targets {
            let exact_pid = target.owned_pid == Some(window.pid);
            let descendant = target
                .owned_pid
                .is_some_and(|owned| is_descendant(window.pid, owned, &parents));
            let name_match = target.targets.iter().any(|t| {
                let t_lower = t.to_lowercase();
                exe_name.as_deref() == Some(t_lower.as_str())
                    || exe_name.as_deref() == Some(format!("{t_lower}.exe").as_str())
                    || exe_stem.as_deref() == Some(t_lower.as_str())
            });
            let title_match = !title_lower.is_empty()
                && target
                    .targets
                    .iter()
                    .any(|t| title_lower.contains(&t.to_lowercase()));

            if let Some(score) =
                score_candidate(exact_pid, descendant, is_new, name_match, title_match)
            {
                candidates.push(WindowCandidate {
                    hwnd: window.hwnd,
                    score,
                    app_index: target.app_index,
                });
            }
        }
    }

    candidates
}

fn process_names(sys: &System, pid: u32) -> (Option<String>, Option<String>) {
    match sys.process(sysinfo::Pid::from_u32(pid)) {
        Some(process) => {
            let name = process.name().to_lowercase();
            let stem = process
                .exe()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());
            (Some(name), stem)
        }
        None => (None, None),
    }
}

/// Greedy assignment: best score first (ties broken by original enumeration
/// order = z-order, topmost wins), one hwnd per app, no hwnd used twice.
pub fn assign_candidates(cands: &[WindowCandidate], app_count: usize) -> Vec<Option<(isize, u8)>> {
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&a, &b| cands[b].score.cmp(&cands[a].score).then(a.cmp(&b)));

    let mut result: Vec<Option<(isize, u8)>> = vec![None; app_count];
    let mut used: HashSet<isize> = HashSet::new();
    for idx in order {
        let cand = &cands[idx];
        if cand.app_index >= app_count
            || result[cand.app_index].is_some()
            || used.contains(&cand.hwnd)
        {
            continue;
        }
        result[cand.app_index] = Some((cand.hwnd, cand.score));
        used.insert(cand.hwnd);
    }
    result
}

/// Move/size `hwnd` according to `layout`, compensating for the DWM invisible
/// border so the *visible* bounds land where requested.
pub fn position_window(hwnd: HWND, layout: &WindowLayout) -> Result<()> {
    enable_per_monitor_dpi_awareness();

    let monitors = ordered_monitors()?;
    let (mut x, mut y, mut w, mut h) = resolve_layout(layout, &monitors);

    unsafe {
        // Windows 10/11 have invisible borders for resizing. `SetWindowPos` sets the *actual* bounds
        // (including invisible borders), but `resolve_layout` returns the *visible* bounds.
        // We calculate the delta by comparing `GetWindowRect` (actual) to `DWMWA_EXTENDED_FRAME_BOUNDS` (visible).
        let mut actual_rect = RECT::default();
        let mut visible_rect = RECT::default();
        if GetWindowRect(hwnd, &mut actual_rect).is_ok()
            && DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut visible_rect as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_ok()
        {
            let left_border = visible_rect.left - actual_rect.left;
            let right_border = actual_rect.right - visible_rect.right;
            let bottom_border = actual_rect.bottom - visible_rect.bottom;
            let top_border = visible_rect.top - actual_rect.top;

            x -= left_border;
            y -= top_border;
            w += left_border + right_border;
            h += top_border + bottom_border;
        }

        // Restore window if it is minimized/maximized before moving it
        ShowWindow(hwnd, SW_RESTORE);

        SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW)
            .with_context(|| "failed to set window position")?;
    }

    Ok(())
}

/// Apply a window state post-launch (DirectExe/Explorer launches can't set
/// STARTUPINFO.wShowWindow, so min/max is applied here after matching).
pub fn set_window_state(hwnd: HWND, state: WindowState) {
    let cmd = match state {
        WindowState::Normal => return,
        WindowState::Minimized => SW_SHOWMINIMIZED,
        WindowState::Maximized => SW_SHOWMAXIMIZED,
    };
    unsafe {
        let _ = ShowWindow(hwnd, cmd);
    }
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

/// Build a list of candidate search strings for a startup app.
///
/// For normal commands: the exe file name, the stem (without .exe), then the
/// app's friendly name. For UWP `shell:` AUMIDs the path-derived pieces are
/// garbage, so we emit the friendly name plus the AUMID short name (e.g.
/// `Microsoft.WindowsCalculator_8wek...!App` → `WindowsCalculator`).
/// Deduplicates so the same target isn't tried twice.
pub fn search_targets(app_name: &str, command: &str) -> Vec<String> {
    fn push_unique(targets: &mut Vec<String>, value: &str) {
        let lower = value.to_lowercase();
        if !value.trim().is_empty() && !targets.iter().any(|t| t.to_lowercase() == lower) {
            targets.push(value.to_string());
        }
    }

    let mut targets = Vec::with_capacity(3);

    if is_shell_command(command) {
        push_unique(&mut targets, app_name);
        if let Some(short) = aumid_short_name(command) {
            push_unique(&mut targets, &short);
        }
        return targets;
    }

    let path = std::path::Path::new(command);

    // 1. File name from command (e.g. "wt.exe" from "C:\...\wt.exe")
    if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
        push_unique(&mut targets, fname);
    }

    // 2. Stem without extension (e.g. "wt" from "wt.exe")
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        push_unique(&mut targets, stem);
    }

    // 3. Friendly app name (e.g. "Calculator")
    push_unique(&mut targets, app_name);

    targets
}

/// `shell:AppsFolder\Microsoft.WindowsCalculator_8wek...!App` → `WindowsCalculator`:
/// the segment between the last `\` and the first `_`/`!`, minus any dotted
/// vendor prefix.
fn aumid_short_name(command: &str) -> Option<String> {
    let segment = command.rsplit('\\').next().unwrap_or(command);
    let cut = match segment.find(['_', '!']) {
        Some(i) => &segment[..i],
        None => segment,
    };
    let short = cut.rsplit('.').next().unwrap_or(cut);
    if short.trim().is_empty() {
        None
    } else {
        Some(short.to_string())
    }
}

fn resolve_layout(layout: &WindowLayout, monitors: &[MonitorWorkArea]) -> (i32, i32, i32, i32) {
    match layout {
        WindowLayout::Bounds {
            x,
            y,
            width,
            height,
        } => clamp_bounds(*x, *y, *width, *height, monitors),
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
        .or_else(|| monitors.iter().find(|m| m.is_primary).cloned())
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

fn intersection_area(area: WorkArea, x: i32, y: i32, w: i32, h: i32) -> i64 {
    let left = x.max(area.left);
    let top = y.max(area.top);
    let right = (x + w).min(area.right);
    let bottom = (y + h).min(area.bottom);
    ((right - left).max(0) as i64) * ((bottom - top).max(0) as i64)
}

/// Clamp raw bounds into the monitor they mostly overlap (primary if they
/// overlap none), so stale profile coordinates never place a window off-screen.
fn clamp_bounds(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    monitors: &[MonitorWorkArea],
) -> (i32, i32, i32, i32) {
    let Some(monitor) = monitors
        .iter()
        .map(|m| (intersection_area(m.work_area, x, y, w, h), m))
        .max_by_key(|(area, _)| *area)
        .filter(|(area, _)| *area > 0)
        .map(|(_, m)| m)
        .or_else(|| monitors.iter().find(|m| m.is_primary))
        .or_else(|| monitors.first())
    else {
        return (x, y, w, h);
    };

    let work = monitor.work_area;
    let w = w.min(work.width()).max(100);
    let h = h.min(work.height()).max(100);
    let x = x.clamp(work.left, (work.right - w).max(work.left));
    let y = y.clamp(work.top, (work.bottom - h).max(work.top));
    (x, y, w, h)
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
    fn targeted_preset_out_of_range_falls_back_to_primary() {
        let layout = WindowLayout::TargetedPreset {
            preset: LayoutPreset::TopHalf,
            monitor: 99,
        };
        assert_eq!(resolve_layout(&layout, &monitors()), (0, 40, 1920, 520));
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
    fn fully_off_screen_bounds_are_clamped_into_primary() {
        assert_eq!(
            clamp_bounds(-5000, -5000, 800, 600, &monitors()),
            (0, 40, 800, 600)
        );
    }

    #[test]
    fn oversized_bounds_are_shrunk_to_work_area() {
        assert_eq!(
            clamp_bounds(1920, 0, 5000, 3000, &monitors()),
            (1920, 0, 2560, 1440)
        );
    }

    #[test]
    fn straddling_bounds_are_clamped_into_majority_monitor() {
        // 1000..2800 x 100..900: 920x800 on primary (DISPLAY2), 880x800 on
        // DISPLAY1 — primary wins, so x is pulled left to fit.
        assert_eq!(
            clamp_bounds(1000, 100, 1800, 800, &monitors()),
            (120, 100, 1800, 800)
        );
    }

    #[test]
    fn display_device_number_reads_windows_display_suffix() {
        assert_eq!(display_device_number(r"\\.\DISPLAY1"), Some(1));
        assert_eq!(display_device_number(r"\\.\DISPLAY12"), Some(12));
        assert_eq!(display_device_number("Generic PnP Monitor"), None);
    }

    #[test]
    fn score_candidate_matrix() {
        // Exact PID always wins, regardless of other signals.
        assert_eq!(score_candidate(true, false, false, false, false), Some(100));
        assert_eq!(score_candidate(true, true, true, true, true), Some(100));
        // Descendant of the owned PID.
        assert_eq!(score_candidate(false, true, false, false, false), Some(90));
        // New window with matching exe name.
        assert_eq!(score_candidate(false, false, true, true, false), Some(70));
        assert_eq!(score_candidate(false, false, true, true, true), Some(70));
        // New window matched only by title.
        assert_eq!(score_candidate(false, false, true, false, true), Some(50));
        // Pre-existing window with matching exe name.
        assert_eq!(score_candidate(false, false, false, true, false), Some(30));
        assert_eq!(score_candidate(false, false, false, true, true), Some(30));
        // Old window + title only: NEVER a match.
        assert_eq!(score_candidate(false, false, false, false, true), None);
        // Nothing at all.
        assert_eq!(score_candidate(false, false, false, false, false), None);
        assert_eq!(score_candidate(false, false, true, false, false), None);
    }

    #[test]
    fn is_descendant_direct_child() {
        let parents: HashMap<u32, u32> = [(20, 10)].into();
        assert!(is_descendant(20, 10, &parents));
    }

    #[test]
    fn is_descendant_three_hops() {
        let parents: HashMap<u32, u32> = [(40, 30), (30, 20), (20, 10)].into();
        assert!(is_descendant(40, 10, &parents));
    }

    #[test]
    fn is_descendant_cycle_is_safe() {
        let parents: HashMap<u32, u32> = [(1, 2), (2, 1)].into();
        assert!(!is_descendant(1, 99, &parents));
        // Self-parented (PID 0 style) also terminates.
        let self_parented: HashMap<u32, u32> = [(5, 5)].into();
        assert!(!is_descendant(5, 99, &self_parented));
    }

    #[test]
    fn is_descendant_missing_pid() {
        let parents: HashMap<u32, u32> = [(20, 10)].into();
        assert!(!is_descendant(999, 10, &parents));
        assert!(!is_descendant(20, 999, &parents));
    }

    #[test]
    fn assign_candidates_higher_score_wins_contested_hwnd() {
        let cands = vec![
            WindowCandidate {
                hwnd: 1,
                score: 70,
                app_index: 0,
            },
            WindowCandidate {
                hwnd: 1,
                score: 90,
                app_index: 1,
            },
        ];
        let assigned = assign_candidates(&cands, 2);
        assert_eq!(assigned[0], None);
        assert_eq!(assigned[1], Some((1, 90)));
    }

    #[test]
    fn assign_candidates_tie_broken_by_enumeration_order() {
        let cands = vec![
            WindowCandidate {
                hwnd: 1,
                score: 70,
                app_index: 0,
            },
            WindowCandidate {
                hwnd: 2,
                score: 70,
                app_index: 0,
            },
        ];
        let assigned = assign_candidates(&cands, 1);
        assert_eq!(assigned[0], Some((1, 70)));
    }

    #[test]
    fn assign_candidates_never_reuses_hwnd() {
        let cands = vec![
            WindowCandidate {
                hwnd: 1,
                score: 70,
                app_index: 0,
            },
            WindowCandidate {
                hwnd: 1,
                score: 70,
                app_index: 1,
            },
            WindowCandidate {
                hwnd: 2,
                score: 50,
                app_index: 1,
            },
        ];
        let assigned = assign_candidates(&cands, 2);
        assert_eq!(assigned[0], Some((1, 70)));
        assert_eq!(assigned[1], Some((2, 50)));
    }

    #[test]
    fn search_targets_shell_command_uses_friendly_and_aumid_short_name() {
        let targets = search_targets(
            "Calculator",
            "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
        );
        assert_eq!(targets, vec!["Calculator", "WindowsCalculator"]);
    }

    #[test]
    fn search_targets_shell_command_dedupes_case_insensitively() {
        let targets = search_targets(
            "windowscalculator",
            "SHELL:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
        );
        assert_eq!(targets, vec!["windowscalculator"]);
    }

    #[test]
    fn search_targets_regular_command_keeps_path_derived_targets() {
        let targets = search_targets("Terminal", r"C:\Tools\wt.exe");
        assert_eq!(targets, vec!["wt.exe", "wt", "Terminal"]);
    }
}
