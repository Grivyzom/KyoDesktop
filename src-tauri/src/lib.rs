use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WebviewWindow, WindowEvent,
};

pub mod sidecar;
pub mod font_manager;

// ──────────────────────────────────────────────────────────────────────────────
// Comando: posición del cursor relativa al centro del avatar (multi-monitor).
// ──────────────────────────────────────────────────────────────────────────────
#[tauri::command]
fn get_cursor_relative(window: tauri::Window) -> (f64, f64) {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        extern "system" {
            fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        }
        let mut pt = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut pt);
        }

        if let (Ok(win_pos), Ok(scale), Ok(size)) = (
            window.outer_position(),
            window.scale_factor(),
            window.outer_size(),
        ) {
            let center_x = win_pos.x as f64 + (size.width as f64 / 2.0);
            let center_y = win_pos.y as f64 + (size.height as f64 / 2.0);
            let dx = (pt.x as f64 - center_x) / scale;
            let dy = (pt.y as f64 - center_y) / scale;
            return (dx, dy);
        }
    }
    (0.0, 0.0)
}

// ──────────────────────────────────────────────────────────────────────────────
// Comando: muestra el menú contextual nativo desde Rust.
// Renderizado por Win32 fuera del WebView → nunca transparente ni recortado.
// ──────────────────────────────────────────────────────────────────────────────
#[tauri::command]
async fn show_context_menu(window: WebviewWindow) -> Result<(), String> {
    let menu = MenuBuilder::new(&window)
        .text("open_chat", "Abrir chat")
        .separator()
        .text("options", "Opciones")
        .separator()
        .text("quit", "Cerrar Kyo")
        .build()
        .map_err(|e| e.to_string())?;

    window.popup_menu(&menu).map_err(|e| e.to_string())?;

    Ok(())
}

// Alias de compatibilidad por si el frontend invoca `show_avatar_menu`
#[tauri::command]
async fn show_avatar_menu(window: WebviewWindow) -> Result<(), String> {
    show_context_menu(window).await
}

// Comando para cerrar la aplicación completamente desde el frontend
#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// Bandera atómica para indicar si la ventana está en modo ajustes (expandida)
static IS_SETTINGS_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Comando para cambiar entre modo avatar (220x220) y modo ajustes (920x640)
#[tauri::command]
async fn set_window_mode(window: WebviewWindow, mode: String) -> Result<(), String> {
    match mode.as_str() {
        "settings" => {
            IS_SETTINGS_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = window.set_resizable(true);
            let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
            let _ = window.set_max_size(None::<tauri::LogicalSize<f64>>);
            let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
            let _ = window.center();
            let _ = window.set_skip_taskbar(false);
            let _ = window.set_focus();
        }
        "avatar" => {
            IS_SETTINGS_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = window.set_skip_taskbar(true);
            let _ = window.set_size(tauri::LogicalSize::new(220.0, 220.0));
            let _ = window.set_min_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
            let _ = window.set_max_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
            let _ = window.set_resizable(false);
        }
        _ => {}
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Arrastre suave y personalizado del avatar sin activar Aero Snap ni FancyZones
// ──────────────────────────────────────────────────────────────────────────────
static IS_AVATAR_DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn start_avatar_drag(window: WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        if IS_AVATAR_DRAGGING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Ok(());
        }

        std::thread::spawn(move || {
            struct DragGuard;
            impl Drop for DragGuard {
                fn drop(&mut self) {
                    IS_AVATAR_DRAGGING.store(false, Ordering::SeqCst);
                }
            }
            let _guard = DragGuard;

            #[repr(C)]
            struct POINT {
                x: i32,
                y: i32,
            }
            extern "system" {
                fn GetCursorPos(lpPoint: *mut POINT) -> i32;
                fn GetAsyncKeyState(vKey: i32) -> i16;
            }

            const VK_LBUTTON: i32 = 0x01;
            const VK_ESCAPE: i32 = 0x1B;

            let mut pt_start = POINT { x: 0, y: 0 };
            unsafe {
                GetCursorPos(&mut pt_start);
            }

            let win_start = match window.outer_position() {
                Ok(pos) => pos,
                Err(_) => return,
            };

            let mut last_x = win_start.x;
            let mut last_y = win_start.y;

            loop {
                // Verificar si el botón izquierdo del ratón fue liberado
                let lbutton = unsafe { GetAsyncKeyState(VK_LBUTTON) };
                if (lbutton as u16 & 0x8000) == 0 {
                    break;
                }

                // Cancelar si se presiona Escape
                let esc = unsafe { GetAsyncKeyState(VK_ESCAPE) };
                if (esc as u16 & 0x8000) != 0 {
                    break;
                }

                let mut pt_cur = POINT { x: 0, y: 0 };
                unsafe {
                    GetCursorPos(&mut pt_cur);
                }

                let dx = pt_cur.x - pt_start.x;
                let dy = pt_cur.y - pt_start.y;

                let target_x = win_start.x + dx;
                let target_y = win_start.y + dy;

                if target_x != last_x || target_y != last_y {
                    let _ = window.set_position(tauri::PhysicalPosition::new(target_x, target_y));
                    last_x = target_x;
                    last_y = target_y;
                }

                std::thread::sleep(Duration::from_millis(4));
            }
        });

        return Ok(());
    }

    #[cfg(not(windows))]
    {
        window.start_dragging().map_err(|e| e.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Posicionar la ventana en la esquina inferior derecha del área de trabajo
// (respetando la barra de tareas de Windows)
// ──────────────────────────────────────────────────────────────────────────────
pub fn position_window_bottom_right(window: &WebviewWindow) {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct RECT {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        extern "system" {
            fn SystemParametersInfoW(uiAction: u32, uiParam: u32, pvParam: *mut std::ffi::c_void, fWinIni: u32) -> i32;
        }
        const SPI_GETWORKAREA: u32 = 0x0030;

        let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let success = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rc as *mut _ as *mut _, 0)
        };

        if success != 0 && rc.right > rc.left && rc.bottom > rc.top {
            let scale = window.scale_factor().unwrap_or(1.0);
            let win_w = (220.0 * scale) as i32;
            let win_h = (220.0 * scale) as i32;
            let margin_x = (24.0 * scale) as i32;
            let margin_y = (24.0 * scale) as i32;

            let x = rc.right - win_w - margin_x;
            let y = rc.bottom - win_h - margin_y;

            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            return;
        }
    }

    if let Ok(Some(monitor)) = window.current_monitor() {
        let screen_size = monitor.size();
        let scale = monitor.scale_factor();
        let win_w = (220.0 * scale) as i32;
        let win_h = (220.0 * scale) as i32;
        let margin = (24.0 * scale) as i32;

        let x = screen_size.width as i32 - win_w - margin;
        let y = screen_size.height as i32 - win_h - margin - (48.0 * scale) as i32;

        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

#[tauri::command]
fn set_position_bottom_right(window: WebviewWindow) -> Result<(), String> {
    position_window_bottom_right(&window);
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Comandos de Gestión de Sesión y Token JWT de Kyo
// ──────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct SessionConfig {
    pub token: Option<String>,
    pub server_url: Option<String>,
    pub active_profile_id: Option<String>,
    pub active_profile_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub updated_at: Option<u64>,
}

#[tauri::command]
fn save_session_config(
    token: String,
    server_url: Option<String>,
    active_profile_id: Option<String>,
    active_profile_name: Option<String>,
    username: Option<String>,
    email: Option<String>,
) -> Result<(), String> {
    let trimmed_token = token.trim().to_string();
    std::env::set_var("KYO_AUTH_TOKEN", &trimmed_token);

    let trimmed_url = server_url.as_ref().map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    if let Some(ref url) = trimmed_url {
        std::env::set_var("KYO_UPSTREAM", url);
        std::env::set_var("KYO_SERVER_URL", url);
    }

    if let Some(ref pid) = active_profile_id {
        std::env::set_var("KYO_ACTIVE_PROFILE_ID", pid);
    }
    if let Some(ref pname) = active_profile_name {
        std::env::set_var("KYO_ACTIVE_PROFILE_NAME", pname);
    }
    if let Some(ref u) = username {
        std::env::set_var("KYO_USERNAME", u);
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = std::path::Path::new(&appdata).join("kyo");
        let _ = std::fs::create_dir_all(&kyo_dir);
        let session_file = kyo_dir.join("session.json");

        let mut map: serde_json::Map<String, serde_json::Value> = if session_file.is_file() {
            std::fs::read_to_string(&session_file)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            serde_json::Map::new()
        };

        map.insert("token".to_string(), serde_json::Value::String(trimmed_token));
        if let Some(url) = trimmed_url {
            map.insert("server_url".to_string(), serde_json::Value::String(url));
        }
        if let Some(pid) = active_profile_id {
            map.insert("active_profile_id".to_string(), serde_json::Value::String(pid));
        }
        if let Some(pname) = active_profile_name {
            map.insert("active_profile_name".to_string(), serde_json::Value::String(pname));
        }
        if let Some(u) = username {
            map.insert("username".to_string(), serde_json::Value::String(u));
        }
        if let Some(e) = email {
            map.insert("email".to_string(), serde_json::Value::String(e));
        }
        map.insert(
            "updated_at".to_string(),
            serde_json::json!(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
        );

        std::fs::write(&session_file, serde_json::Value::Object(map).to_string())
            .map_err(|e| format!("Error escribiendo session.json: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
fn save_session_token(token: String) -> Result<(), String> {
    save_session_config(token, None, None, None, None, None)
}

#[tauri::command]
fn get_session_config() -> SessionConfig {
    let mut config = SessionConfig::default();

    if let Ok(token) = std::env::var("KYO_AUTH_TOKEN") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            config.token = Some(trimmed);
        }
    }
    if let Ok(url) = std::env::var("KYO_UPSTREAM").or_else(|_| std::env::var("KYO_SERVER_URL")) {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() {
            config.server_url = Some(trimmed);
        }
    }
    if let Ok(pid) = std::env::var("KYO_ACTIVE_PROFILE_ID") {
        let trimmed = pid.trim().to_string();
        if !trimmed.is_empty() {
            config.active_profile_id = Some(trimmed);
        }
    }
    if let Ok(pname) = std::env::var("KYO_ACTIVE_PROFILE_NAME") {
        let trimmed = pname.trim().to_string();
        if !trimmed.is_empty() {
            config.active_profile_name = Some(trimmed);
        }
    }
    if let Ok(u) = std::env::var("KYO_USERNAME") {
        let trimmed = u.trim().to_string();
        if !trimmed.is_empty() {
            config.username = Some(trimmed);
        }
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = std::path::Path::new(&appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if config.token.is_none() {
                        if let Some(token) = json.get("token").or_else(|| json.get("access_token")).and_then(|t| t.as_str()) {
                            config.token = Some(token.to_string());
                        }
                    }
                    if config.server_url.is_none() {
                        if let Some(url) = json.get("server_url").or_else(|| json.get("upstream")).and_then(|u| u.as_str()) {
                            config.server_url = Some(url.to_string());
                        }
                    }
                    if config.active_profile_id.is_none() {
                        if let Some(pid) = json.get("active_profile_id").and_then(|p| p.as_str()) {
                            config.active_profile_id = Some(pid.to_string());
                        }
                    }
                    if config.active_profile_name.is_none() {
                        if let Some(pname) = json.get("active_profile_name").and_then(|p| p.as_str()) {
                            config.active_profile_name = Some(pname.to_string());
                        }
                    }
                    if config.username.is_none() {
                        if let Some(u) = json.get("username").and_then(|p| p.as_str()) {
                            config.username = Some(u.to_string());
                        }
                    }
                    if config.email.is_none() {
                        if let Some(e) = json.get("email").and_then(|p| p.as_str()) {
                            config.email = Some(e.to_string());
                        }
                    }
                    if let Some(ts) = json.get("updated_at").and_then(|v| v.as_u64()) {
                        config.updated_at = Some(ts);
                    }
                }
            }
        }
    }

    config
}

#[tauri::command]
fn get_session_token() -> Option<String> {
    get_session_config().token
}

// ──────────────────────────────────────────────────────────────────────────────
// ──────────────────────────────────────────────────────────────────────────────
// Restaurar el avatar flotante a su tamaño y estado original (220x220)
// ──────────────────────────────────────────────────────────────────────────────
pub fn restore_avatar(app: &tauri::AppHandle) {
    IS_SETTINGS_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_resizable(true);
        let _ = window.set_size(tauri::LogicalSize::new(220.0, 220.0));
        let _ = window.set_min_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
        let _ = window.set_max_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
        let _ = window.set_resizable(false);
        let _ = window.set_skip_taskbar(true);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.emit("restore-avatar", ());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Conexión con Dioxus Chat: Detección, Enfoque y Lanzamiento de Ventana
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(windows)]
mod win32_chat {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    type HWND = isize;
    type BOOL = i32;

    static SPAWNED_PID: AtomicU32 = AtomicU32::new(0);

    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> HWND;
        fn IsWindow(hWnd: HWND) -> BOOL;
        fn IsWindowVisible(hWnd: HWND) -> BOOL;
        fn IsIconic(hWnd: HWND) -> BOOL;
        fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        fn SetForegroundWindow(hWnd: HWND) -> BOOL;
        fn BringWindowToTop(hWnd: HWND) -> BOOL;
        fn GetForegroundWindow() -> HWND;
        fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut u32) -> u32;
        fn GetCurrentThreadId() -> u32;
        fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: BOOL) -> BOOL;
        fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(HWND, isize) -> BOOL, lParam: isize) -> BOOL;
        fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: BOOL, dwProcessId: u32) -> isize;
        fn CloseHandle(hObject: isize) -> BOOL;
        fn QueryFullProcessImageNameW(hProcess: isize, dwFlags: u32, lpExeName: *mut u16, lpdwSize: *mut u32) -> BOOL;
    }

    const SW_RESTORE: i32 = 9;
    const SW_SHOW: i32 = 5;

    unsafe extern "system" fn enum_win_proc(hwnd: HWND, lparam: isize) -> BOOL {
        let out_hwnd = lparam as *mut HWND;

        // Descartar ventanas no visibles
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }

        // Descartar clases auxiliares comunes de Windows y frameworks GUI (tooltips, event targets, IME)
        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
        if class_len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]).to_lowercase();
            if class_name.contains("tooltip") || class_name.contains("target") || class_name.contains("ime") {
                return 1;
            }
        }

        // 1. Detección por título de ventana
        let mut buffer = [0u16; 256];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), 256);
        if len > 0 {
            let text = String::from_utf16_lossy(&buffer[..len as usize]);
            if text == "Kyo Desktop" || text.contains("Kyo Desktop") || text.contains("Dioxus Chat") || text.contains("Dioxus") {
                *out_hwnd = hwnd;
                return 0; // detener enumeración
            }
        }

        // 2. Detección por PID del proceso hijo lanzado
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let spawned = SPAWNED_PID.load(Ordering::SeqCst);
        if spawned != 0 && pid == spawned {
            *out_hwnd = hwnd;
            return 0;
        }

        // 3. Detección por proceso ejecutable ("desktop.exe")
        if pid != 0 {
            const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
            let hproc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if hproc != 0 {
                let mut path_buf = [0u16; 1024];
                let mut size: u32 = 1024;
                if QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
                    let exe_path = String::from_utf16_lossy(&path_buf[..size as usize]).to_lowercase();
                    let is_target = exe_path.ends_with("desktop.exe") || exe_path.contains("dioxus-chat");
                    let is_tauri_binary = exe_path.ends_with("kyo-agent.exe") || exe_path.ends_with("tauri-app.exe");
                    if is_target && !is_tauri_binary && !exe_path.contains("explorer.exe") {
                        *out_hwnd = hwnd;
                        CloseHandle(hproc);
                        return 0;
                    }
                }
                CloseHandle(hproc);
            }
        }

        1 // continuar
    }

    pub unsafe fn find_dioxus_chat_window() -> Option<HWND> {
        // 1. Búsqueda exacta por título: "Kyo Desktop"
        let wide_title_kyo: Vec<u16> = OsStr::new("Kyo Desktop")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let direct_k = FindWindowW(std::ptr::null(), wide_title_kyo.as_ptr());
        if direct_k != 0 && IsWindow(direct_k) != 0 && IsWindowVisible(direct_k) != 0 {
            return Some(direct_k);
        }

        // 2. Búsqueda alternativa por título: "Dioxus Chat"
        let wide_title_d: Vec<u16> = OsStr::new("Dioxus Chat")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let direct_d = FindWindowW(std::ptr::null(), wide_title_d.as_ptr());
        if direct_d != 0 && IsWindow(direct_d) != 0 && IsWindowVisible(direct_d) != 0 {
            return Some(direct_d);
        }

        // 3. Fallback: enumerar ventanas visibles de nivel superior
        let mut found_hwnd: HWND = 0;
        EnumWindows(enum_win_proc, &mut found_hwnd as *mut HWND as isize);
        if found_hwnd != 0 && IsWindow(found_hwnd) != 0 {
            return Some(found_hwnd);
        }

        None
    }

    pub unsafe fn focus_window(hwnd: HWND) {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }

        let fg_hwnd = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut());
        let cur_thread = GetCurrentThreadId();

        if fg_thread != 0 && fg_thread != cur_thread {
            AttachThreadInput(cur_thread, fg_thread, 1);
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            AttachThreadInput(cur_thread, fg_thread, 0);
        } else {
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
        }
    }

    pub fn resolve_dioxus_chat_exe() -> Option<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Relativo al ejecutable actual (mismo directorio o ancestros)
        if let Ok(cur_exe) = std::env::current_exe() {
            if let Some(exe_dir) = cur_exe.parent() {
                candidates.push(exe_dir.join("desktop.exe"));
                candidates.push(exe_dir.join("dioxus-chat/target/release/desktop.exe"));
                candidates.push(exe_dir.join("dioxus-chat/desktop.exe"));
            }
            let mut cur = cur_exe.clone();
            for _ in 0..5 {
                if cur.pop() {
                    candidates.push(cur.join("desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/release/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/debug/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/dx/desktop/debug/windows/app/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/dx/desktop/release/windows/app/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/desktop.exe"));
                }
            }
        }

        // 2. Relativo al directorio de trabajo
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("dioxus-chat/target/release/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/target/debug/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/target/dx/desktop/debug/windows/app/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/target/dx/desktop/release/windows/app/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/desktop.exe"));
        }

        // 3. Rutas fijas de desarrollo
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\release\desktop.exe"));
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\debug\desktop.exe"));
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\dx\desktop\debug\windows\app\desktop.exe"));

        // Filtrar solo los ejecutables existentes
        let mut existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.is_file()).collect();

        // Priorizar el binario más recientemente compilado
        existing.sort_by(|a, b| {
            let time_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let time_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            time_b.cmp(&time_a)
        });

        existing.into_iter().next()
    }

    pub fn launch_dioxus_chat(app: &tauri::AppHandle) -> Result<(), String> {
        let exe = resolve_dioxus_chat_exe()
            .ok_or_else(|| "No se encontró el ejecutable desktop.exe de Dioxus Chat".to_string())?;

        let parent = exe.parent().unwrap_or(Path::new("."));
        let mut cmd = Command::new(&exe);
        cmd.current_dir(parent);

        // Si el sidecar Go ya tiene puerto asignado, inyectarlo en el entorno
        if let Some(port) = crate::sidecar::get_sidecar_port(app) {
            cmd.env("KYO_SIDECAR_PORT", port.to_string());
        }

        // Si existe configuración de sesión guardada, inyectarla en el entorno
        let cfg = crate::get_session_config();
        if let Some(token) = cfg.token {
            cmd.env("KYO_AUTH_TOKEN", token);
        }
        if let Some(url) = cfg.server_url {
            cmd.env("KYO_UPSTREAM", &url);
            cmd.env("KYO_SERVER_URL", &url);
        }
        if let Some(pid) = cfg.active_profile_id {
            cmd.env("KYO_ACTIVE_PROFILE_ID", pid);
        }
        if let Some(pname) = cfg.active_profile_name {
            cmd.env("KYO_ACTIVE_PROFILE_NAME", pname);
        }
        if let Some(u) = cfg.username {
            cmd.env("KYO_USERNAME", u);
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const DETACHED_PROCESS: u32 = 0x00000008;
            cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        }

        let child = cmd.spawn()
            .map_err(|e| format!("Error al iniciar {}: {}", exe.display(), e))?;

        SPAWNED_PID.store(child.id(), Ordering::SeqCst);

        Ok(())
    }

    pub fn start_chat_lifecycle_watcher(app: tauri::AppHandle, hwnd_opt: Option<HWND>) {
        static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);
        let current_session = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

        std::thread::spawn(move || {
            let target_hwnd = match hwnd_opt {
                Some(h) => h,
                None => {
                    let mut found = None;
                    for _ in 0..60 {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        if SESSION_COUNTER.load(Ordering::SeqCst) != current_session {
                            return;
                        }
                        unsafe {
                            if let Some(h) = find_dioxus_chat_window() {
                                focus_window(h);
                                found = Some(h);
                                break;
                            }
                        }
                    }
                    match found {
                        Some(h) => h,
                        None => {
                            // Si Dioxus Chat no logró abrirse, restaurar el avatar
                            if SESSION_COUNTER.load(Ordering::SeqCst) == current_session {
                                crate::restore_avatar(&app);
                            }
                            return;
                        }
                    }
                }
            };

            // Monitorear activamente mientras la ventana de Dioxus Chat permanezca abierta
            loop {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if SESSION_COUNTER.load(Ordering::SeqCst) != current_session {
                    return;
                }
                unsafe {
                    if IsWindow(target_hwnd) == 0 {
                        break; // La ventana fue cerrada (X, Alt+F4, o proceso terminado)
                    }
                }
            }

            // Dioxus Chat cerrado: restaurar y hacer reaparecer el agente automáticamente
            if SESSION_COUNTER.load(Ordering::SeqCst) == current_session {
                SPAWNED_PID.store(0, Ordering::SeqCst);
                crate::restore_avatar(&app);
            }
        });
    }
}

/// Comando invocado desde el frontend al hacer doble clic sobre el avatar o desde menús
#[tauri::command]
async fn open_dioxus_chat(app: tauri::AppHandle) -> Result<String, String> {
    // 1. Ocultar el agente inmediatamente al abrir el chat
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    #[cfg(windows)]
    {
        unsafe {
            if let Some(hwnd) = win32_chat::find_dioxus_chat_window() {
                win32_chat::focus_window(hwnd);
                win32_chat::start_chat_lifecycle_watcher(app.clone(), Some(hwnd));
                return Ok("focused".to_string());
            }
        }
        win32_chat::launch_dioxus_chat(&app)?;
        win32_chat::start_chat_lifecycle_watcher(app.clone(), None);
        Ok("launched".to_string())
    }
    #[cfg(not(windows))]
    {
        Err("Plataforma no soportada".to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Punto de entrada de la aplicación.
// ──────────────────────────────────────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(sidecar::SidecarState::default())
        .invoke_handler(tauri::generate_handler![
            get_cursor_relative,
            show_context_menu,
            show_avatar_menu,
            exit_app,
            set_window_mode,
            start_avatar_drag,
            set_position_bottom_right,
            open_dioxus_chat,
            sidecar::sidecar_port,
            save_session_token,
            get_session_token,
            save_session_config,
            get_session_config,
            font_manager::get_system_fonts,
            font_manager::install_bundled_font,
            font_manager::save_font_preference,
            font_manager::get_font_preference
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_chat" | "ctx-chat" => {
                let app_handle = app.clone();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("request-open-chat", ());
                } else {
                    tauri::async_runtime::spawn(async move {
                        let _ = open_dioxus_chat(app_handle).await;
                    });
                }
            }
            "options" | "ctx-settings" => {
                IS_SETTINGS_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_resizable(true);
                    let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
                    let _ = window.set_max_size(None::<tauri::LogicalSize<f64>>);
                    let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
                    let _ = window.center();
                    let _ = window.set_skip_taskbar(false);
                    let _ = window.set_focus();
                    let _ = window.emit("open-settings", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .setup(|app| {
            // 1. Crear items del menú contextual de la bandeja
            let chat_item = MenuItem::with_id(app, "open_chat", "Abrir chat", true, None::<&str>)?;
            let opt_item = MenuItem::with_id(app, "options", "Opciones", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Cerrar Kyo", true, None::<&str>)?;

            // 2. Construir menú
            let menu = Menu::with_items(app, &[&chat_item, &opt_item, &separator, &quit_item])?;

            // 3. Configurar Tray Icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false) // Clic derecho abre el menú
                .tooltip("Kyo")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open_chat" | "ctx-chat" => {
                        let app_handle = app.clone();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.emit("request-open-chat", ());
                        } else {
                            tauri::async_runtime::spawn(async move {
                                let _ = open_dioxus_chat(app_handle).await;
                            });
                        }
                    }
                    "options" | "ctx-settings" => {
                        IS_SETTINGS_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_resizable(true);
                            let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
                            let _ = window.set_max_size(None::<tauri::LogicalSize<f64>>);
                            let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
                            let _ = window.center();
                            let _ = window.set_skip_taskbar(false);
                            let _ = window.set_focus();
                            let _ = window.emit("open-settings", ());
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        restore_avatar(app);
                    }
                })
                .build(app)?;

            // 4. Posicionar por defecto en la esquina inferior derecha y asegurar modo avatar fijo (no redimensionable)
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() {
                    let _ = window.set_icon(icon.clone());
                }
                position_window_bottom_right(&window);
                let _ = window.set_min_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
                let _ = window.set_max_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
                let _ = window.set_resizable(false);
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }

            // 5. Iniciar subproceso Go Sidecar de forma asíncrona
            sidecar::spawn(&app.handle());

            // 6. Instalar la fuente empaquetada a nivel de usuario en segundo plano
            tauri::async_runtime::spawn(async move {
                if let Err(e) = font_manager::install_bundled_font() {
                    eprintln!("[font_manager] Error autoinstalando fuente: {e}");
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Interceptar la 'X' para minimizar a bandeja en lugar de salir
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                let _ = window.emit("restore-avatar", ());
            }
            // Proteger el contenedor del avatar: si el SO intenta redimensionarlo fuera de ajustes, restaurar 220x220
            if let WindowEvent::Resized(size) = event {
                if !IS_SETTINGS_OPEN.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Ok(scale) = window.scale_factor() {
                        let expected_w = (220.0 * scale).round() as u32;
                        let expected_h = (220.0 * scale).round() as u32;
                        if (size.width as i32 - expected_w as i32).abs() > 6 || (size.height as i32 - expected_h as i32).abs() > 6 {
                            let _ = window.set_size(tauri::LogicalSize::new(220.0, 220.0));
                            let _ = window.set_min_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
                            let _ = window.set_max_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
                            let _ = window.set_resizable(false);
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
