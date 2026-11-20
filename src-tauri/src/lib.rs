use tauri::{
    menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WebviewWindow, WindowEvent,
};

pub mod sidecar;
pub mod font_manager;
pub mod avatar_desktop;
pub mod music_detector;

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
    let layer_sub = SubmenuBuilder::new(&window, "Capa de ventana")
        .text("layer_top", "▲ Siempre visible (Superior)")
        .text("layer_normal", "■ Entre ventanas abiertas (Normal)")
        .text("layer_bottom", "▼ Fondo de escritorio")
        .separator()
        .text("layer_behind_active", "👁 Colocar detrás de ventana activa")
        .build()
        .map_err(|e| e.to_string())?;

    let move_sub = SubmenuBuilder::new(&window, "Desplazar a...")
        .text("pos_bottom_right", "↘ Esquina inferior derecha")
        .text("pos_bottom_left", "↙ Esquina inferior izquierda")
        .text("pos_top_right", "↗ Esquina superior derecha")
        .text("pos_top_left", "↖ Esquina superior izquierda")
        .text("pos_center", "◎ Centro del escritorio")
        .build()
        .map_err(|e| e.to_string())?;

    let snap = music_detector::get_music_state(Some(false));
    let music_status_label = if let Some(ref tr) = snap.current_track {
        let icon = if tr.is_playing { "♫" } else { "⏸" };
        let label = if tr.artist.is_empty() {
            format!("{} {} ({})", icon, tr.title, tr.source_app_name)
        } else {
            format!("{} {} — {} ({})", icon, tr.title, tr.artist, tr.source_app_name)
        };
        if label.chars().count() > 48 {
            let short: String = label.chars().take(45).collect();
            format!("{}...", short)
        } else {
            label
        }
    } else if let Some(first_app) = snap.running_apps.first() {
        format!("🎵 {} abierto (En pausa)", first_app.name)
    } else {
        "🎵 Sin música detectada".to_string()
    };

    let music_sub = SubmenuBuilder::new(&window, "Música y Multimedia")
        .text("music_focus", &music_status_label)
        .separator()
        .text("music_play_pause", "⏯ Reproducir / Pausar")
        .text("music_next", "⏭ Siguiente pista")
        .text("music_prev", "⏮ Pista anterior")
        .separator()
        .text("music_dance", "💃 Hacer bailar a Kyo")
        .text("music_move_near", "📍 Acercar Kyo a la app de música")
        .build()
        .map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(&window)
        .text("open_chat", "Panel de control")
        .separator()
        .item(&music_sub)
        .item(&layer_sub)
        .item(&move_sub)
        .separator()
        .text("options", "Opciones")
        .separator()
        .text("quit", "Cerrar Kyo")
        .build()
        .map_err(|e| e.to_string())?;

    struct ContextMenuGuard;
    impl Drop for ContextMenuGuard {
        fn drop(&mut self) {
            IS_CONTEXT_MENU_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
            avatar_desktop::ensure_avatar_topmost_if_needed();
        }
    }
    IS_CONTEXT_MENU_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
    let _menu_guard = ContextMenuGuard;

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

// Banderas atómicas para el estado de ventana (ajustes / menú contextual)
static IS_SETTINGS_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static IS_CONTEXT_MENU_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_settings_open() -> bool {
    IS_SETTINGS_OPEN.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn is_context_menu_open() -> bool {
    IS_CONTEXT_MENU_OPEN.load(std::sync::atomic::Ordering::SeqCst)
}

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
            avatar_desktop::apply_avatar_overlay_mode(&window, false);
            let _ = window.set_focus();
        }
        "avatar" => {
            IS_SETTINGS_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = window.set_skip_taskbar(true);
            let _ = window.set_size(tauri::LogicalSize::new(220.0, 220.0));
            let _ = window.set_min_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
            let _ = window.set_max_size(Some(tauri::LogicalSize::new(220.0, 220.0)));
            let _ = window.set_resizable(false);
            avatar_desktop::apply_avatar_overlay_mode(&window, true);
            avatar_desktop::restore_saved_layer(&window);
        }
        _ => {}
    }
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// Arrastre suave y personalizado del avatar sin activar Aero Snap ni FancyZones
// ──────────────────────────────────────────────────────────────────────────────
pub(crate) static IS_AVATAR_DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_avatar_dragging() -> bool {
    IS_AVATAR_DRAGGING.load(std::sync::atomic::Ordering::SeqCst)
}

#[tauri::command]
fn start_avatar_drag(window: WebviewWindow) -> Result<(), String> {
    avatar_desktop::cancel_active_animation();
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        if IS_AVATAR_DRAGGING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Ok(());
        }

        let _ = window.emit("avatar://drag-start", ());

        std::thread::spawn(move || {
            struct DragGuard {
                win: WebviewWindow,
            }
            impl Drop for DragGuard {
                fn drop(&mut self) {
                    IS_AVATAR_DRAGGING.store(false, Ordering::SeqCst);
                    let _ = self.win.emit("avatar://drag-end", ());
                }
            }
            let _guard = DragGuard { win: window.clone() };

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

                std::thread::sleep(Duration::from_millis(16));
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
// Comandos de Gestión de Sesión y Token JWT de Kyo (DPAPI + Caché en Memoria)
// ──────────────────────────────────────────────────────────────────────────────

const DPAPI_PREFIX: &str = "dpapi:v1:";

pub(crate) mod token_crypto {
    const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn base64_encode(input: &[u8]) -> String {
        let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
        for chunk in input.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;

            out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
            out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
        let clean: Vec<u8> = input
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        if clean.is_empty() || clean.len() % 4 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity((clean.len() / 4) * 3);
        for chunk in clean.chunks(4) {
            let mut vals = [0u32; 4];
            let mut pad = 0usize;
            for (i, &b) in chunk.iter().enumerate() {
                if b == b'=' {
                    vals[i] = 0;
                    pad += 1;
                } else {
                    vals[i] = match b {
                        b'A'..=b'Z' => (b - b'A') as u32,
                        b'a'..=b'z' => (b - b'a' + 26) as u32,
                        b'0'..=b'9' => (b - b'0' + 52) as u32,
                        b'+' | b'-' => 62,
                        b'/' | b'_' => 63,
                        _ => return None,
                    };
                }
            }
            let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
            out.push(((triple >> 16) & 0xFF) as u8);
            if pad < 2 {
                out.push(((triple >> 8) & 0xFF) as u8);
            }
            if pad < 1 {
                out.push((triple & 0xFF) as u8);
            }
        }
        Some(out)
    }

    #[cfg(windows)]
    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[cfg(windows)]
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut std::ffi::c_void,
            p_prompt_struct: *mut std::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut std::ffi::c_void,
            p_prompt_struct: *mut std::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[cfg(windows)]
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    #[cfg(windows)]
    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    #[cfg(windows)]
    const DPAPI_ENTROPY: &[u8] = b"KyoDesktopTokenSecurityV1";

    pub fn encrypt_token_for_disk(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with(super::DPAPI_PREFIX) {
            return trimmed.to_string();
        }
        #[cfg(windows)]
        {
            let plain = trimmed.as_bytes();
            let in_blob = DataBlob {
                cb_data: plain.len() as u32,
                pb_data: plain.as_ptr() as *mut u8,
            };
            let entropy_blob = DataBlob {
                cb_data: DPAPI_ENTROPY.len() as u32,
                pb_data: DPAPI_ENTROPY.as_ptr() as *mut u8,
            };
            let mut out_blob = DataBlob {
                cb_data: 0,
                pb_data: std::ptr::null_mut(),
            };
            let ok = unsafe {
                CryptProtectData(
                    &in_blob,
                    std::ptr::null(),
                    &entropy_blob,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out_blob,
                )
            };
            if ok != 0 && !out_blob.pb_data.is_null() {
                let slice = unsafe {
                    std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize)
                };
                let b64 = base64_encode(slice);
                unsafe {
                    LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
                }
                return format!("{}{}", super::DPAPI_PREFIX, b64);
            }
        }
        trimmed.to_string()
    }

    pub fn decrypt_token_from_disk(stored: &str) -> Option<String> {
        let trimmed = stored.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(b64_part) = trimmed.strip_prefix(super::DPAPI_PREFIX) {
            #[cfg(windows)]
            {
                let cipher = base64_decode(b64_part)?;
                if cipher.is_empty() {
                    return None;
                }
                let in_blob = DataBlob {
                    cb_data: cipher.len() as u32,
                    pb_data: cipher.as_ptr() as *mut u8,
                };
                let entropy_blob = DataBlob {
                    cb_data: DPAPI_ENTROPY.len() as u32,
                    pb_data: DPAPI_ENTROPY.as_ptr() as *mut u8,
                };
                let mut out_blob = DataBlob {
                    cb_data: 0,
                    pb_data: std::ptr::null_mut(),
                };
                let ok = unsafe {
                    CryptUnprotectData(
                        &in_blob,
                        std::ptr::null_mut(),
                        &entropy_blob,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        CRYPTPROTECT_UI_FORBIDDEN,
                        &mut out_blob,
                    )
                };
                if ok == 0 || out_blob.pb_data.is_null() {
                    return None;
                }
                let slice = unsafe {
                    std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize)
                };
                let vec = slice.to_vec();
                unsafe {
                    LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
                }
                return String::from_utf8(vec).ok();
            }
            #[cfg(not(windows))]
            {
                let _ = b64_part;
                return None;
            }
        }
        Some(trimmed.to_string())
    }
}

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

static SESSION_CONFIG_CACHE: std::sync::OnceLock<
    std::sync::RwLock<(Option<std::time::SystemTime>, SessionConfig)>,
> = std::sync::OnceLock::new();

fn session_config_cache() -> &'static std::sync::RwLock<(Option<std::time::SystemTime>, SessionConfig)> {
    SESSION_CONFIG_CACHE.get_or_init(|| std::sync::RwLock::new((None, SessionConfig::default())))
}

/// Directorio estándar multiplataforma para datos persistentes de Kyo
/// (%APPDATA%\kyo en Windows, ~/.config/kyo en Linux, ~/Library/Application Support/kyo en macOS).
pub fn get_kyo_data_dir() -> Option<std::path::PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Some(std::path::PathBuf::from(appdata).join("kyo"));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(std::path::PathBuf::from(xdg).join("kyo"));
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        return Some(std::path::PathBuf::from(home).join("Library").join("Application Support").join("kyo"));
        #[cfg(not(target_os = "macos"))]
        return Some(std::path::PathBuf::from(home).join(".config").join("kyo"));
    }
    None
}

fn write_session_atomic(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, content).map_err(|e| format!("Error escribiendo temporal session.json: {e}"))?;
    if std::fs::rename(&tmp_path, path).is_err() {
        std::fs::write(path, content).map_err(|e| format!("Error escribiendo session.json: {e}"))?;
        let _ = std::fs::remove_file(&tmp_path);
    }
    Ok(())
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
    let mut clean_token = token.trim();
    if (clean_token.starts_with('"') && clean_token.ends_with('"')) || (clean_token.starts_with('\'') && clean_token.ends_with('\'')) {
        if clean_token.len() >= 2 {
            clean_token = &clean_token[1..clean_token.len() - 1].trim();
        }
    }
    if clean_token.len() >= 7 && clean_token[..7].eq_ignore_ascii_case("bearer ") {
        clean_token = clean_token[7..].trim();
    }
    if (clean_token.starts_with('"') && clean_token.ends_with('"')) || (clean_token.starts_with('\'') && clean_token.ends_with('\'')) {
        if clean_token.len() >= 2 {
            clean_token = &clean_token[1..clean_token.len() - 1].trim();
        }
    }
    let trimmed_token: String = clean_token
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();

    let trimmed_url = server_url.as_ref().map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Some(kyo_dir) = get_kyo_data_dir() {
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

        let encrypted_token = token_crypto::encrypt_token_for_disk(&trimmed_token);
        map.insert("token".to_string(), serde_json::Value::String(encrypted_token));
        map.remove("access_token");
        if let Some(ref url) = trimmed_url {
            map.insert("server_url".to_string(), serde_json::Value::String(url.clone()));
        }
        if let Some(ref pid) = active_profile_id {
            map.insert("active_profile_id".to_string(), serde_json::Value::String(pid.clone()));
        }
        if let Some(ref pname) = active_profile_name {
            map.insert("active_profile_name".to_string(), serde_json::Value::String(pname.clone()));
        }
        if let Some(ref u) = username {
            map.insert("username".to_string(), serde_json::Value::String(u.clone()));
        }
        if let Some(ref e) = email {
            map.insert("email".to_string(), serde_json::Value::String(e.clone()));
        }
        map.insert("updated_at".to_string(), serde_json::json!(now_ts));

        write_session_atomic(&session_file, &serde_json::Value::Object(map).to_string())?;

        let mtime = std::fs::metadata(&session_file).and_then(|m| m.modified()).ok();
        if let Ok(mut guard) = session_config_cache().write() {
            let prev = guard.1.clone();
            guard.0 = mtime;
            guard.1 = SessionConfig {
                token: if trimmed_token.is_empty() { None } else { Some(trimmed_token) },
                server_url: trimmed_url.or(prev.server_url),
                active_profile_id: active_profile_id.or(prev.active_profile_id),
                active_profile_name: active_profile_name.or(prev.active_profile_name),
                username: username.or(prev.username),
                email: email.or(prev.email),
                updated_at: Some(now_ts),
            };
        }
    }

    Ok(())
}

#[tauri::command]
fn save_session_token(token: String) -> Result<(), String> {
    save_session_config(token, None, None, None, None, None)
}

#[tauri::command]
fn get_session_config() -> SessionConfig {
    if let Some(kyo_dir) = get_kyo_data_dir() {
        let session_file = kyo_dir.join("session.json");
        let file_mtime = std::fs::metadata(&session_file).and_then(|m| m.modified()).ok();

        if let Some(mtime) = file_mtime {
            if let Ok(guard) = session_config_cache().read() {
                if guard.0 == Some(mtime) {
                    return guard.1.clone();
                }
            }
        }

        let mut config = SessionConfig::default();
        let mut needs_dpapi_upgrade = false;

        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(mut map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content) {
                    if let Some(raw_token) = map.get("token").or_else(|| map.get("access_token")).and_then(|t| t.as_str()) {
                        let trimmed = raw_token.trim();
                        if !trimmed.is_empty() {
                            if !trimmed.starts_with(DPAPI_PREFIX) {
                                needs_dpapi_upgrade = true;
                            }
                            config.token = token_crypto::decrypt_token_from_disk(trimmed);
                        }
                    }
                    if let Some(url) = map.get("server_url").or_else(|| map.get("upstream")).and_then(|u| u.as_str()) {
                        config.server_url = Some(url.to_string());
                    }
                    if let Some(pid) = map.get("active_profile_id").and_then(|p| p.as_str()) {
                        config.active_profile_id = Some(pid.to_string());
                    }
                    if let Some(pname) = map.get("active_profile_name").and_then(|p| p.as_str()) {
                        config.active_profile_name = Some(pname.to_string());
                    }
                    if let Some(u) = map.get("username").and_then(|p| p.as_str()) {
                        config.username = Some(u.to_string());
                    }
                    if let Some(e) = map.get("email").and_then(|p| p.as_str()) {
                        config.email = Some(e.to_string());
                    }
                    if let Some(ts) = map.get("updated_at").and_then(|v| v.as_u64()) {
                        config.updated_at = Some(ts);
                    }

                    // Migrar transparentemente tokens antiguos en texto plano a DPAPI
                    if needs_dpapi_upgrade {
                        if let Some(ref plain_tok) = config.token {
                            let enc = token_crypto::encrypt_token_for_disk(plain_tok);
                            if enc.starts_with(DPAPI_PREFIX) {
                                map.insert("token".to_string(), serde_json::Value::String(enc));
                                map.remove("access_token");
                                let _ = write_session_atomic(&session_file, &serde_json::Value::Object(map).to_string());
                            }
                        }
                    }
                }
            }
        }

        if config.token.is_none() {
            if let Ok(token) = std::env::var("KYO_AUTH_TOKEN") {
                let trimmed = token.trim().to_string();
                if !trimmed.is_empty() {
                    config.token = token_crypto::decrypt_token_from_disk(&trimmed);
                }
            }
        }
        if config.server_url.is_none() {
            if let Ok(url) = std::env::var("KYO_UPSTREAM").or_else(|_| std::env::var("KYO_SERVER_URL")) {
                let trimmed = url.trim().to_string();
                if !trimmed.is_empty() {
                    config.server_url = Some(trimmed);
                }
            }
        }

        let updated_mtime = std::fs::metadata(&session_file).and_then(|m| m.modified()).ok();
        if let Ok(mut guard) = session_config_cache().write() {
            guard.0 = updated_mtime;
            guard.1 = config.clone();
        }
        return config;
    }

    SessionConfig::default()
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
        avatar_desktop::apply_avatar_overlay_mode(&window, true);
        let _ = window.show();
        let _ = window.unminimize();
        avatar_desktop::restore_saved_layer(&window);
        let _ = window.emit("restore-avatar", ());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Conexión con Dioxus Chat: Detección, Enfoque y Lanzamiento de Ventana
// ──────────────────────────────────────────────────────────────────────────────

/// Localiza el binario ejecutable de Dioxus Chat (`desktop` o `desktop.exe`) de forma segura y multiplataforma.
pub fn resolve_dioxus_chat_exe() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    let bin_name = if cfg!(windows) { "desktop.exe" } else { "desktop" };

    // 1. Relativo al ejecutable actual (mismo directorio o ancestros)
    if let Ok(cur_exe) = std::env::current_exe() {
        if let Some(exe_dir) = cur_exe.parent() {
            candidates.push(exe_dir.join(bin_name));
            candidates.push(exe_dir.join("dioxus-chat").join("target").join("release").join(bin_name));
            candidates.push(exe_dir.join("dioxus-chat").join(bin_name));
        }
        let mut cur = cur_exe.clone();
        for _ in 0..5 {
            if cur.pop() {
                candidates.push(cur.join(bin_name));
                candidates.push(cur.join("dioxus-chat").join("target").join("release").join(bin_name));
                candidates.push(cur.join("dioxus-chat").join("target").join("debug").join(bin_name));
                candidates.push(cur.join("dioxus-chat").join("target").join("dx").join("desktop").join("debug").join("windows").join("app").join(bin_name));
                candidates.push(cur.join("dioxus-chat").join("target").join("dx").join("desktop").join("release").join("windows").join("app").join(bin_name));
                candidates.push(cur.join("dioxus-chat").join(bin_name));
            }
        }
    }

    // 2. Rutas fijas de desarrollo
    candidates.push(std::path::PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\release\desktop.exe"));
    candidates.push(std::path::PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\debug\desktop.exe"));
    candidates.push(std::path::PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\target\dx\desktop\debug\windows\app\desktop.exe"));

    // Filtrar solo los ejecutables existentes
    let mut existing: Vec<std::path::PathBuf> = candidates.into_iter().filter(|p| p.is_file()).collect();

    // Priorizar el binario más recientemente modificado/compilado
    existing.sort_by(|a, b| {
        let time_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let time_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    existing.into_iter().next()
}

#[cfg(windows)]
mod win32_chat {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
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
        fn IsZoomed(hWnd: HWND) -> BOOL;
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
    const SW_SHOWMAXIMIZED: i32 = 3;

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
        }
        if IsZoomed(hwnd) == 0 {
            ShowWindow(hwnd, SW_SHOWMAXIMIZED);
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

    pub fn launch_dioxus_chat(app: &tauri::AppHandle) -> Result<(), String> {
        let exe = crate::resolve_dioxus_chat_exe()
            .ok_or_else(|| "No se encontró el ejecutable desktop de Dioxus Chat".to_string())?;

        let parent = exe.parent().unwrap_or(Path::new("."));
        let mut cmd = Command::new(&exe);
        cmd.current_dir(parent);

        // Si el sidecar Go ya tiene puerto asignado, inyectarlo en el entorno
        if let Some(port) = crate::sidecar::get_sidecar_port_from_app(app) {
            cmd.env("KYO_SIDECAR_PORT", port.to_string());
        }

        // SEGURIDAD: No inyectamos KYO_AUTH_TOKEN en texto plano en el bloque de entorno del subproceso
        // (evita exposición en PEB/Process Explorer). Dioxus Chat desencripta session.json vía DPAPI.
        let cfg = crate::get_session_config();
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

#[cfg(not(windows))]
mod non_windows_chat {
    use std::path::Path;
    use std::process::Command;

    pub fn launch_and_watch(app: tauri::AppHandle) -> Result<(), String> {
        let exe = crate::resolve_dioxus_chat_exe()
            .ok_or_else(|| "No se encontró el ejecutable desktop de Dioxus Chat".to_string())?;

        let parent = exe.parent().unwrap_or(Path::new("."));
        let mut cmd = Command::new(&exe);
        cmd.current_dir(parent);

        // Si el sidecar Go ya tiene puerto asignado, inyectarlo en el entorno
        if let Some(port) = crate::sidecar::get_sidecar_port_from_app(&app) {
            cmd.env("KYO_SIDECAR_PORT", port.to_string());
        }

        // SEGURIDAD: No exponer KYO_AUTH_TOKEN en variables de entorno del proceso hijo
        let cfg = crate::get_session_config();
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

        let mut child = cmd.spawn()
            .map_err(|e| format!("Error al iniciar {}: {}", exe.display(), e))?;

        // Monitorear proceso hijo: al cerrar la ventana, restaurar el avatar
        std::thread::spawn(move || {
            let _ = child.wait();
            crate::restore_avatar(&app);
        });

        Ok(())
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
        non_windows_chat::launch_and_watch(app)?;
        Ok("launched".to_string())
    }
}

fn handle_app_menu_action(app: &tauri::AppHandle, id: &str) {
    match id {
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
                avatar_desktop::apply_avatar_overlay_mode(&window, false);
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
        "layer_top" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::set_avatar_layer(w, "top".to_string(), None);
            }
        }
        "layer_normal" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::set_avatar_layer(w, "normal".to_string(), None);
            }
        }
        "layer_bottom" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::set_avatar_layer(w, "bottom".to_string(), None);
            }
        }
        "layer_behind_active" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::set_avatar_layer(w, "behind_active".to_string(), None);
            }
        }
        "pos_bottom_right" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::position_window_anchor(w, "bottom-right".to_string(), Some(true), Some(750));
            }
        }
        "pos_bottom_left" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::position_window_anchor(w, "bottom-left".to_string(), Some(true), Some(750));
            }
        }
        "pos_top_right" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::position_window_anchor(w, "top-right".to_string(), Some(true), Some(750));
            }
        }
        "pos_top_left" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::position_window_anchor(w, "top-left".to_string(), Some(true), Some(750));
            }
        }
        "pos_center" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = avatar_desktop::position_window_anchor(w, "center".to_string(), Some(true), Some(750));
            }
        }
        "music_play_pause" => {
            if let Some(w) = app.get_webview_window("main") {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = music_detector::control_music(app_c, w, "play_pause".to_string(), None).await;
                });
            }
        }
        "music_next" => {
            if let Some(w) = app.get_webview_window("main") {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = music_detector::control_music(app_c, w, "next".to_string(), None).await;
                });
            }
        }
        "music_prev" => {
            if let Some(w) = app.get_webview_window("main") {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = music_detector::control_music(app_c, w, "previous".to_string(), None).await;
                });
            }
        }
        "music_focus" => {
            if let Some(w) = app.get_webview_window("main") {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = music_detector::control_music(app_c, w, "focus_app".to_string(), None).await;
                });
            }
        }
        "music_move_near" => {
            if let Some(w) = app.get_webview_window("main") {
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = music_detector::control_music(app_c, w, "move_near_app".to_string(), None).await;
                });
            }
        }
        "music_dance" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.emit("agent://state", "dance");
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Punto de entrada de la aplicación.
// ──────────────────────────────────────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
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
            sidecar::get_sidecar_port,
            sidecar::sidecar_port,
            save_session_token,
            get_session_token,
            save_session_config,
            get_session_config,
            font_manager::get_system_fonts,
            font_manager::install_bundled_font,
            font_manager::save_font_preference,
            font_manager::get_font_preference,
            avatar_desktop::set_avatar_layer,
            avatar_desktop::get_avatar_layer,
            avatar_desktop::get_open_windows,
            avatar_desktop::get_desktop_bounds,
            avatar_desktop::animate_avatar_to,
            avatar_desktop::teleport_avatar,
            avatar_desktop::stop_avatar_movement,
            avatar_desktop::is_avatar_moving,
            avatar_desktop::set_avatar_ignore_cursor,
            avatar_desktop::position_window_anchor,
            music_detector::get_music_state,
            music_detector::control_music
        ])
        .on_menu_event(|app, event| {
            handle_app_menu_action(app, event.id().as_ref());
        })
        .setup(|app| {
            // 1. Crear items y submenús contextuales de la bandeja
            let chat_item = MenuItem::with_id(app, "open_chat", "Panel de control", true, None::<&str>)?;
            let opt_item = MenuItem::with_id(app, "options", "Opciones", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Cerrar Kyo", true, None::<&str>)?;

            let layer_sub = SubmenuBuilder::new(app, "Capa de ventana")
                .text("layer_top", "▲ Siempre visible (Superior)")
                .text("layer_normal", "■ Entre ventanas abiertas (Normal)")
                .text("layer_bottom", "▼ Fondo de escritorio")
                .separator()
                .text("layer_behind_active", "👁 Colocar detrás de ventana activa")
                .build()?;

            let move_sub = SubmenuBuilder::new(app, "Desplazar a...")
                .text("pos_bottom_right", "↘ Esquina inferior derecha")
                .text("pos_bottom_left", "↙ Esquina inferior izquierda")
                .text("pos_top_right", "↗ Esquina superior derecha")
                .text("pos_top_left", "↖ Esquina superior izquierda")
                .text("pos_center", "◎ Centro del escritorio")
                .build()?;

            let music_sub = SubmenuBuilder::new(app, "Música y Multimedia")
                .text("music_play_pause", "⏯ Reproducir / Pausar")
                .text("music_next", "⏭ Siguiente pista")
                .text("music_prev", "⏮ Pista anterior")
                .separator()
                .text("music_dance", "💃 Hacer bailar a Kyo")
                .text("music_focus", "↗ Enfocar app de música")
                .text("music_move_near", "📍 Acercar Kyo a la app")
                .build()?;

            // 2. Construir menú de la bandeja
            let menu = Menu::with_items(app, &[
                &chat_item,
                &sep1,
                &music_sub,
                &layer_sub,
                &move_sub,
                &sep2,
                &opt_item,
                &quit_item,
            ])?;

            // 3. Configurar Tray Icon
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false) // Clic derecho abre el menú
                .tooltip("Kyo")
                .on_menu_event(|app, event| {
                    handle_app_menu_action(app, event.id.as_ref());
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
                avatar_desktop::apply_avatar_overlay_mode(&window, true);
                let _ = window.show();
                let _ = window.unminimize();
                avatar_desktop::restore_saved_layer(&window);

                let win_boot = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let win_main = win_boot.clone();
                    let _ = win_boot.run_on_main_thread(move || {
                        if !is_settings_open() {
                            avatar_desktop::refresh_overlay_styles_if_avatar();
                            avatar_desktop::ensure_avatar_topmost_if_needed();
                            let _ = win_main;
                        }
                    });
                });
            }

            // 5. Iniciar subproceso Go Sidecar de forma asíncrona
            sidecar::spawn(&app.handle());

            // 5b. Iniciar detector de música y sesiones multimedia (Spotify, SMTC, VLC, etc.)
            music_detector::start_music_watcher(&app.handle());

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
                            let _ = window.run_on_main_thread(|| {
                                avatar_desktop::refresh_overlay_styles_if_avatar();
                            });
                        }
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Error al construir la aplicación Tauri");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. } = event {
            sidecar::kill_sidecar(app_handle);
        }
    });
}
