//! Componente nativo de Ajustes y Configuración de Kyo en Dioxus.
//!
//! Reemplaza el componente SettingsModal.vue de Vue 3:
//! - Pestañas: Conexión Kyo, General y Acerca de.
//! - Gestión del Token de autenticación JWT / API Key con persistencia en %APPDATA%/kyo/session.json.
//! - Diagnóstico en vivo de la conexión con el Sidecar Go o Fallback Cloud con cálculo de latencia.
//! - Conmutador de tema visual.

use std::path::PathBuf;
use dioxus::prelude::*;
use crate::sidecar::{check_cloud_health, check_sidecar_health, SidecarConnectionStatus, SidecarState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    KyoConnection,
    General,
    About,
}

/// Guarda el token y URL del servidor en memoria y en %APPDATA%/kyo/session.json.
pub fn save_auth_config(token: &str, server_url: &str) -> Result<(), String> {
    let trimmed_token = token.trim().to_string();
    let trimmed_url = server_url.trim().to_string();

    std::env::set_var("KYO_AUTH_TOKEN", &trimmed_token);
    if !trimmed_url.is_empty() {
        std::env::set_var("KYO_UPSTREAM", &trimmed_url);
        std::env::set_var("KYO_SERVER_URL", &trimmed_url);
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = PathBuf::from(appdata).join("kyo");
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
        if !trimmed_url.is_empty() {
            map.insert("server_url".to_string(), serde_json::Value::String(trimmed_url));
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
            .map_err(|e| format!("Error guardando session.json: {e}"))?;
    }

    Ok(())
}

/// Guarda la configuración completa de sesión en memoria y en %APPDATA%/kyo/session.json.
pub fn save_full_session_config(
    token: &str,
    server_url: &str,
    username: Option<&str>,
    email: Option<&str>,
    profile_name: Option<&str>,
    profile_id: Option<&str>,
) -> Result<(), String> {
    let trimmed_token = token.trim().to_string();
    let trimmed_url = server_url.trim().to_string();

    std::env::set_var("KYO_AUTH_TOKEN", &trimmed_token);
    if !trimmed_url.is_empty() {
        std::env::set_var("KYO_UPSTREAM", &trimmed_url);
        std::env::set_var("KYO_SERVER_URL", &trimmed_url);
    }
    if let Some(u) = username {
        let tu = u.trim();
        if !tu.is_empty() {
            std::env::set_var("KYO_USERNAME", tu);
        }
    }
    if let Some(p) = profile_name {
        let tp = p.trim();
        if !tp.is_empty() {
            std::env::set_var("KYO_ACTIVE_PROFILE_NAME", tp);
        }
    }
    if let Some(pid) = profile_id {
        let tpid = pid.trim();
        if !tpid.is_empty() {
            std::env::set_var("KYO_ACTIVE_PROFILE_ID", tpid);
        }
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = PathBuf::from(appdata).join("kyo");
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
        if !trimmed_url.is_empty() {
            map.insert("server_url".to_string(), serde_json::Value::String(trimmed_url));
        }
        if let Some(u) = username {
            if !u.trim().is_empty() {
                map.insert("username".to_string(), serde_json::Value::String(u.trim().to_string()));
            }
        }
        if let Some(e) = email {
            if !e.trim().is_empty() {
                map.insert("email".to_string(), serde_json::Value::String(e.trim().to_string()));
            }
        }
        if let Some(p) = profile_name {
            if !p.trim().is_empty() {
                map.insert("active_profile_name".to_string(), serde_json::Value::String(p.trim().to_string()));
            }
        }
        if let Some(pid) = profile_id {
            if !pid.trim().is_empty() {
                map.insert("active_profile_id".to_string(), serde_json::Value::String(pid.trim().to_string()));
            }
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
            .map_err(|e| format!("Error guardando session.json: {e}"))?;
    }

    Ok(())
}

/// Limpia la sesión de autenticación (elimina el token y datos de usuario).
pub fn clear_session_auth() -> Result<(), String> {
    std::env::remove_var("KYO_AUTH_TOKEN");
    std::env::remove_var("KYO_USERNAME");
    std::env::remove_var("KYO_ACTIVE_PROFILE_NAME");
    std::env::remove_var("KYO_ACTIVE_PROFILE_ID");

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = PathBuf::from(appdata).join("kyo");
        let session_file = kyo_dir.join("session.json");
        if session_file.is_file() {
            let mut map: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(&session_file)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default();
            map.remove("token");
            map.remove("access_token");
            map.remove("username");
            map.remove("email");
            map.remove("active_profile_name");
            map.remove("active_profile_id");
            let _ = std::fs::write(&session_file, serde_json::Value::Object(map).to_string());
        }
    }
    Ok(())
}

/// Guarda el token en memoria y en %APPDATA%/kyo/session.json.
pub fn save_auth_token(token: &str) -> Result<(), String> {
    save_auth_config(token, "")
}

/// Recupera la URL del servidor remoto guardada.
pub fn load_server_url() -> String {
    if let Ok(url) = std::env::var("KYO_UPSTREAM").or_else(|_| std::env::var("KYO_SERVER_URL")) {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(u) = json.get("server_url").or_else(|| json.get("upstream")).and_then(|t| t.as_str()) {
                        return u.to_string();
                    }
                }
            }
        }
    }

    "https://kyo.broco.dev".to_string()
}

/// Recupera el token guardado en el archivo session.json o variable de entorno.
pub fn load_auth_token() -> String {
    if let Ok(token) = std::env::var("KYO_AUTH_TOKEN") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(t) = json.get("token").and_then(|t| t.as_str()) {
                        return t.to_string();
                    }
                    if let Some(t) = json.get("access_token").and_then(|t| t.as_str()) {
                        return t.to_string();
                    }
                }
            }
        }
    }

    String::new()
}

/// Recupera el nombre del perfil activo guardado en session.json o variable de entorno.
pub fn load_active_profile_name() -> Option<String> {
    if let Ok(name) = std::env::var("KYO_ACTIVE_PROFILE_NAME") {
        let trimmed = name.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(n) = json.get("active_profile_name").and_then(|t| t.as_str()) {
                        let trimmed = n.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(trimmed);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recupera el nombre de usuario de la cuenta guardado en session.json o variable de entorno.
pub fn load_username() -> Option<String> {
    if let Ok(name) = std::env::var("KYO_USERNAME") {
        let trimmed = name.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(u) = json.get("username").and_then(|t| t.as_str()) {
                        let trimmed = u.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(trimmed);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recupera el correo de la cuenta guardado en session.json.
pub fn load_email() -> Option<String> {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_file = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(e) = json.get("email").and_then(|t| t.as_str()) {
                        let trimmed = e.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(trimmed);
                        }
                    }
                }
            }
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub struct JwtAnalysis {
    pub is_jwt: bool,
    pub is_expired: bool,
    pub expires_in_sec: Option<i64>,
    pub expiration_str: Option<String>,
    pub sub: Option<String>,
    pub role: Option<String>,
    pub alg: Option<String>,
    pub warning: Option<String>,
}

pub fn analyze_token(raw: &str) -> Option<JwtAnalysis> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 3 {
        return Some(JwtAnalysis {
            is_jwt: false,
            is_expired: false,
            expires_in_sec: None,
            expiration_str: None,
            sub: None,
            role: None,
            alg: None,
            warning: Some("Token opaco / Bearer clásico (sin formato JWT de 3 segmentos)".to_string()),
        });
    }

    use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
    use base64::Engine;

    let decode_part = |s: &str| -> Option<Vec<u8>> {
        URL_SAFE_NO_PAD.decode(s).or_else(|_| URL_SAFE.decode(s)).ok()
    };

    let header_bytes = decode_part(parts[0]);
    let alg = header_bytes
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("alg").and_then(|a| a.as_str()).map(|s| s.to_string()));

    if alg.as_deref() == Some("none") {
        return Some(JwtAnalysis {
            is_jwt: true,
            is_expired: false,
            expires_in_sec: None,
            expiration_str: None,
            sub: None,
            role: None,
            alg: Some("none".into()),
            warning: Some("Vulnerabilidad de seguridad: algoritmo 'none' rechazado (RFC 7519)".into()),
        });
    }

    let payload_bytes = decode_part(parts[1]);
    let payload: Option<serde_json::Value> = payload_bytes.and_then(|b| serde_json::from_slice(&b).ok());

    if let Some(p) = payload {
        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let exp = p.get("exp").and_then(|e| e.as_i64());
        let (is_expired, expires_in_sec, expiration_str) = if let Some(exp_val) = exp {
            let diff = exp_val - now_sec;
            let expired = diff <= 0;
            let human = if expired {
                format!("Expirado hace {}s", -diff)
            } else if diff < 3600 {
                format!("Expira en {} min", diff / 60)
            } else if diff < 86400 {
                format!("Expira en {} horas", diff / 3600)
            } else {
                format!("Expira en {} días", diff / 86400)
            };
            (expired, Some(diff), Some(human))
        } else {
            (false, None, Some("Sin caducidad fija (exp)".into()))
        };

        let sub = p.get("sub").or_else(|| p.get("id")).and_then(|s| s.as_str()).map(|s| s.to_string());
        let role = p.get("role").or_else(|| p.get("user_role")).and_then(|s| s.as_str()).map(|s| s.to_string());

        let warning = if is_expired {
            Some("Este token ha expirado. Las solicitudes serán rechazadas (HTTP 401).".into())
        } else {
            None
        };

        Some(JwtAnalysis {
            is_jwt: true,
            is_expired,
            expires_in_sec,
            expiration_str,
            sub,
            role,
            alg,
            warning,
        })
    } else {
        Some(JwtAnalysis {
            is_jwt: true,
            is_expired: false,
            expires_in_sec: None,
            expiration_str: None,
            sub: None,
            role: None,
            alg,
            warning: Some("Cuerpo del JWT no se pudo decodificar (formato JSON no válido)".into()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppFont(pub Signal<String>);

/// Guarda la preferencia de tipografía en %APPDATA%/kyo/settings.json
pub fn save_font_family(font: &str) -> Result<(), String> {
    let trimmed = font.trim().to_string();
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = PathBuf::from(appdata).join("kyo");
        let _ = std::fs::create_dir_all(&kyo_dir);
        let settings_file = kyo_dir.join("settings.json");
        let mut json = if settings_file.is_file() {
            std::fs::read_to_string(&settings_file)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        json["font_family"] = serde_json::json!(trimmed);
        std::fs::write(&settings_file, json.to_string())
            .map_err(|e| format!("Error guardando settings.json: {e}"))?;
    }
    Ok(())
}

/// Carga la preferencia de tipografía guardada o retorna "Segoe UI" (predeterminada)
pub fn load_font_family() -> String {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let settings_file = PathBuf::from(appdata).join("kyo").join("settings.json");
        if settings_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&settings_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(f) = json.get("font_family").and_then(|t| t.as_str()) {
                        let trimmed = f.trim();
                        if !trimmed.is_empty() && trimmed != "Comic Sans MS" && trimmed != "Kyo" {
                            return trimmed.to_string();
                        }
                    }
                }
            }
        }
    }
    "Segoe UI".to_string()
}

/// Guarda la preferencia de tema (oscuro/claro) en %APPDATA%/kyo/settings.json
pub fn save_theme_preference(is_dark: bool) -> Result<(), String> {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let kyo_dir = PathBuf::from(appdata).join("kyo");
        let _ = std::fs::create_dir_all(&kyo_dir);
        let settings_file = kyo_dir.join("settings.json");
        let mut json = if settings_file.is_file() {
            std::fs::read_to_string(&settings_file)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or_else(|| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        json["theme"] = serde_json::json!(if is_dark { "dark" } else { "light" });
        std::fs::write(&settings_file, json.to_string())
            .map_err(|e| format!("Error guardando settings.json: {e}"))?;
    }
    Ok(())
}

/// Carga la preferencia de tema guardada (true = dark por defecto, false = light)
pub fn load_theme_preference() -> bool {
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let settings_file = PathBuf::from(appdata).join("kyo").join("settings.json");
        if settings_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&settings_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(t) = json.get("theme").and_then(|v| v.as_str()) {
                        if t == "light" {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

#[cfg(windows)]
pub fn get_system_fonts() -> Vec<String> {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    type HKEY = isize;
    type DWORD = u32;
    type LSTATUS = i32;

    const HKEY_CURRENT_USER: HKEY = 0x80000001_u32 as isize;
    const HKEY_LOCAL_MACHINE: HKEY = 0x80000002_u32 as isize;
    const KEY_READ: DWORD = 0x20019;
    const KEY_WOW64_64KEY: DWORD = 0x0100;
    const ERROR_SUCCESS: LSTATUS = 0;
    const ERROR_NO_MORE_ITEMS: LSTATUS = 259;

    extern "system" {
        fn RegOpenKeyExW(
            hKey: HKEY,
            lpSubKey: *const u16,
            ulOptions: DWORD,
            samDesired: DWORD,
            phkResult: *mut HKEY,
        ) -> LSTATUS;

        fn RegEnumValueW(
            hKey: HKEY,
            dwIndex: DWORD,
            lpValueName: *mut u16,
            lpcchValueName: *mut DWORD,
            lpReserved: *mut DWORD,
            lpType: *mut DWORD,
            lpData: *mut u8,
            lpcbData: *mut DWORD,
        ) -> LSTATUS;

        fn RegCloseKey(hKey: HKEY) -> LSTATUS;
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    unsafe fn read_from_key(root: HKEY, subkey: &str, set: &mut BTreeSet<String>) {
        let wide_subkey = to_wide(subkey);
        let mut hk: HKEY = 0;
        if RegOpenKeyExW(root, wide_subkey.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut hk) != ERROR_SUCCESS {
            return;
        }

        let mut index = 0;
        let mut name_buf = [0u16; 512];
        loop {
            let mut name_len = name_buf.len() as DWORD;
            let status = RegEnumValueW(
                hk,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status == ERROR_SUCCESS {
                let raw_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                let trimmed = raw_name.trim_matches('\0').trim();
                let lower = trimmed.to_lowercase();
                let mut cleaned = trimmed;
                let suffixes = [
                    " (truetype)",
                    " (opentype)",
                    " (all type)",
                    " (truetype & opentype)",
                    " (postscript)",
                ];
                for s in suffixes {
                    if lower.ends_with(s) {
                        cleaned = &cleaned[..cleaned.len() - s.len()];
                        break;
                    }
                }
                let cleaned_str = cleaned.trim().to_string();
                if !cleaned_str.is_empty() && !cleaned_str.starts_with('@') {
                    set.insert(cleaned_str);
                }
            }
            index += 1;
        }
        RegCloseKey(hk);
    }

    let mut set = BTreeSet::new();
    unsafe {
        read_from_key(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts", &mut set);
        read_from_key(HKEY_CURRENT_USER, r"Software\Microsoft\Windows NT\CurrentVersion\Fonts", &mut set);
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort_by_key(|a| a.to_lowercase());
    list
}

#[cfg(not(windows))]
pub fn get_system_fonts() -> Vec<String> {
    vec!["Segoe UI".into(), "Arial".into(), "Inter".into(), "Roboto".into()]
}

/// Modal de Configuración unificada de Kyo Desktop.
#[component]
pub fn SettingsModal(mut is_open: Signal<bool>) -> Element {
    let sidecar_state = use_context::<Signal<SidecarState>>();
    let active_view_ctx = try_use_context::<Signal<crate::models::ActiveView>>();
    let app_font_ctx = try_use_context::<AppFont>();
    let mut local_font = use_signal(load_font_family);
    let system_fonts = use_signal(get_system_fonts);
    let mut font_search = use_signal(|| String::new());
    let mut is_font_list_open = use_signal(|| false);
    let active_font = if let Some(af) = app_font_ctx {
        af.0.read().clone()
    } else {
        local_font.read().clone()
    };
    let mut active_tab = use_signal(|| SettingsTab::KyoConnection);
    let app_theme = try_use_context::<crate::models::AppTheme>();

    // Estado del formulario de Token y Upstream
    let mut token_input = use_signal(load_auth_token);
    let mut upstream_input = use_signal(load_server_url);
    let active_profile_name = use_signal(load_active_profile_name);
    let connected_username = use_signal(load_username);
    let mut is_token_visible = use_signal(|| false);
    let mut show_saved_badge = use_signal(|| false);
    let mut is_checking_health = use_signal(|| false);

    let state = sidecar_state.read().clone();
    let (dot_class, text_class, badge_class) = state.status.color_classes();

    let port_str = if let Some(p) = state.port {
        format!("127.0.0.1:{p}")
    } else {
        "Inactivo / No detectado".to_string()
    };
    let latency_str = if let Some(lat) = state.latency_ms {
        format!("{lat} ms")
    } else {
        "-- ms".to_string()
    };

    let mut do_check_health = move || {
        is_checking_health.set(true);
        let mut sc = sidecar_state;
        let p_opt = state.port;

        dioxus::prelude::spawn(async move {
            if let Some(port) = p_opt {
                if let Some(lat) = check_sidecar_health(port).await {
                    let mut w = sc.write();
                    w.latency_ms = Some(lat);
                    w.status = SidecarConnectionStatus::ConnectedLocal;
                } else if let Some(cloud_lat) = check_cloud_health().await {
                    let mut w = sc.write();
                    w.latency_ms = Some(cloud_lat);
                    w.status = SidecarConnectionStatus::FallbackCloud;
                } else {
                    sc.write().status = SidecarConnectionStatus::Disconnected;
                }
            } else if let Some(cloud_lat) = check_cloud_health().await {
                let mut w = sc.write();
                w.latency_ms = Some(cloud_lat);
                w.status = SidecarConnectionStatus::FallbackCloud;
            } else {
                sc.write().status = SidecarConnectionStatus::Disconnected;
            }
            is_checking_health.set(false);
        });
    };

    let mut do_save_token = move || {
        let t_val = token_input.read().clone();
        let u_val = upstream_input.read().clone();
        if let Ok(()) = save_auth_config(&t_val, &u_val) {
            show_saved_badge.set(true);
            dioxus::prelude::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
                show_saved_badge.set(false);
            });
            do_check_health();
        }
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 animate-fade-in select-none",
            onclick: move |_| is_open.set(false),

            div {
                class: "w-full max-w-2xl h-[560px] rounded-2xl bg-zinc-900 border border-zinc-700 shadow-2xl flex flex-col overflow-hidden text-zinc-100",
                style: "width: 100%; max-width: 680px; height: 560px; max-height: 90vh; display: flex; flex-direction: column; overflow: hidden;",
                onclick: move |evt| evt.stop_propagation(),

                // Cabecera del modal
                header {
                    class: "flex-none h-14 px-6 border-b border-zinc-800 flex items-center justify-between bg-zinc-900/80",
                    div {
                        class: "flex items-center gap-3",
                        div {
                            class: "w-8 h-8 rounded-lg bg-indigo-600/20 border border-indigo-500/30 flex items-center justify-center text-indigo-400",
                            svg {
                                class: "w-4 h-4",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                circle { cx: "12", cy: "12", r: "3" }
                                path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" }
                            }
                        }
                        div {
                            h2 { class: "text-sm font-bold tracking-tight text-zinc-100", "Configuración de Kyo" }
                            p { class: "text-[11px] text-zinc-400", "Ajustes de conectividad, persistencia y motor IA" }
                        }
                    }

                    button {
                        class: "p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors cursor-pointer",
                        onclick: move |_| is_open.set(false),
                        svg {
                            class: "w-4 h-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            path { d: "M18 6L6 18M6 6l12 12" }
                        }
                    }
                }

                // Cuerpo con dos columnas: navegación lateral de pestañas + panel de contenido
                div {
                    class: "flex flex-1 min-h-0",
                    style: "display: flex; flex: 1 1 0%; min-height: 0; overflow: hidden;",

                    // Navegación de pestañas
                    aside {
                        class: "w-48 border-r border-zinc-800 p-3 space-y-1 bg-zinc-950/40",
                        style: "width: 192px; flex-shrink: 0;",
                        button {
                            class: if *active_tab.read() == SettingsTab::KyoConnection {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium bg-indigo-600/20 text-indigo-300 border border-indigo-500/30 transition-all text-left"
                            } else {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60 transition-all text-left cursor-pointer"
                            },
                            onclick: move |_| active_tab.set(SettingsTab::KyoConnection),
                            svg {
                                class: "w-4 h-4 flex-none",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                path { d: "M13 2L3 14h9l-1 8 10-12h-9l1-8z" }
                            }
                            span { "Conexión Kyo" }
                        }

                        button {
                            class: if *active_tab.read() == SettingsTab::General {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium bg-indigo-600/20 text-indigo-300 border border-indigo-500/30 transition-all text-left"
                            } else {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60 transition-all text-left cursor-pointer"
                            },
                            onclick: move |_| active_tab.set(SettingsTab::General),
                            svg {
                                class: "w-4 h-4 flex-none",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                rect { x: "2", y: "3", width: "20", height: "14", rx: "2" }
                                line { x1: "8", y1: "21", x2: "16", y2: "21" }
                                line { x1: "12", y1: "17", x2: "12", y2: "21" }
                            }
                            span { "General" }
                        }

                        button {
                            class: if *active_tab.read() == SettingsTab::About {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium bg-indigo-600/20 text-indigo-300 border border-indigo-500/30 transition-all text-left"
                            } else {
                                "w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60 transition-all text-left cursor-pointer"
                            },
                            onclick: move |_| active_tab.set(SettingsTab::About),
                            svg {
                                class: "w-4 h-4 flex-none",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                circle { cx: "12", cy: "12", r: "10" }
                                line { x1: "12", y1: "16", x2: "12", y2: "12" }
                                line { x1: "12", y1: "8", x2: "12.01", y2: "8" }
                            }
                            span { "Acerca de" }
                        }
                    }

                    // Contenido de la pestaña activa
                    main {
                        class: "flex-1 p-6 overflow-y-auto space-y-6 text-xs",
                        style: "flex: 1 1 0%; min-height: 0; overflow-y: auto;",

                        match *active_tab.read() {
                            SettingsTab::KyoConnection => rsx! {
                                // Acceso directo al Panel de Control
                                div {
                                    class: "p-3.5 rounded-xl border flex items-center justify-between gap-3 bg-indigo-950/20 border-indigo-500/30",
                                    div {
                                        class: "flex items-center gap-2.5",
                                        div {
                                            class: "w-8 h-8 rounded-lg bg-indigo-600/20 border border-indigo-500/30 flex items-center justify-center text-indigo-400",
                                            svg {
                                                class: "w-4 h-4",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                rect { x: "3", y: "3", width: "7", height: "7", rx: "1" }
                                                rect { x: "14", y: "3", width: "7", height: "7", rx: "1" }
                                                rect { x: "14", y: "14", width: "7", height: "7", rx: "1" }
                                                rect { x: "3", y: "14", width: "7", height: "7", rx: "1" }
                                            }
                                        }
                                        div {
                                            span { class: "font-semibold text-zinc-100 block", "Vista Panel de Control" }
                                            span { class: "text-zinc-400 text-[11px]", "Gestiona la conexión y token en la vista principal" }
                                        }
                                    }
                                    button {
                                        class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white font-medium text-xs transition-colors cursor-pointer shrink-0",
                                        onclick: move |_| {
                                            is_open.set(false);
                                            if let Some(mut av) = active_view_ctx {
                                                av.set(crate::models::ActiveView::Dashboard);
                                            }
                                        },
                                        span { "Abrir Panel" }
                                        svg {
                                            class: "w-3 h-3",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M5 12h14M12 5l7 7-7 7" }
                                        }
                                    }
                                }

                                // Sección: Estado del Sidecar
                                div {
                                    class: "p-4 rounded-xl bg-zinc-950/60 border border-zinc-800 space-y-3",
                                    div {
                                        class: "flex items-center justify-between",
                                        div {
                                            class: "flex items-center gap-2",
                                            span { class: "font-semibold text-zinc-200 text-xs", "Estado del Servicio Local" }
                                            span {
                                                class: "px-2 py-0.5 rounded-full text-[10px] font-medium border flex items-center gap-1.5 {badge_class}",
                                                span { class: "w-1.5 h-1.5 rounded-full {dot_class}" }
                                                span { "{state.status.label()}" }
                                            }
                                        }

                                        button {
                                            class: "flex items-center gap-1 px-2.5 py-1 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-[11px] font-medium transition-colors cursor-pointer disabled:opacity-50",
                                            disabled: *is_checking_health.read(),
                                            onclick: move |_| do_check_health(),
                                            svg {
                                                class: if *is_checking_health.read() { "w-3 h-3 animate-spin" } else { "w-3 h-3" },
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                path { d: "M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l6.71-6.19" }
                                            }
                                            span { if *is_checking_health.read() { "Comprobando..." } else { "Comprobar" } }
                                        }
                                    }

                                    div {
                                        class: "grid grid-cols-2 gap-3 pt-1 text-[11px] font-mono",
                                        div {
                                            class: "p-2.5 rounded-lg bg-zinc-900 border border-zinc-800/80",
                                            span { class: "text-zinc-500 block text-[10px]", "PUERTO SIDECAR" }
                                            span { class: "text-zinc-200 font-semibold", "{port_str}" }
                                        }
                                        div {
                                            class: "p-2.5 rounded-lg bg-zinc-900 border border-zinc-800/80",
                                            span { class: "text-zinc-500 block text-[10px]", "LATENCIA ROUNDTRIP" }
                                            span { class: "{text_class} font-semibold", "{latency_str}" }
                                        }
                                    }
                                }

                                // Sección: Cuenta de Usuario y Perfil Activo
                                if connected_username.read().is_some() || active_profile_name.read().is_some() {
                                    div {
                                        class: "p-3 rounded-xl bg-indigo-950/20 border border-indigo-500/30 space-y-2",
                                        div {
                                            class: "flex items-center justify-between",
                                            span { class: "font-semibold text-zinc-200 text-xs", "Cuenta & Perfil Web Conectado" }
                                            if let Some(ref pname) = *active_profile_name.read() {
                                                span {
                                                    class: "px-2 py-0.5 rounded-full text-[10px] font-mono bg-purple-500/20 text-purple-300 border border-purple-500/30 flex items-center gap-1",
                                                    "🗄️ {pname}"
                                                }
                                            }
                                        }
                                        div {
                                            class: "flex items-center gap-2.5 text-xs",
                                            div {
                                                class: "w-7 h-7 rounded-lg bg-indigo-600/30 border border-indigo-500/40 flex items-center justify-center text-indigo-300 font-bold text-xs shrink-0",
                                                if let Some(ref u) = *connected_username.read() {
                                                    "{u.chars().next().unwrap_or('K').to_uppercase()}"
                                                } else {
                                                    "K"
                                                }
                                            }
                                            div {
                                                class: "min-w-0",
                                                if let Some(ref u) = *connected_username.read() {
                                                    span { class: "font-semibold text-zinc-100", "@{u}" }
                                                } else {
                                                    span { class: "font-semibold text-zinc-100", "Usuario Kyō" }
                                                }
                                                if let Some(ref pname) = *active_profile_name.read() {
                                                    span { class: "text-zinc-400 text-[11px] block truncate", "Perfil activo: {pname}" }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Sección: URL Upstream Remota
                                div {
                                    class: "space-y-1.5",
                                    div {
                                        class: "flex items-center justify-between",
                                        label { class: "text-zinc-300 font-medium", "Servidor Remoto Kyo / MCP (Upstream)" }
                                        if *upstream_input.read() != "https://kyo.broco.dev" {
                                            button {
                                                class: "text-[10px] text-indigo-400 hover:text-indigo-300 transition-colors cursor-pointer",
                                                onclick: move |_| upstream_input.set("https://kyo.broco.dev".to_string()),
                                                "Restablecer Oficial"
                                            }
                                        }
                                    }
                                    input {
                                        class: "w-full px-3 py-2 rounded-xl bg-zinc-950 border border-zinc-800 text-zinc-200 text-xs font-mono focus:outline-none focus:border-indigo-500",
                                        value: "{upstream_input}",
                                        placeholder: "https://kyo.broco.dev",
                                        oninput: move |evt| upstream_input.set(evt.value()),
                                    }
                                    p { class: "text-[10px] text-zinc-500", "Endpoint oficial de sincronización e inferencia para el agente Kyo y MCP (requiere HTTPS)." }
                                }

                                // Sección: Token de Autenticación JWT
                                div {
                                    class: "space-y-2",
                                    div {
                                        class: "flex items-center justify-between",
                                        label { class: "text-zinc-300 font-medium", "Token de Autenticación (JWT / API Key)" }
                                        if *show_saved_badge.read() {
                                            span {
                                                class: "text-emerald-400 text-[11px] font-medium flex items-center gap-1 animate-fade-in",
                                                svg {
                                                    class: "w-3 h-3",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2.5",
                                                    path { d: "M20 6L9 17l-5-5" }
                                                }
                                                span { "✓ Guardado y validando" }
                                            }
                                        }
                                    }

                                    div {
                                        class: "flex items-center gap-2",
                                        div {
                                            class: "relative flex-1",
                                            input {
                                                class: "w-full px-3 py-2 pr-10 rounded-xl bg-zinc-950 border border-zinc-800 text-zinc-100 placeholder-zinc-600 focus:outline-none focus:border-indigo-500 font-mono text-xs",
                                                r#type: if *is_token_visible.read() { "text" } else { "password" },
                                                placeholder: "Pega tu Bearer token o JWT aquí...",
                                                value: "{token_input}",
                                                oninput: move |evt| token_input.set(evt.value()),
                                            }
                                            button {
                                                class: "absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300 p-1 cursor-pointer",
                                                onclick: move |_| {
                                                    let curr = *is_token_visible.read();
                                                    is_token_visible.set(!curr);
                                                },
                                                title: if *is_token_visible.read() { "Ocultar token" } else { "Mostrar token" },
                                                svg {
                                                    class: "w-3.5 h-3.5",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    path { d: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" }
                                                    circle { cx: "12", cy: "12", r: "3" }
                                                }
                                            }
                                        }

                                        button {
                                            class: "px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white font-medium text-xs shadow-md shadow-indigo-600/30 transition-all cursor-pointer flex-none",
                                            onclick: move |_| do_save_token(),
                                            "Guardar y Probar"
                                        }
                                    }

                                    if let Some(analysis) = analyze_token(&token_input.read()) {
                                        div {
                                            class: "p-3 rounded-xl bg-zinc-900/90 border border-zinc-800 space-y-2 text-[11px]",
                                            div {
                                                class: "flex items-center justify-between",
                                                span { class: "text-zinc-400 font-medium", "Análisis de Token (RFC 7519)" }
                                                if analysis.is_jwt {
                                                    if analysis.is_expired {
                                                        span { class: "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-rose-500/20 text-rose-300 border border-rose-500/30", "TOKEN EXPIRADO" }
                                                    } else {
                                                        span { class: "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30", "JWT VÁLIDO" }
                                                    }
                                                } else {
                                                    span { class: "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-amber-500/20 text-amber-300 border border-amber-500/30", "BEARER / OPACO" }
                                                }
                                            }

                                            div {
                                                class: "grid grid-cols-2 gap-2 text-zinc-300 font-mono text-[10px]",
                                                if let Some(exp_str) = analysis.expiration_str {
                                                    div {
                                                        class: "p-2 rounded bg-zinc-950/60 border border-zinc-800/60",
                                                        span { class: "text-zinc-500 block", "VIGENCIA" }
                                                        span { class: if analysis.is_expired { "text-rose-400 font-medium" } else { "text-emerald-400 font-medium" }, "{exp_str}" }
                                                    }
                                                }
                                                if let Some(alg) = analysis.alg {
                                                    div {
                                                        class: "p-2 rounded bg-zinc-950/60 border border-zinc-800/60",
                                                        span { class: "text-zinc-500 block", "ALGORITMO" }
                                                        span { class: "text-indigo-300 font-medium", "{alg}" }
                                                    }
                                                }
                                                if let Some(sub) = analysis.sub {
                                                    div {
                                                        class: "p-2 rounded bg-zinc-950/60 border border-zinc-800/60",
                                                        span { class: "text-zinc-500 block", "USUARIO (SUB)" }
                                                        span { class: "text-zinc-200 truncate block", "{sub}" }
                                                    }
                                                }
                                                if let Some(role) = analysis.role {
                                                    div {
                                                        class: "p-2 rounded bg-zinc-950/60 border border-zinc-800/60",
                                                        span { class: "text-zinc-500 block", "ROL / ACCESO" }
                                                        span { class: "text-zinc-200 uppercase", "{role}" }
                                                    }
                                                }
                                            }

                                            if let Some(warn) = analysis.warning {
                                                p {
                                                    class: "text-[10px] text-amber-400 bg-amber-950/30 border border-amber-800/40 p-2 rounded-lg flex items-center gap-1.5",
                                                    svg {
                                                        class: "w-3.5 h-3.5 flex-none",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        circle { cx: "12", cy: "12", r: "10" }
                                                        line { x1: "12", y1: "8", x2: "12", y2: "12" }
                                                        line { x1: "12", y1: "16", x2: "12.01", y2: "16" }
                                                    }
                                                    span { "{warn}" }
                                                }
                                            }
                                        }
                                    }

                                    p { class: "text-[10px] text-zinc-500", "Persistido localmente en %APPDATA%/kyo/session.json y compartido con la extensión desktop y MCP." }
                                }
                            },

                            SettingsTab::General => {
                                let search_q = font_search.read().trim().to_lowercase();
                                let mut font_list = vec!["Kyo".to_string()];
                                for f in system_fonts.read().iter() {
                                    if f != "Kyo" {
                                        font_list.push(f.clone());
                                    }
                                }
                                let filtered_fonts: Vec<String> = font_list
                                    .into_iter()
                                    .filter(|f| search_q.is_empty() || f.to_lowercase().contains(&search_q))
                                    .collect();

                                rsx! {
                                    div {
                                        class: "space-y-4",
                                        div {
                                            class: "p-4 rounded-xl bg-zinc-950/60 border border-zinc-800 flex items-center justify-between",
                                            div {
                                                h3 { class: "font-semibold text-zinc-200 text-xs", "Tema Visual" }
                                                p { class: "text-[11px] text-zinc-400", "Alterna entre modo oscuro inmersivo y modo claro diurno." }
                                            }
                                            div {
                                                class: "flex items-center gap-1.5 p-1 rounded-xl bg-zinc-900 border border-zinc-800",
                                                button {
                                                    class: if app_theme.map(|t| *t.0.read()).unwrap_or(true) {
                                                        "flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium bg-zinc-800 text-zinc-100 shadow-sm transition-all"
                                                    } else {
                                                        "flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium text-zinc-400 hover:text-zinc-200 transition-all cursor-pointer"
                                                    },
                                                    onclick: move |_| {
                                                        if let Some(mut t) = app_theme {
                                                            t.0.set(true);
                                                        }
                                                    },
                                                    svg {
                                                        class: "w-3.5 h-3.5 text-amber-300",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                                                    }
                                                    span { "Oscuro" }
                                                }
                                                button {
                                                    class: if !app_theme.map(|t| *t.0.read()).unwrap_or(true) {
                                                        "flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium bg-zinc-800 text-zinc-100 shadow-sm transition-all"
                                                    } else {
                                                        "flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium text-zinc-400 hover:text-zinc-200 transition-all cursor-pointer"
                                                    },
                                                    onclick: move |_| {
                                                        if let Some(mut t) = app_theme {
                                                            t.0.set(false);
                                                        }
                                                    },
                                                    svg {
                                                        class: "w-3.5 h-3.5 text-amber-400",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        circle { cx: "12", cy: "12", r: "5" }
                                                        line { x1: "12", y1: "1", x2: "12", y2: "3" }
                                                        line { x1: "12", y1: "21", x2: "12", y2: "23" }
                                                    }
                                                    span { "Claro" }
                                                }
                                            }
                                        }

                                        // Configuración y selector de Tipografía del Software con previsualización lateral
                                        div {
                                            class: "p-4 rounded-xl bg-zinc-950/60 border border-zinc-800 space-y-3",
                                            div {
                                                class: "flex items-center justify-between gap-4",
                                                div {
                                                    h3 { class: "font-semibold text-zinc-200 text-xs", "Tipografía de la Aplicación" }
                                                    p { class: "text-[11px] text-zinc-400", "Previsualiza el texto de ejemplo al costado derecho de cada fuente en la lista." }
                                                }
                                                div {
                                                    class: "flex items-center gap-2",
                                                    span {
                                                        class: "px-2.5 py-1 rounded-lg bg-zinc-800/80 text-zinc-300 text-[11px] font-mono",
                                                        "Activa: {active_font}"
                                                    }
                                                    if active_font != "Kyo" {
                                                        button {
                                                            class: "px-2.5 py-1 rounded-xl bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 text-[11px] transition-colors cursor-pointer flex-none border border-indigo-500/30",
                                                            onclick: move |_| {
                                                                if let Some(mut af) = app_font_ctx {
                                                                    af.0.set("Kyo".to_string());
                                                                }
                                                                local_font.set("Kyo".to_string());
                                                                let _ = save_font_family("Kyo");
                                                                let js = r#"
                                                                    document.documentElement.style.setProperty('--kyo-font-family', "'Kyo', ui-sans-serif, system-ui, sans-serif");
                                                                    if (document.body) document.body.style.setProperty('--kyo-font-family', "'Kyo', ui-sans-serif, system-ui, sans-serif");
                                                                    let st = document.getElementById('kyo-runtime-font');
                                                                    if (!st) {
                                                                        st = document.createElement('style');
                                                                        st.id = 'kyo-runtime-font';
                                                                        document.head.appendChild(st);
                                                                    }
                                                                    st.textContent = `
                                                                        :root, html, body, div, span, p, h1, h2, h3, h4, h5, h6, label, button, input, select, textarea, aside, header, main, section, nav, a {
                                                                            font-family: 'Kyo', ui-sans-serif, system-ui, sans-serif !important;
                                                                        }
                                                                        *, *::before, *::after {
                                                                            font-family: inherit;
                                                                        }
                                                                        code, kbd, samp, pre, .font-mono {
                                                                            font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace !important;
                                                                        }
                                                                    `;
                                                                "#;
                                                                let _ = document::eval(js);
                                                            },
                                                            "Restablecer a Kyo"
                                                        }
                                                    }
                                                }
                                            }

                                            // Buscador y desplegable de fuentes (cerrado por defecto)
                                            div {
                                                class: "relative",
                                                div {
                                                    class: "relative flex items-center",
                                                    input {
                                                        class: "w-full pl-8 pr-9 py-2 rounded-xl bg-zinc-900 border border-zinc-700 text-zinc-100 text-xs focus:outline-none focus:border-indigo-500 placeholder:text-zinc-500 cursor-pointer",
                                                        r#type: "text",
                                                        placeholder: if *is_font_list_open.read() { "Escribe para filtrar fuentes de tu equipo..." } else { "Haz clic para buscar o cambiar la tipografía activa..." },
                                                        value: "{font_search.read()}",
                                                        onclick: move |_| is_font_list_open.set(true),
                                                        onfocus: move |_| is_font_list_open.set(true),
                                                        oninput: move |evt: FormEvent| {
                                                            is_font_list_open.set(true);
                                                            font_search.set(evt.value());
                                                        },
                                                    }
                                                    svg {
                                                        class: "w-3.5 h-3.5 absolute left-2.5 text-zinc-500 pointer-events-none",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        circle { cx: "11", cy: "11", r: "8" }
                                                        line { x1: "21", y1: "21", x2: "16.65", y2: "16.65" }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "absolute right-2 p-1 text-zinc-400 hover:text-zinc-200 cursor-pointer transition-colors",
                                                        onclick: move |_| {
                                                            let curr = *is_font_list_open.read();
                                                            is_font_list_open.set(!curr);
                                                        },
                                                        svg {
                                                            class: if *is_font_list_open.read() { "w-3.5 h-3.5 rotate-180 transition-transform" } else { "w-3.5 h-3.5 transition-transform" },
                                                            view_box: "0 0 24 24",
                                                            fill: "none",
                                                            stroke: "currentColor",
                                                            stroke_width: "2",
                                                            polyline { points: "6 9 12 15 18 9" }
                                                        }
                                                    }
                                                }
                                            }

                                            // Listado de fuentes: SOLO se muestra si is_font_list_open es true
                                            if *is_font_list_open.read() {
                                                div {
                                                    class: "rounded-xl border border-zinc-800 overflow-hidden bg-zinc-900/95 shadow-xl animate-fade-in",
                                                    div {
                                                        class: "max-h-56 overflow-y-auto divide-y divide-zinc-800/80",
                                                        style: "max-height: 220px; height: 220px; overflow-y: auto; display: flex; flex-direction: column;",
                                                        if filtered_fonts.is_empty() {
                                                            div {
                                                                class: "p-4 text-center text-zinc-500 text-xs",
                                                                "No se encontraron fuentes coincidentes"
                                                            }
                                                        }
                                                        for f in filtered_fonts.iter() {
                                                            button {
                                                                key: "{f}",
                                                                r#type: "button",
                                                                class: if active_font == *f {
                                                                    "w-full px-3 py-2.5 text-left flex items-center justify-between gap-4 transition-colors bg-indigo-600/20 text-indigo-300 font-semibold border-l-2 border-indigo-500 cursor-pointer"
                                                                } else {
                                                                    "w-full px-3 py-2.5 text-left flex items-center justify-between gap-4 transition-colors hover:bg-zinc-800/50 text-zinc-300 cursor-pointer"
                                                                },
                                                                style: if active_font == *f {
                                                                    "display: flex; align-items: center; justify-content: space-between; width: 100%; padding: 8px 12px; gap: 16px; border-left: 3px solid #6366f1; background-color: rgba(99, 102, 241, 0.18); border-bottom: 1px solid rgba(39, 39, 42, 0.5); cursor: pointer; text-align: left;"
                                                                } else {
                                                                    "display: flex; align-items: center; justify-content: space-between; width: 100%; padding: 8px 12px; gap: 16px; background-color: transparent; border-bottom: 1px solid rgba(39, 39, 42, 0.4); cursor: pointer; text-align: left;"
                                                                },
                                                                onclick: {
                                                                    let f_click = f.clone();
                                                                    move |_| {
                                                                        if let Some(mut af) = app_font_ctx {
                                                                            af.0.set(f_click.clone());
                                                                        }
                                                                        local_font.set(f_click.clone());
                                                                        let _ = save_font_family(&f_click);

                                                                        // Aplicación inmediata en el WebView2
                                                                        let target = if f_click == "Kyo" {
                                                                            "'Kyo', ui-sans-serif, system-ui, sans-serif".to_string()
                                                                        } else {
                                                                            format!("'{f_click}', 'Kyo', ui-sans-serif, sans-serif")
                                                                        };
                                                                        let js = format!(
                                                                            r#"
                                                                            document.documentElement.style.setProperty('--kyo-font-family', "{target}");
                                                                            if (document.body) document.body.style.setProperty('--kyo-font-family', "{target}");
                                                                            let st = document.getElementById('kyo-runtime-font');
                                                                            if (!st) {{
                                                                                st = document.createElement('style');
                                                                                st.id = 'kyo-runtime-font';
                                                                                document.head.appendChild(st);
                                                                            }}
                                                                            st.textContent = `
                                                                                :root, html, body, div, span, p, h1, h2, h3, h4, h5, h6, label, button, input, select, textarea, aside, header, main, section, nav, a {{
                                                                                    font-family: {target} !important;
                                                                                }}
                                                                                *, *::before, *::after {{
                                                                                    font-family: inherit;
                                                                                }}
                                                                                code, kbd, samp, pre, .font-mono {{
                                                                                    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace !important;
                                                                                }}
                                                                            `;
                                                                            "#
                                                                        );
                                                                        let _ = document::eval(&js);

                                                                        // Cierra la lista tras la selección
                                                                        is_font_list_open.set(false);
                                                                    }
                                                                },
                                                                // Lado izquierdo: indicador y nombre de la fuente
                                                                div {
                                                                    class: "flex items-center gap-2 min-w-[150px] shrink-0",
                                                                    style: "display: flex; align-items: center; gap: 8px; flex-shrink: 0; min-width: 150px; max-width: 45%; overflow: hidden;",
                                                                    if active_font == *f {
                                                                        svg {
                                                                            class: "w-3.5 h-3.5 text-indigo-400 shrink-0",
                                                                            style: "width: 14px; height: 14px; color: #818cf8; flex-shrink: 0;",
                                                                            view_box: "0 0 24 24",
                                                                            fill: "none",
                                                                            stroke: "currentColor",
                                                                            stroke_width: "2.5",
                                                                            path { d: "M20 6L9 17l-5-5" }
                                                                        }
                                                                    } else {
                                                                        span { class: "w-3.5 shrink-0", style: "width: 14px; flex-shrink: 0;" }
                                                                    }
                                                                    span {
                                                                        class: "text-xs truncate font-medium",
                                                                        style: "font-size: 12px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                                                        "{f}"
                                                                    }
                                                                    if f == "Kyo" {
                                                                        span {
                                                                            class: "text-[9px] px-1.5 py-0.5 rounded bg-indigo-500/20 text-indigo-400 font-normal shrink-0",
                                                                            style: "font-size: 9px; padding: 2px 6px; border-radius: 4px; background-color: rgba(99, 102, 241, 0.2); color: #818cf8; font-weight: 400; flex-shrink: 0;",
                                                                            "Oficial"
                                                                        }
                                                                    }
                                                                }

                                                                // Costado derecho del listado: Texto de muestra con la tipografía aplicada
                                                                div {
                                                                    class: "text-xs truncate text-right opacity-85 hover:opacity-100 flex-1",
                                                                    style: format!("font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; flex: 1 1 0%; opacity: 0.85; font-family: '{f}', 'Kyo', ui-sans-serif, sans-serif;"),
                                                                    "El veloz murciélago 0123456789 (Aa Bb Gg)"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Vista previa de la tipografía activa (se muestra cuando la lista está cerrada)
                                            if !*is_font_list_open.read() {
                                                div {
                                                    class: "p-3.5 rounded-xl border border-dashed border-zinc-800 bg-zinc-900/40 text-zinc-300 space-y-1 transition-all cursor-pointer hover:border-zinc-700",
                                                    style: format!("font-family: '{}', 'Kyo', ui-sans-serif, sans-serif;", active_font),
                                                    onclick: move |_| is_font_list_open.set(true),
                                                    div {
                                                        class: "flex items-center justify-between text-[10px] text-zinc-500 uppercase tracking-wider font-sans",
                                                        span { "Tipografía activa en el software: {active_font}" }
                                                        span { class: "text-[10px] text-zinc-600 font-mono", if active_font == "Kyo" { "Tipografía Oficial de Kyo" } else { "Fuente local de tu sistema" } }
                                                    }
                                                    p { class: "text-xs font-semibold leading-relaxed", "El veloz murciélago hindú comía feliz cardillo y kiwi. 0123456789" }
                                                    p { class: "text-[11px] text-zinc-400", "The quick brown fox jumps over the lazy dog. Aa Bb Gg Qq Ww Zz" }
                                                }
                                            }
                                        }

                                        div {
                                            class: "p-4 rounded-xl bg-zinc-950/60 border border-zinc-800 flex items-center justify-between",
                                            div {
                                                h3 { class: "font-semibold text-zinc-200 text-xs", "Directorio de Réplicas SQLite" }
                                                p { class: "text-[11px] text-zinc-400 font-mono truncate max-w-sm", "%APPDATA%\\kyo\\replicas" }
                                            }
                                            span { class: "px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-[10px]", "Activo" }
                                        }
                                    }
                                }
                            },

                            SettingsTab::About => rsx! {
                                div {
                                    class: "space-y-4 text-center py-6",
                                    div {
                                        class: "w-16 h-16 mx-auto rounded-2xl bg-gradient-to-tr from-indigo-600 to-violet-600 flex items-center justify-center text-white shadow-lg shadow-indigo-500/25",
                                        svg {
                                            class: "w-8 h-8 fill-none stroke-current stroke-2",
                                            view_box: "0 0 24 24",
                                            path { d: "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" }
                                        }
                                    }
                                    div {
                                        h3 { class: "text-base font-bold text-zinc-100", "Kyo Desktop" }
                                        p { class: "text-xs text-indigo-400 font-mono", "v0.2.0 • Rust Nativo Single-Runtime" }
                                    }
                                    p { class: "text-xs text-zinc-400 max-w-md mx-auto leading-relaxed",
                                        "Aplicación de escritorio de alta eficiencia construida íntegramente con Rust y Dioxus 0.7, con microservicio local en Go para sincronización Offline-First."
                                    }
                                    div {
                                        class: "pt-4 border-t border-zinc-800/80 flex items-center justify-center gap-4 text-[11px] text-zinc-500 font-mono",
                                        span { "Engine: Dioxus 0.7 (Tao/Wry)" }
                                        span { "•" }
                                        span { "Sidecar: Go 1.22+" }
                                        span { "•" }
                                        span { "Zero-Tauri Consolidation" }
                                    }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
