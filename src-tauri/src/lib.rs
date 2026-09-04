use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WebviewWindow, WindowEvent,
};

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
        .text("open_chat", "💬 Abrir Chat")
        .separator()
        .text("options", "⚙️ Opciones")
        .separator()
        .text("quit", "Cerrar kyo")
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

// Comando para cambiar entre modo avatar (220x220) y modo ajustes (920x640)
#[tauri::command]
async fn set_window_mode(window: WebviewWindow, mode: String) -> Result<(), String> {
    match mode.as_str() {
        "settings" => {
            let _ = window.set_resizable(true);
            let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
            let _ = window.center();
            let _ = window.set_focus();
        }
        "avatar" => {
            let _ = window.set_size(tauri::LogicalSize::new(220.0, 220.0));
        }
        _ => {}
    }
    Ok(())
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

    type HWND = isize;
    type BOOL = i32;

    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> HWND;
        fn IsWindow(hWnd: HWND) -> BOOL;
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
    }

    const SW_RESTORE: i32 = 9;
    const SW_SHOW: i32 = 5;

    unsafe extern "system" fn enum_win_proc(hwnd: HWND, lparam: isize) -> BOOL {
        let out_hwnd = lparam as *mut HWND;
        let mut buffer = [0u16; 256];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), 256);
        if len > 0 {
            let text = String::from_utf16_lossy(&buffer[..len as usize]);
            if text.contains("Dioxus Chat") {
                *out_hwnd = hwnd;
                return 0; // detener enumeración
            }
        }
        1 // continuar
    }

    pub unsafe fn find_dioxus_chat_window() -> Option<HWND> {
        // 1. Búsqueda exacta por título
        let wide_title: Vec<u16> = OsStr::new("Dioxus Chat")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let direct = FindWindowW(std::ptr::null(), wide_title.as_ptr());
        if direct != 0 && IsWindow(direct) != 0 {
            return Some(direct);
        }

        // 2. Fallback: enumerar ventanas de nivel superior
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

        // 1. Relativo al ejecutable actual
        if let Ok(mut cur) = std::env::current_exe() {
            for _ in 0..5 {
                if cur.pop() {
                    candidates.push(cur.join("dioxus-chat/target/dx/desktop/debug/windows/app/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/release/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/target/debug/desktop.exe"));
                    candidates.push(cur.join("dioxus-chat/desktop.exe"));
                }
            }
        }

        // 2. Relativo al directorio de trabajo
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("dioxus-chat/target/dx/desktop/debug/windows/app/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/target/release/desktop.exe"));
            candidates.push(cwd.join("dioxus-chat/target/debug/desktop.exe"));
        }

        // 3. Rutas fijas de desarrollo
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\dx\desktop\debug\windows\app\desktop.exe"));
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\debug\desktop.exe"));
        candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\release\desktop.exe"));

        for p in candidates {
            if p.exists() && p.is_file() {
                return Some(p);
            }
        }

        None
    }

    pub fn launch_dioxus_chat() -> Result<(), String> {
        let exe = resolve_dioxus_chat_exe()
            .ok_or_else(|| "No se encontró el ejecutable desktop.exe de Dioxus Chat".to_string())?;

        let parent = exe.parent().unwrap_or(Path::new("."));
        Command::new(&exe)
            .current_dir(parent)
            .spawn()
            .map_err(|e| format!("Error al iniciar {}: {}", exe.display(), e))?;

        // Monitoreo en segundo plano para enfocar la ventana cuando termine de iniciar
        std::thread::spawn(|| {
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(150));
                unsafe {
                    if let Some(hwnd) = find_dioxus_chat_window() {
                        focus_window(hwnd);
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}

/// Comando invocado desde el frontend al hacer doble clic sobre el avatar
#[tauri::command]
async fn open_dioxus_chat() -> Result<String, String> {
    #[cfg(windows)]
    {
        unsafe {
            if let Some(hwnd) = win32_chat::find_dioxus_chat_window() {
                win32_chat::focus_window(hwnd);
                return Ok("focused".to_string());
            }
        }
        win32_chat::launch_dioxus_chat()?;
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
        .invoke_handler(tauri::generate_handler![
            get_cursor_relative,
            show_context_menu,
            show_avatar_menu,
            exit_app,
            set_window_mode,
            open_dioxus_chat
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_chat" | "ctx-chat" => {
                tauri::async_runtime::spawn(async {
                    let _ = open_dioxus_chat().await;
                });
            }
            "options" | "ctx-settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_resizable(true);
                    let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
                    let _ = window.center();
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
            let chat_item = MenuItem::with_id(app, "open_chat", "💬 Abrir Chat", true, None::<&str>)?;
            let opt_item = MenuItem::with_id(app, "options", "⚙️ Opciones", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Cerrar kyo", true, None::<&str>)?;

            // 2. Construir menú
            let menu = Menu::with_items(app, &[&chat_item, &opt_item, &separator, &quit_item])?;

            // 3. Configurar Tray Icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false) // Clic derecho abre el menú
                .tooltip("kyo")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open_chat" | "ctx-chat" => {
                        tauri::async_runtime::spawn(async {
                            let _ = open_dioxus_chat().await;
                        });
                    }
                    "options" | "ctx-settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_resizable(true);
                            let _ = window.set_size(tauri::LogicalSize::new(920.0, 640.0));
                            let _ = window.center();
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
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // 4. Mostrar y enfocar la ventana principal de forma asíncrona
            if let Some(window) = app.get_webview_window("main") {
                tauri::async_runtime::spawn(async move {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Interceptar la 'X' para minimizar a bandeja en lugar de salir
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
