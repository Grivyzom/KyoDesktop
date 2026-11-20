use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, WebviewWindow};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DesktopBounds {
    pub work_area: ScreenRect,
    pub screen_size: ScreenSize,
    pub scale_factor: f64,
    pub current_x: i32,
    pub current_y: i32,
    pub window_width: i32,
    pub window_height: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenWindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MovementTick {
    pub x: i32,
    pub y: i32,
    pub vx: f64,
    pub vy: f64,
    pub progress: f64,
    pub is_moving: bool,
}

#[cfg(windows)]
pub mod win32 {
    pub type HWND = isize;
    pub type BOOL = i32;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct POINT {
        pub x: i32,
        pub y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct WINDOWPOS {
        pub hwnd: HWND,
        pub hwnd_insert_after: HWND,
        pub x: i32,
        pub y: i32,
        pub cx: i32,
        pub cy: i32,
        pub flags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct MSG {
        pub hwnd: HWND,
        pub message: u32,
        pub w_param: usize,
        pub l_param: isize,
        pub time: u32,
        pub pt: POINT,
    }

    pub type SubclassProc = unsafe extern "system" fn(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
        uid_subclass: usize,
        ref_data: usize,
    ) -> isize;

    pub type WinEventProc = unsafe extern "system" fn(
        win_event_hook: isize,
        event: u32,
        hwnd: HWND,
        id_object: i32,
        id_child: i32,
        event_thread: u32,
        event_time: u32,
    );

    pub const HWND_TOP: HWND = 0;
    pub const HWND_BOTTOM: HWND = 1;
    pub const HWND_TOPMOST: HWND = -1;
    pub const HWND_NOTOPMOST: HWND = -2;

    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOMOVE: u32 = 0x0002;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    pub const SWP_FRAMECHANGED: u32 = 0x0020;
    pub const SWP_SHOWWINDOW: u32 = 0x0040;
    pub const SWP_HIDEWINDOW: u32 = 0x0080;
    pub const SWP_NOSENDCHANGING: u32 = 0x0400;
    pub const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    pub const SPI_GETWORKAREA: u32 = 0x0030;

    pub const GWL_STYLE: i32 = -16;
    pub const GWL_EXSTYLE: i32 = -20;
    pub const WS_CAPTION: u32 = 0x00C0_0000;
    pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
    pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
    pub const WS_EX_APPWINDOW: u32 = 0x0004_0000;
    pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

    pub const WM_ERASEBKGND: u32 = 0x0014;
    pub const WM_MOUSEACTIVATE: u32 = 0x0021;
    pub const WM_WINDOWPOSCHANGING: u32 = 0x0046;
    pub const WM_NCACTIVATE: u32 = 0x0086;
    pub const WM_DWMCOMPOSITIONCHANGED: u32 = 0x031E;
    pub const MA_NOACTIVATE: isize = 3;

    pub const GW_HWNDPREV: u32 = 3;
    pub const MONITOR_DEFAULTTONEAREST: u32 = 2;

    pub const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
    pub const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
    pub const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
    pub const EVENT_OBJECT_DESTROY: u32 = 0x8001;
    pub const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
    pub const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

    pub const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;
    pub const DWMWA_EXCLUDED_FROM_PEEK: u32 = 12;

    pub const QUNS_BUSY: i32 = 2;
    pub const QUNS_RUNNING_D3D_FULL_SCREEN: i32 = 3;
    pub const QUNS_PRESENTATION_MODE: i32 = 4;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct MONITORINFO {
        pub cb_size: u32,
        pub rc_monitor: RECT,
        pub rc_work: RECT,
        pub dw_flags: u32,
    }

    pub const QS_ALLINPUT: u32 = 0x04FF;
    pub const PM_REMOVE: u32 = 0x0001;
    pub const WAIT_TIMEOUT: u32 = 0x0000_0102;

    extern "system" {
        pub fn SetWindowPos(
            hWnd: HWND,
            hWndInsertAfter: HWND,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> BOOL;
        pub fn GetWindowLongW(hWnd: HWND, nIndex: i32) -> i32;
        pub fn SetWindowLongW(hWnd: HWND, nIndex: i32, dwNewLong: i32) -> i32;
        pub fn GetWindow(hWnd: HWND, uCmd: u32) -> HWND;
        pub fn GetForegroundWindow() -> HWND;
        pub fn IsWindow(hWnd: HWND) -> BOOL;
        pub fn IsWindowVisible(hWnd: HWND) -> BOOL;
        pub fn IsIconic(hWnd: HWND) -> BOOL;
        pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
        pub fn MonitorFromWindow(hwnd: HWND, dwFlags: u32) -> isize;
        pub fn GetMonitorInfoW(hMonitor: isize, lpmi: *mut MONITORINFO) -> BOOL;
        pub fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        pub fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(HWND, isize) -> BOOL, lParam: isize) -> BOOL;
        pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
        pub fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: BOOL, dwProcessId: u32) -> isize;
        pub fn CloseHandle(hObject: isize) -> BOOL;
        pub fn QueryFullProcessImageNameW(hProcess: isize, dwFlags: u32, lpExeName: *mut u16, lpdwSize: *mut u32) -> BOOL;
        pub fn SystemParametersInfoW(uiAction: u32, uiParam: u32, pvParam: *mut std::ffi::c_void, fWinIni: u32) -> BOOL;
        pub fn SetWinEventHook(
            eventMin: u32,
            eventMax: u32,
            hmodWinEventProc: isize,
            pfnWinEventProc: WinEventProc,
            idProcess: u32,
            idThread: u32,
            dwFlags: u32,
        ) -> isize;
        pub fn UnhookWinEvent(hWinEventHook: isize) -> BOOL;
        pub fn PeekMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> BOOL;
        pub fn MsgWaitForMultipleObjects(nCount: u32, pHandles: *const isize, fWaitAll: BOOL, dwMilliseconds: u32, dwWakeMask: u32) -> u32;
        pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
        pub fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    }

    #[link(name = "shell32")]
    extern "system" {
        pub fn SHQueryUserNotificationState(pquns: *mut i32) -> i32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        pub fn DwmSetWindowAttribute(
            hwnd: HWND,
            dwAttribute: u32,
            pvAttribute: *const std::ffi::c_void,
            cbAttribute: u32,
        ) -> i32;
    }

    #[link(name = "comctl32")]
    extern "system" {
        pub fn SetWindowSubclass(
            hWnd: HWND,
            pfnSubclass: SubclassProc,
            uIdSubclass: usize,
            dwRefData: usize,
        ) -> BOOL;
        pub fn DefSubclassProc(
            hWnd: HWND,
            uMsg: u32,
            wParam: usize,
            lParam: isize,
        ) -> isize;
    }
}

static CURRENT_LAYER: Mutex<Option<String>> = Mutex::new(None);
static IS_TOP_LAYER: AtomicBool = AtomicBool::new(true);
static IS_YIELDED_TO_FULLSCREEN: AtomicBool = AtomicBool::new(false);
static AVATAR_HWND: AtomicIsize = AtomicIsize::new(0);
#[cfg(windows)]
static SUBCLASS_INSTALLED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static WINEVENT_HOOK_STARTED: AtomicBool = AtomicBool::new(false);
static MOVEMENT_GENERATION: AtomicU64 = AtomicU64::new(0);
static IS_AVATAR_ANIMATING: AtomicBool = AtomicBool::new(false);

pub fn cancel_active_animation() {
    MOVEMENT_GENERATION.fetch_add(1, Ordering::SeqCst);
    IS_AVATAR_ANIMATING.store(false, Ordering::SeqCst);
}

#[inline]
fn is_top_layer() -> bool {
    IS_TOP_LAYER.load(Ordering::SeqCst)
}

#[inline]
fn is_yielded_to_fullscreen() -> bool {
    IS_YIELDED_TO_FULLSCREEN.load(Ordering::SeqCst)
}

#[cfg(windows)]
const AVATAR_SUBCLASS_ID: usize = 0x4B59_4F01;

#[cfg(windows)]
unsafe fn apply_dwm_overlay_attrs(hwnd: win32::HWND, avatar_mode: bool) {
    let val: i32 = if avatar_mode { 1 } else { 0 };
    let _ = win32::DwmSetWindowAttribute(
        hwnd,
        win32::DWMWA_TRANSITIONS_FORCEDISABLED,
        &val as *const i32 as *const std::ffi::c_void,
        std::mem::size_of::<i32>() as u32,
    );
    let _ = win32::DwmSetWindowAttribute(
        hwnd,
        win32::DWMWA_EXCLUDED_FROM_PEEK,
        &val as *const i32 as *const std::ffi::c_void,
        std::mem::size_of::<i32>() as u32,
    );
}

#[cfg(windows)]
unsafe fn is_fullscreen_app_on_avatar_monitor(avatar_hwnd: win32::HWND) -> Option<win32::HWND> {
    use win32::*;

    let fg = GetForegroundWindow();
    if fg == 0 || fg == avatar_hwnd || IsWindow(fg) == 0 || IsWindowVisible(fg) == 0 || IsIconic(fg) != 0 {
        return None;
    }

    let mut class_buf = [0u16; 128];
    let len = GetClassNameW(fg, class_buf.as_mut_ptr(), 128);
    if len > 0 {
        let cname = String::from_utf16_lossy(&class_buf[..len as usize]);
        if cname == "Progman"
            || cname == "WorkerW"
            || cname == "Shell_TrayWnd"
            || cname == "Shell_SecondaryTrayWnd"
            || cname == "#32768"
            || cname == "Windows.UI.Core.CoreWindow"
            || cname == "XamlExplorerHostIslandWindow"
        {
            return None;
        }
    }

    let fg_mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
    let av_mon = MonitorFromWindow(avatar_hwnd, MONITOR_DEFAULTTONEAREST);
    if fg_mon == 0 || av_mon == 0 || fg_mon != av_mon {
        return None;
    }

    let mut mi = MONITORINFO {
        cb_size: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(fg_mon, &mut mi) == 0 {
        return None;
    }

    let mut fg_rc = RECT::default();
    if GetWindowRect(fg, &mut fg_rc) == 0 {
        return None;
    }

    let covers_monitor = fg_rc.left <= mi.rc_monitor.left
        && fg_rc.top <= mi.rc_monitor.top
        && fg_rc.right >= mi.rc_monitor.right
        && fg_rc.bottom >= mi.rc_monitor.bottom;

    if !covers_monitor {
        return None;
    }

    // Si la barra de tareas ocupa espacio (rc_work != rc_monitor), cualquier ventana que cubra
    // todo rc_monitor (incluyendo la barra de tareas) está en pantalla completa (juego / F11 / video).
    let taskbar_takes_space = mi.rc_work.left != mi.rc_monitor.left
        || mi.rc_work.top != mi.rc_monitor.top
        || mi.rc_work.right != mi.rc_monitor.right
        || mi.rc_work.bottom != mi.rc_monitor.bottom;

    if taskbar_takes_space {
        return Some(fg);
    }

    // Si la barra de tareas está auto-ocultada (o en monitor secundario sin barra de tareas),
    // confiar en SHQueryUserNotificationState para distinguir juegos/apps fullscreen de ventanas maximizadas.
    let mut quns: i32 = 0;
    if SHQueryUserNotificationState(&mut quns) == 0
        && (quns == QUNS_BUSY || quns == QUNS_RUNNING_D3D_FULL_SCREEN || quns == QUNS_PRESENTATION_MODE)
    {
        return Some(fg);
    }

    None
}

#[cfg(windows)]
unsafe fn apply_overlay_ex_style(hwnd: win32::HWND, avatar_mode: bool) {
    use win32::*;
    if hwnd == 0 || IsWindow(hwnd) == 0 {
        return;
    }
    let cur_ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let new_ex = if avatar_mode {
        (cur_ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW
    } else {
        (cur_ex & !(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)) | WS_EX_APPWINDOW
    };

    let needs_style_update = new_ex != cur_ex;
    let missing_topmost = avatar_mode
        && is_top_layer()
        && !is_yielded_to_fullscreen()
        && (cur_ex & WS_EX_TOPMOST) == 0;

    if needs_style_update {
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex as i32);
    }

    if needs_style_update || missing_topmost {
        let should_topmost = avatar_mode && is_top_layer() && !is_yielded_to_fullscreen();
        let insert_after = if should_topmost { HWND_TOPMOST } else { 0 };
        let mut flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED;
        if insert_after == 0 {
            flags |= SWP_NOZORDER;
        }
        SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, flags);
    }

    apply_dwm_overlay_attrs(hwnd, avatar_mode);
}

#[cfg(windows)]
unsafe fn needs_topmost_reassert(avatar_hwnd: win32::HWND) -> bool {
    use win32::*;
    let ex = GetWindowLongW(avatar_hwnd, GWL_EXSTYLE) as u32;
    if (ex & WS_EX_TOPMOST) == 0 {
        return true;
    }

    let mut prev = GetWindow(avatar_hwnd, GW_HWNDPREV);
    let mut steps = 0;
    while prev != 0 && steps < 40 {
        if IsWindowVisible(prev) != 0 {
            let prev_ex = GetWindowLongW(prev, GWL_EXSTYLE) as u32;
            // Si cualquier ventana visible no-topmost quedó por encima del avatar, re-afirmar TOPMOST
            if (prev_ex & WS_EX_TOPMOST) == 0 {
                return true;
            }
        }
        prev = GetWindow(prev, GW_HWNDPREV);
        steps += 1;
    }
    false
}

#[cfg(windows)]
unsafe fn sync_topmost_zorder(avatar_hwnd: win32::HWND, use_async: bool) {
    use win32::*;

    if crate::is_settings_open() || crate::is_context_menu_open() || !is_top_layer() {
        return;
    }
    if avatar_hwnd == 0 || IsWindow(avatar_hwnd) == 0 || IsWindowVisible(avatar_hwnd) == 0 {
        return;
    }

    let async_flag = if use_async { SWP_ASYNCWINDOWPOS } else { 0 };

    if let Some(fs_hwnd) = is_fullscreen_app_on_avatar_monitor(avatar_hwnd) {
        IS_YIELDED_TO_FULLSCREEN.store(true, Ordering::SeqCst);
        let ex = GetWindowLongW(avatar_hwnd, GWL_EXSTYLE) as u32;
        if (ex & WS_EX_TOPMOST) != 0 {
            SetWindowPos(
                avatar_hwnd,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | async_flag,
            );
            SetWindowPos(
                avatar_hwnd,
                fs_hwnd,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | async_flag,
            );
        }
        return;
    }

    let was_yielded = IS_YIELDED_TO_FULLSCREEN.swap(false, Ordering::SeqCst);
    if was_yielded || needs_topmost_reassert(avatar_hwnd) {
        SetWindowPos(
            avatar_hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | async_flag,
        );
    }
}

#[cfg(windows)]
unsafe extern "system" fn avatar_wnd_subclass_proc(
    hwnd: win32::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid_subclass: usize,
    _ref_data: usize,
) -> isize {
    use win32::*;

    if !crate::is_settings_open() {
        match msg {
            WM_WINDOWPOSCHANGING => {
                let wp = &mut *(lparam as *mut WINDOWPOS);
                if is_top_layer()
                    && !is_yielded_to_fullscreen()
                    && (wp.flags & SWP_NOZORDER) == 0
                    && (wp.flags & SWP_HIDEWINDOW) == 0
                {
                    wp.hwnd_insert_after = HWND_TOPMOST;
                }
                if !crate::is_context_menu_open() && (wp.flags & SWP_HIDEWINDOW) == 0 {
                    wp.flags |= SWP_NOACTIVATE;
                }
            }
            WM_NCACTIVATE => {
                if !crate::is_context_menu_open() {
                    // Evita que DefWindowProcW repinte el marco no-cliente al cerrarse otras ventanas
                    return 1;
                }
            }
            WM_MOUSEACTIVATE => {
                if !crate::is_context_menu_open() {
                    // Procesa el clic/arrastre sin robar foco ni provocar parpadeo de activación
                    return MA_NOACTIVATE;
                }
            }
            WM_ERASEBKGND => {
                // Evita borrados GDI del fondo transparente durante recomposiciones DWM
                return 1;
            }
            WM_DWMCOMPOSITIONCHANGED => {
                apply_dwm_overlay_attrs(hwnd, true);
                sync_topmost_zorder(hwnd, false);
            }
            _ => {}
        }
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(windows)]
unsafe extern "system" fn win_event_topmost_guard(
    _hook: isize,
    event: u32,
    hwnd: win32::HWND,
    id_object: i32,
    id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    use win32::*;

    if crate::is_settings_open() || crate::is_context_menu_open() || !is_top_layer() {
        return;
    }

    // Para EVENT_OBJECT_DESTROY, ignorar controles hijos y atender solo destrucción de ventanas raíz
    if event == EVENT_OBJECT_DESTROY && (id_object != 0 || id_child != 0) {
        return;
    }

    let avatar_hwnd = AVATAR_HWND.load(Ordering::SeqCst);
    if avatar_hwnd == 0 || hwnd == avatar_hwnd || IsWindow(avatar_hwnd) == 0 || IsWindowVisible(avatar_hwnd) == 0 {
        return;
    }

    if event == EVENT_SYSTEM_FOREGROUND && hwnd != 0 {
        let mut class_buf = [0u16; 128];
        let len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 128);
        if len > 0 {
            let cname = String::from_utf16_lossy(&class_buf[..len as usize]);
            // No interferir con menús emergentes nativos ni overlays del Shell (Menú Inicio / Búsqueda)
            if cname == "#32768"
                || cname == "Windows.UI.Core.CoreWindow"
                || cname == "XamlExplorerHostIslandWindow"
            {
                return;
            }
        }
    }

    sync_topmost_zorder(avatar_hwnd, true);
}

#[cfg(windows)]
fn start_topmost_win_event_guard() {
    if WINEVENT_HOOK_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    std::thread::spawn(|| unsafe {
        use win32::*;
        let h_fg = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            0,
            win_event_topmost_guard,
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        let h_min = SetWinEventHook(
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
            0,
            win_event_topmost_guard,
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        let h_destroy = SetWinEventHook(
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_DESTROY,
            0,
            win_event_topmost_guard,
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );

        let mut msg = MSG::default();
        loop {
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                if msg.message == 0x0012 {
                    // WM_QUIT
                    if h_fg != 0 {
                        UnhookWinEvent(h_fg);
                    }
                    if h_min != 0 {
                        UnhookWinEvent(h_min);
                    }
                    if h_destroy != 0 {
                        UnhookWinEvent(h_destroy);
                    }
                    return;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let wait_res = MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 400, QS_ALLINPUT);
            if wait_res == WAIT_TIMEOUT {
                let avatar_hwnd = AVATAR_HWND.load(Ordering::SeqCst);
                sync_topmost_zorder(avatar_hwnd, true);
            }
        }
    });
}

/// Configura los estilos Win32 y atributos DWM para que el avatar se comporte como
/// un overlay no-activable (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) que nunca parpadea
/// ni desaparece al cerrarse otras ventanas del sistema.
pub fn apply_avatar_overlay_mode(window: &WebviewWindow, is_avatar_mode: bool) {
    let _ = window.set_focusable(!is_avatar_mode);

    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            let raw_hwnd = hwnd.0 as isize;
            AVATAR_HWND.store(raw_hwnd, Ordering::SeqCst);

            let should_install_subclass = SUBCLASS_INSTALLED
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok();

            let win_for_main = window.clone();
            let _ = win_for_main.run_on_main_thread(move || unsafe {
                if should_install_subclass {
                    win32::SetWindowSubclass(raw_hwnd, avatar_wnd_subclass_proc, AVATAR_SUBCLASS_ID, 0);
                }
                apply_overlay_ex_style(raw_hwnd, is_avatar_mode);
            });

            start_topmost_win_event_guard();
        }
    }
}

/// Re-aplica estilos de overlay directamente sobre el HWND guardado (útil tras eventos Resized).
pub fn refresh_overlay_styles_if_avatar() {
    #[cfg(windows)]
    {
        if crate::is_settings_open() {
            return;
        }
        let raw_hwnd = AVATAR_HWND.load(Ordering::SeqCst);
        if raw_hwnd != 0 {
            unsafe {
                apply_overlay_ex_style(raw_hwnd, true);
            }
        }
    }
}

/// Re-afirma HWND_TOPMOST sin activar la ventana si el avatar está en capa superior.
pub fn ensure_avatar_topmost_if_needed() {
    #[cfg(windows)]
    {
        if crate::is_settings_open() || !is_top_layer() {
            return;
        }
        let raw_hwnd = AVATAR_HWND.load(Ordering::SeqCst);
        if raw_hwnd != 0 {
            unsafe {
                sync_topmost_zorder(raw_hwnd, false);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// GESTIÓN DE CAPAS DE VENTANAS (Z-ORDER)
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn set_avatar_layer(window: WebviewWindow, layer: String, target_hwnd: Option<isize>) -> Result<String, String> {
    if !matches!(
        layer.as_str(),
        "top" | "normal" | "bottom" | "behind_active" | "behind_window"
    ) {
        return Err(format!("Capa desconocida: {}", layer));
    }

    IS_TOP_LAYER.store(layer == "top", Ordering::SeqCst);
    if layer != "top" {
        IS_YIELDED_TO_FULLSCREEN.store(false, Ordering::SeqCst);
    }
    if let Ok(mut cur) = CURRENT_LAYER.lock() {
        *cur = Some(layer.clone());
    }

    #[cfg(windows)]
    {
        use win32::*;

        let raw_hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        AVATAR_HWND.store(raw_hwnd, Ordering::SeqCst);

        match layer.as_str() {
            "top" => {
                let _ = window.set_always_on_top(true);
            }
            _ => {
                let _ = window.set_always_on_top(false);
            }
        }

        let layer_for_main = layer.clone();
        let win_for_main = window.clone();
        let _ = win_for_main.run_on_main_thread(move || unsafe {
            match layer_for_main.as_str() {
                "top" => {
                    if let Some(fs_hwnd) = is_fullscreen_app_on_avatar_monitor(raw_hwnd) {
                        IS_YIELDED_TO_FULLSCREEN.store(true, Ordering::SeqCst);
                        SetWindowPos(raw_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        SetWindowPos(raw_hwnd, fs_hwnd, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    } else {
                        IS_YIELDED_TO_FULLSCREEN.store(false, Ordering::SeqCst);
                        SetWindowPos(raw_hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                }
                "normal" => {
                    SetWindowPos(raw_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
                "bottom" => {
                    SetWindowPos(raw_hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
                "behind_active" => {
                    let fg = GetForegroundWindow();
                    if fg != 0 && fg != raw_hwnd {
                        SetWindowPos(raw_hwnd, fg, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    } else {
                        SetWindowPos(raw_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                }
                "behind_window" => {
                    if let Some(target) = target_hwnd {
                        if target != 0 && target != raw_hwnd {
                            SetWindowPos(raw_hwnd, target, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                        }
                    }
                }
                _ => {}
            }

            // Asegurar que tao::apply_diff tras set_always_on_top no borre WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
            if !crate::is_settings_open() {
                apply_overlay_ex_style(raw_hwnd, true);
            }
        });

        let _ = window.emit("avatar://layer-changed", &layer);
        return Ok(layer);
    }

    #[cfg(not(windows))]
    {
        match layer.as_str() {
            "top" => { let _ = window.set_always_on_top(true); }
            _ => { let _ = window.set_always_on_top(false); }
        }
        let _ = window.emit("avatar://layer-changed", &layer);
        Ok(layer)
    }
}

#[tauri::command]
pub fn get_avatar_layer() -> String {
    if let Ok(cur) = CURRENT_LAYER.lock() {
        if let Some(ref l) = *cur {
            return l.clone();
        }
    }
    "top".to_string()
}

/// Restaura la capa guardada (útil tras cerrar ajustes u otras ventanas modales)
pub fn restore_saved_layer(window: &WebviewWindow) {
    let layer = get_avatar_layer();
    let _ = set_avatar_layer(window.clone(), layer, None);
}

// ──────────────────────────────────────────────────────────────────────────────
// LISTADO DE VENTANAS ABIERTAS DEL SISTEMA
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_open_windows(window: WebviewWindow) -> Result<Vec<OpenWindowInfo>, String> {
    #[cfg(windows)]
    {
        use win32::*;

        let self_hwnd = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);

        struct EnumData {
            self_hwnd: isize,
            windows: Vec<OpenWindowInfo>,
        }

        unsafe extern "system" fn enum_proc(hwnd: isize, lparam: isize) -> i32 {
            let data = &mut *(lparam as *mut EnumData);
            if hwnd == data.self_hwnd {
                return 1;
            }
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }

            let mut class_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
            if class_len > 0 {
                let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]).to_lowercase();
                if class_name.contains("tooltip")
                    || class_name.contains("ime")
                    || class_name == "progman"
                    || class_name == "workerw"
                    || class_name.contains("shell_traywnd")
                {
                    return 1;
                }
            }

            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
            if len <= 0 {
                return 1;
            }
            let title = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
            if title.is_empty() || title == "Kyo" || title == "Program Manager" {
                return 1;
            }

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut process_name = String::new();
            if pid != 0 {
                let hproc = OpenProcess(0x1000, 0, pid);
                if hproc != 0 {
                    let mut path_buf = [0u16; 1024];
                    let mut path_len = 1024u32;
                    if QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len) != 0 {
                        let full_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
                        if let Some(name) = std::path::Path::new(&full_path).file_name() {
                            process_name = name.to_string_lossy().to_string();
                        }
                    }
                    CloseHandle(hproc);
                }
            }

            data.windows.push(OpenWindowInfo {
                hwnd,
                title,
                process_name,
            });
            1
        }

        let mut data = EnumData {
            self_hwnd,
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(enum_proc, &mut data as *mut EnumData as isize);
        }
        Ok(data.windows)
    }

    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LÍMITES Y ÁREA DE TRABAJO DEL ESCRITORIO
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_desktop_bounds(window: WebviewWindow) -> Result<DesktopBounds, String> {
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let current_pos = window.outer_position().unwrap_or(tauri::PhysicalPosition::new(0, 0));
    let current_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(220, 220));

    let mut work_left = 0i32;
    let mut work_top = 0i32;
    let mut work_right = 1920i32;
    let mut work_bottom = 1080i32;
    let mut screen_w = 1920u32;
    let mut screen_h = 1080u32;

    #[cfg(windows)]
    {
        use win32::*;
        let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let success = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rc as *mut _ as *mut _, 0)
        };
        if success != 0 && rc.right > rc.left && rc.bottom > rc.top {
            work_left = rc.left;
            work_top = rc.top;
            work_right = rc.right;
            work_bottom = rc.bottom;
        }
    }

    if let Ok(Some(monitor)) = window.current_monitor() {
        let m_size = monitor.size();
        let m_pos = monitor.position();
        screen_w = m_size.width;
        screen_h = m_size.height;

        // Si SystemParametersInfoW no devolvió valores válidos, usar límites del monitor
        if work_right <= work_left || work_bottom <= work_top {
            work_left = m_pos.x;
            work_top = m_pos.y;
            work_right = m_pos.x + m_size.width as i32;
            work_bottom = m_pos.y + m_size.height as i32 - (48.0 * scale_factor) as i32;
        }
    }

    let bounds = DesktopBounds {
        work_area: ScreenRect {
            left: work_left,
            top: work_top,
            right: work_right,
            bottom: work_bottom,
            width: work_right - work_left,
            height: work_bottom - work_top,
        },
        screen_size: ScreenSize {
            width: screen_w,
            height: screen_h,
        },
        scale_factor,
        current_x: current_pos.x,
        current_y: current_pos.y,
        window_width: current_size.width as i32,
        window_height: current_size.height as i32,
    };

    Ok(bounds)
}

// ──────────────────────────────────────────────────────────────────────────────
// DESPLAZAMIENTO SUAVE PROGRAMÁTICO EN X e Y
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn teleport_avatar(window: WebviewWindow, x: i32, y: i32) -> Result<(), String> {
    cancel_active_animation();
    window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    let _ = window.emit("avatar://moved", serde_json::json!({ "x": x, "y": y }));
    Ok(())
}

#[tauri::command]
pub fn stop_avatar_movement() -> Result<(), String> {
    cancel_active_animation();
    Ok(())
}

#[tauri::command]
pub fn is_avatar_moving() -> bool {
    IS_AVATAR_ANIMATING.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn set_avatar_ignore_cursor(window: WebviewWindow, ignore: bool) -> Result<(), String> {
    window.set_ignore_cursor_events(ignore).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn animate_avatar_to(
    window: WebviewWindow,
    target_x: i32,
    target_y: i32,
    duration_ms: u64,
    easing: Option<String>,
) -> Result<(), String> {
    let start_pos = window.outer_position().map_err(|e| e.to_string())?;
    let start_x = start_pos.x;
    let start_y = start_pos.y;

    if start_x == target_x && start_y == target_y {
        return Ok(());
    }

    let dur_ms = duration_ms.max(60);
    let gen = MOVEMENT_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    IS_AVATAR_ANIMATING.store(true, Ordering::SeqCst);

    let win_clone = window.clone();
    let easing_mode = easing.unwrap_or_else(|| "smooth".to_string());

    std::thread::spawn(move || {
        let start_time = Instant::now();
        let total_duration = Duration::from_millis(dur_ms);

        let mut last_emitted_x = start_x;
        let mut last_emitted_y = start_y;

        loop {
            // Verificar si fue cancelado por una nueva animación
            if MOVEMENT_GENERATION.load(Ordering::SeqCst) != gen {
                break;
            }

            // Verificar si el usuario empezó a arrastrar manualmente el avatar
            if crate::is_avatar_dragging() {
                break;
            }

            let elapsed = start_time.elapsed();
            let raw_t = (elapsed.as_secs_f64() / total_duration.as_secs_f64()).min(1.0);

            let eased_t = match easing_mode.as_str() {
                "linear" => raw_t,
                "ease_in" => raw_t * raw_t,
                "ease_out" => raw_t * (2.0 - raw_t),
                "bounce" => {
                    if raw_t < 0.85 {
                        (raw_t / 0.85).powi(2) * 1.04
                    } else {
                        let sub = (raw_t - 0.85) / 0.15;
                        1.04 - 0.04 * (sub * std::f64::consts::PI).sin()
                    }
                }
                "spring" => {
                    // Simulación ligera resorte sin sobregiro excesivo
                    1.0 - (-5.0 * raw_t).exp() * (1.0 - raw_t)
                }
                // Por defecto: "smooth" (Cubic Ease In-Out suave)
                _ => {
                    if raw_t < 0.5 {
                        4.0 * raw_t * raw_t * raw_t
                    } else {
                        1.0 - (-2.0 * raw_t + 2.0).powi(3) / 2.0
                    }
                }
            };

            let cur_x = (start_x as f64 + (target_x - start_x) as f64 * eased_t).round() as i32;
            let cur_y = (start_y as f64 + (target_y - start_y) as f64 * eased_t).round() as i32;

            if cur_x != last_emitted_x || cur_y != last_emitted_y {
                let _ = win_clone.set_position(tauri::PhysicalPosition::new(cur_x, cur_y));

                let vx = (cur_x - last_emitted_x) as f64;
                let vy = (cur_y - last_emitted_y) as f64;

                let _ = win_clone.emit("avatar://moving", MovementTick {
                    x: cur_x,
                    y: cur_y,
                    vx,
                    vy,
                    progress: raw_t,
                    is_moving: raw_t < 1.0,
                });

                last_emitted_x = cur_x;
                last_emitted_y = cur_y;
            }

            if raw_t >= 1.0 {
                break;
            }

            std::thread::sleep(Duration::from_millis(16));
        }

        if MOVEMENT_GENERATION.load(Ordering::SeqCst) == gen {
            IS_AVATAR_ANIMATING.store(false, Ordering::SeqCst);
            let _ = win_clone.set_position(tauri::PhysicalPosition::new(target_x, target_y));
            let _ = win_clone.emit("avatar://moving", MovementTick {
                x: target_x,
                y: target_y,
                vx: 0.0,
                vy: 0.0,
                progress: 1.0,
                is_moving: false
            });
            let _ = win_clone.emit("avatar://moved", serde_json::json!({ "x": target_x, "y": target_y }));
        }
    });

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// POSICIONAMIENTO POR ANCLAJES RÁPIDOS
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn position_window_anchor(
    window: WebviewWindow,
    anchor: String,
    animate: Option<bool>,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    let bounds = get_desktop_bounds(window.clone())?;
    let scale = bounds.scale_factor;
    let win_w = bounds.window_width;
    let win_h = bounds.window_height;
    let margin_x = (24.0 * scale) as i32;
    let margin_y = (24.0 * scale) as i32;

    let (target_x, target_y) = match anchor.as_str() {
        "bottom-right" => (
            bounds.work_area.right - win_w - margin_x,
            bounds.work_area.bottom - win_h - margin_y,
        ),
        "bottom-left" => (
            bounds.work_area.left + margin_x,
            bounds.work_area.bottom - win_h - margin_y,
        ),
        "top-right" => (
            bounds.work_area.right - win_w - margin_x,
            bounds.work_area.top + margin_y,
        ),
        "top-left" => (
            bounds.work_area.left + margin_x,
            bounds.work_area.top + margin_y,
        ),
        "center" => (
            bounds.work_area.left + (bounds.work_area.width - win_w) / 2,
            bounds.work_area.top + (bounds.work_area.height - win_h) / 2,
        ),
        "bottom-center" => (
            bounds.work_area.left + (bounds.work_area.width - win_w) / 2,
            bounds.work_area.bottom - win_h - margin_y,
        ),
        _ => return Err(format!("Anclaje desconocido: {}", anchor)),
    };

    if animate.unwrap_or(true) {
        animate_avatar_to(window, target_x, target_y, duration_ms.unwrap_or(750), Some("smooth".to_string()))
    } else {
        teleport_avatar(window, target_x, target_y)
    }
}
