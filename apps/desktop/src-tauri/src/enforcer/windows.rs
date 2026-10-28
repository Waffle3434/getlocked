//! Windows implementation: Toolhelp32 process snapshots, window enumeration,
//! WM_CLOSE and TerminateProcess. Needs no admin rights and changes nothing
//! on the system.

use std::collections::{HashMap, HashSet};

use getlocked_core::matcher::ProcessInfo;
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, WAIT_TIMEOUT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GW_OWNER,
    WM_CLOSE,
};

use super::Platform;

pub struct WinPlatform {
    session_id: u32,
    /// pid → full exe path (None if the process can't be opened). Cleared
    /// for pids that disappear.
    paths: HashMap<u32, Option<String>>,
    /// pid → windows to send WM_CLOSE to, from the last snapshot.
    windows: HashMap<u32, Vec<isize>>,
}

impl WinPlatform {
    pub fn new() -> Self {
        Self { session_id: session_of(std::process::id()).unwrap_or(1), paths: HashMap::new(), windows: HashMap::new() }
    }
}

fn session_of(pid: u32) -> Option<u32> {
    let mut sid = 0u32;
    unsafe { ProcessIdToSessionId(pid, &mut sid) }.ok().map(|_| sid)
}

/// Full exe path, or None if we can't open the process (elevated, protected
/// or another user's), in which case we leave it alone.
fn exe_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn process_alive(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else { return false };
        let alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
        let _ = CloseHandle(h);
        alive
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn list_processes() -> Vec<(u32, String)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                out.push((entry.th32ProcessID, wide_to_string(&entry.szExeFile)));
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    }
}

unsafe extern "system" fn collect_top_level(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<isize>);
    let unowned = GetWindow(hwnd, GW_OWNER).map_or(true, |owner| owner.0.is_null());
    if IsWindowVisible(hwnd).as_bool() && unowned && !is_cloaked(hwnd) {
        out.push(hwnd.0 as isize);
    }
    BOOL(1)
}

unsafe extern "system" fn collect_children(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<isize>);
    out.push(hwnd.0 as isize);
    BOOL(1)
}

/// Visible top-level windows grouped by owning pid. Store (UWP) apps live
/// inside an ApplicationFrameHost frame, so those frames are credited to the
/// app process found among their children.
fn visible_windows(frame_hosts: &HashSet<u32>) -> HashMap<u32, Vec<isize>> {
    let mut top: Vec<isize> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_top_level), LPARAM(&mut top as *mut _ as isize));
    }
    let mut by_pid: HashMap<u32, Vec<isize>> = HashMap::new();
    for raw in top {
        let hwnd = HWND(raw as *mut _);
        let mut pid = window_pid(hwnd);
        if frame_hosts.contains(&pid) {
            let mut children: Vec<isize> = Vec::new();
            unsafe {
                let _ = EnumChildWindows(Some(hwnd), Some(collect_children), LPARAM(&mut children as *mut _ as isize));
            }
            if let Some(app_pid) =
                children.into_iter().map(|c| window_pid(HWND(c as *mut _))).find(|p| *p != pid && *p != 0)
            {
                pid = app_pid;
            }
        }
        by_pid.entry(pid).or_default().push(raw);
    }
    by_pid
}

impl Platform for WinPlatform {
    fn snapshot(&mut self) -> Vec<ProcessInfo> {
        let procs = list_processes();
        let live: HashSet<u32> = procs.iter().map(|(pid, _)| *pid).collect();
        self.paths.retain(|pid, _| live.contains(pid));

        let frame_hosts: HashSet<u32> = procs
            .iter()
            .filter(|(_, name)| name.eq_ignore_ascii_case("ApplicationFrameHost.exe"))
            .map(|(pid, _)| *pid)
            .collect();
        self.windows = visible_windows(&frame_hosts);

        let mut out = Vec::new();
        for (pid, exe_name) in procs {
            if pid == 0 || pid == 4 || session_of(pid) != Some(self.session_id) {
                continue;
            }
            let path = self.paths.entry(pid).or_insert_with(|| exe_path(pid)).clone();
            if path.is_none() {
                continue;
            }
            out.push(ProcessInfo {
                pid,
                exe_name,
                exe_path: path,
                has_visible_window: self.windows.contains_key(&pid),
            });
        }
        out
    }

    fn request_close(&mut self, pid: u32) -> bool {
        let Some(wins) = self.windows.get(&pid) else { return false };
        let mut posted = false;
        for &raw in wins {
            posted |= unsafe { PostMessageW(Some(HWND(raw as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok();
        }
        posted
    }

    fn kill(&mut self, pid: u32) {
        unsafe {
            if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                let _ = TerminateProcess(h, 1);
                let _ = CloseHandle(h);
            }
        }
    }
}
