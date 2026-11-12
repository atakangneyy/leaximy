//! The desktop visualizer: a borderless window drawn by the app and placed
//! behind the desktop icons, where the wallpaper is, on the primary monitor.
//!
//! Windows keeps the wallpaper in a `WorkerW` window that Progman creates
//! when sent the undocumented message 0x052C. Up to Windows 11 23H2 that
//! `WorkerW` is a top-level window right behind the one holding the icons
//! (`SHELLDLL_DefView`), and the visualizer becomes its child. From 24H2 the
//! icons and the `WorkerW` are both children of Progman, and the visualizer
//! becomes a layered child of Progman between the two.

use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetSystemMetrics,
    GetWindowLongPtrW, LWA_ALPHA, SM_CXSCREEN, SM_CYSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SMTO_NORMAL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos,
    WS_CHILD, WS_EX_LAYERED, WS_POPUP,
};

use crate::app::App;

/// The window's title, which is also how it is found to be moved.
const TITLE: &str = "Leaximy Desktop Visualizer";

/// Whether the window has been placed behind the icons.
static ATTACHED: AtomicBool = AtomicBool::new(false);

/// When the window was last looked for.
static CHECKED: Mutex<Option<Instant>> = Mutex::new(None);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Draws the visualizer in its own window and keeps it behind the icons.
pub fn show(app: &mut App, ctx: &egui::Context) {
    // SAFETY: GetSystemMetrics takes no pointers.
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    if width <= 0 || height <= 0 {
        return;
    }
    let scale = ctx.pixels_per_point().max(0.5);
    let builder = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_decorations(false)
        .with_taskbar(false)
        .with_active(false)
        .with_resizable(false)
        .with_inner_size([width as f32 / scale, height as f32 / scale]);
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of(TITLE),
        builder,
        |ui, _class| crate::ui::wallpaper(app, ui),
    );
    attach(width, height);
}

/// Forgets the window once the visualizer is turned off and egui closes it.
pub fn hide() {
    if ATTACHED.swap(false, Ordering::Relaxed) {
        *CHECKED.lock().unwrap_or_else(|error| error.into_inner()) = None;
    }
}

/// What `EnumWindows` looks for: the `WorkerW` behind the icons.
struct Search {
    progman: HWND,
    worker: HWND,
}

unsafe extern "system" fn behind_icons(window: HWND, search: LPARAM) -> i32 {
    let def_view = wide("SHELLDLL_DefView");
    let worker = wide("WorkerW");
    // SAFETY: `search` is the `Search` that `attach` passed, alive for the
    // whole enumeration; the strings are nul-terminated and outlive the calls.
    unsafe {
        let search = &mut *(search as *mut Search);
        if window != search.progman
            && !FindWindowExW(window, null_mut(), def_view.as_ptr(), null()).is_null()
        {
            let behind = FindWindowExW(null_mut(), window, worker.as_ptr(), null());
            if !behind.is_null() {
                search.worker = behind;
                return 0;
            }
        }
    }
    1
}

/// Moves the window behind the icons while it is still a top-level window.
/// A window that is already there is a child, which `FindWindowW` no longer
/// sees, so this does nothing more once it has worked.
fn attach(width: i32, height: i32) {
    {
        let mut checked = CHECKED.lock().unwrap_or_else(|error| error.into_inner());
        let wait = if ATTACHED.load(Ordering::Relaxed) {
            Duration::from_secs(2)
        } else {
            Duration::from_millis(50)
        };
        if checked.is_some_and(|at| at.elapsed() < wait) {
            return;
        }
        *checked = Some(Instant::now());
    }
    let title = wide(TITLE);
    let progman_class = wide("Progman");
    let def_view_class = wide("SHELLDLL_DefView");
    let worker_class = wide("WorkerW");
    // SAFETY: every string is nul-terminated and outlives its call; the
    // handles come from the system and are only passed back to it.
    unsafe {
        let window = FindWindowW(null(), title.as_ptr());
        if window.is_null() {
            return;
        }
        let progman = FindWindowW(progman_class.as_ptr(), null());
        if progman.is_null() {
            return;
        }
        // Ask Progman for the wallpaper layer.
        SendMessageTimeoutW(progman, 0x052C, 0xD, 0, SMTO_NORMAL, 1000, null_mut());
        SendMessageTimeoutW(progman, 0x052C, 0xD, 1, SMTO_NORMAL, 1000, null_mut());

        let mut search = Search {
            progman,
            worker: null_mut(),
        };
        EnumWindows(Some(behind_icons), &mut search as *mut Search as LPARAM);

        // The primary monitor's corner inside the parent, which spans the
        // whole virtual screen.
        let x = -GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = -GetSystemMetrics(SM_YVIRTUALSCREEN);
        if !search.worker.is_null() {
            SetParent(window, search.worker);
            SetWindowPos(
                window,
                null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        } else {
            // Windows 11 24H2 and later: a layered child of Progman, under
            // the icons and over Progman's own wallpaper window.
            let def_view = FindWindowExW(progman, null_mut(), def_view_class.as_ptr(), null());
            let worker = FindWindowExW(progman, null_mut(), worker_class.as_ptr(), null());
            let style = GetWindowLongPtrW(window, GWL_STYLE);
            SetWindowLongPtrW(
                window,
                GWL_STYLE,
                (style & !(WS_POPUP as isize)) | WS_CHILD as isize,
            );
            let extended = GetWindowLongPtrW(window, GWL_EXSTYLE);
            SetWindowLongPtrW(window, GWL_EXSTYLE, extended | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(window, 0, 255, LWA_ALPHA);
            SetParent(window, progman);
            SetWindowPos(
                window,
                def_view,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            if !worker.is_null() {
                SetWindowPos(
                    worker,
                    window,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        ATTACHED.store(true, Ordering::Relaxed);
    }
}
