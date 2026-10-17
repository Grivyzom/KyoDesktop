//! Panel de control exclusivo para la conexión con la plataforma Web y Backend de Kyō.
//!
//! Diseño dirigido por estado:
//! - **Desconectado / Error:** la tarjeta "Clave de Acceso (Token Key)" ocupa el primer plano
//!   con un paso a paso (Obtener token → Pegar token → Conectar y sincronizar).
//! - **Conectado:** banner con identidad + telemetría, recursos de Kyō Web (bóveda principal,
//!   espacios y bóvedas) en la columna principal y credenciales colapsadas en un acordeón.
//!
//! El estado de la conexión vive en el contexto [`KyoCloud`] para que la barra lateral
//! refleje los espacios de la nube y para que sobreviva al cambio de vista.

use dioxus::prelude::*;
use crate::models::ActiveView;
use crate::settings::{
    analyze_token, clear_session_auth, load_auth_token, load_server_url,
    save_full_session_config,
};
use crate::sidecar::{SidecarConnectionStatus, SidecarState};

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");

pub(crate) const KYO_WEB_URL: &str = "https://kyo.broco.dev";

// ── Iconos (trazos estilo Lucide, un único `path`) ──────────────────────────
pub(crate) const ICON_CHECK: &str = "M20 6L9 17l-5-5";
pub(crate) const ICON_REFRESH: &str = "M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6";
pub(crate) const ICON_ARROW_RIGHT: &str = "M5 12h14M12 5l7 7-7 7";
pub(crate) const ICON_EXTERNAL: &str = "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3";
pub(crate) const ICON_KEY: &str = "M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4";
pub(crate) const ICON_LOCK: &str = "M5 11h14v10H5zM7 11V7a5 5 0 0 1 10 0v4";
pub(crate) const ICON_VAULT: &str = "M21 8v13H3V8M1 3h22v5H1zM10 12h4";
pub(crate) const ICON_FOLDER: &str = "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z";
pub(crate) const ICON_CHAT: &str = "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z";
pub(crate) const ICON_CHEVRON: &str = "M9 18l6-6-6-6";
pub(crate) const ICON_POWER: &str = "M18.36 6.64a9 9 0 1 1-12.73 0M12 2v10";
pub(crate) const ICON_ALERT: &str = "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 8v4M12 16h.01";
pub(crate) const ICON_WIFI_OFF: &str = "M1 1l22 22M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.58 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01";
pub(crate) const ICON_SERVER: &str = "M2 4h20v6H2zM2 14h20v6H2zM6 7h.01M6 17h.01";
pub(crate) const ICON_CLOCK: &str = "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 6v6l4 2";
pub(crate) const ICON_ACTIVITY: &str = "M22 12h-4l-3 9L9 3l-3 9H2";
pub(crate) const ICON_SIDEBAR: &str = "M3 3h18v18H3zM9 3v18M13 9l3 3-3 3";
pub(crate) const ICON_EYE: &str = "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z";
pub(crate) const ICON_EYE_OFF: &str = "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24M1 1l22 22";
pub(crate) const ICON_X: &str = "M18 6L6 18M6 6l12 12";
pub(crate) const ICON_SHIELD: &str = "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z";
pub(crate) const ICON_GRID: &str = "M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z";
pub(crate) const ICON_CLOUD: &str = "M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z";

/// Icono SVG de trazo único. `class` controla tamaño/color (`dk-ic`, `dk-ic-sm`, `dk-ic-lg`).
#[component]
pub(crate) fn Icon(d: &'static str, #[props(into, default = "dk-ic".to_string())] class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d }
        }
    }
}

/// Tipo de recurso alojado en Kyō Web.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CloudResourceKind {
    Space,
    Vault,
}

/// Espacio de trabajo o bóveda sincronizada desde el backend de Kyō.
#[derive(Clone, Debug, PartialEq)]
pub struct CloudResource {
    pub id: String,
    pub name: String,
    pub kind: CloudResourceKind,
}

/// Información de la cuenta y recursos detectados en el backend de Kyō.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedAccountInfo {
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub role: Option<String>,
    pub spaces_count: Option<usize>,
    pub vaults_count: Option<usize>,
    pub primary_vault: Option<String>,
    /// Primera página de espacios devuelta por `/api/v1/spaces`.
    pub spaces: Vec<CloudResource>,
    /// Primera página de bóvedas devuelta por `/api/v1/vaults`.
    pub vaults: Vec<CloudResource>,
}

impl ConnectedAccountInfo {
    /// Bóveda principal: la primera bóveda devuelta por el servidor.
    pub fn primary_vault_resource(&self) -> Option<CloudResource> {
        self.vaults.first().cloned().or_else(|| {
            self.primary_vault.clone().map(|name| CloudResource {
                id: name.clone(),
                name,
                kind: CloudResourceKind::Vault,
            })
        })
    }
}

/// Estado de la conexión con la plataforma y backend de Kyō.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected {
        latency_ms: u64,
        server_url: String,
        last_checked: String,
        account: Option<ConnectedAccountInfo>,
    },
    Error {
        message: String,
        is_auth_error: bool,
        status_code: Option<u16>,
        server_url: String,
    },
}

/// Estado compartido de Kyō Web (proveído en la raíz de la app).
///
/// - `status`: estado de la conexión y recursos sincronizados.
/// - `active_context`: espacio/bóveda elegido para iniciar nuevos chats.
#[derive(Clone, Copy, PartialEq)]
pub struct KyoCloud {
    pub status: Signal<ConnectionStatus>,
    pub active_context: Signal<Option<CloudResource>>,
}

impl KyoCloud {
    pub fn new() -> Self {
        Self {
            status: Signal::new(ConnectionStatus::Disconnected),
            active_context: Signal::new(None),
        }
    }

    /// Cuenta conectada, si la sesión está activa.
    pub fn account(&self) -> Option<ConnectedAccountInfo> {
        match &*self.status.read() {
            ConnectionStatus::Connected { account, .. } => account.clone(),
            _ => None,
        }
    }
}

/// Obtiene el contexto [`KyoCloud`]; si la app no lo provee, crea uno local.
pub fn use_kyo_cloud() -> KyoCloud {
    match try_use_context::<KyoCloud>() {
        Some(c) => c,
        None => use_context_provider(KyoCloud::new),
    }
}

/// Formatea una marca temporal básica en UTC para la última verificación.
fn get_current_time_str() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sec = now % 60;
    let min = (now / 60) % 60;
    let hr = (now / 3600) % 24;
    format!("{:02}:{:02}:{:02} UTC", hr, min, sec)
}

/// Abre una URL en el navegador predeterminado del sistema operativo.
pub(crate) fn open_in_browser(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Extrae `{ id, name }` del arreglo `data` de una respuesta paginada.
fn parse_resources(json: &serde_json::Value, kind: CloudResourceKind) -> Vec<CloudResource> {
    json.get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let name = item
                        .get("name")
                        .or_else(|| item.get("title"))
                        .and_then(|n| n.as_str())?
                        .trim()
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let id = match item.get("id") {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(v) if !v.is_null() => v.to_string(),
                        _ => name.clone(),
                    };
                    Some(CloudResource { id, name, kind })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Lee `total` o, en su defecto, la longitud de `data`.
fn parse_total(json: &serde_json::Value) -> Option<usize> {
    json.get("total")
        .and_then(|t| t.as_u64())
        .map(|c| c as usize)
        .or_else(|| json.get("data").and_then(|d| d.as_array()).map(|a| a.len()))
}

/// Realiza una sonda HTTP al backend de Kyō para validar credenciales y medir latencia.
pub async fn probe_kyo_backend(
    server_url: &str,
    token: &str,
) -> Result<(u64, ConnectedAccountInfo), (String, bool, Option<u16>)> {
    let clean_url = server_url.trim().trim_end_matches('/').to_string();
    if clean_url.is_empty() {
        return Err(("La URL del servidor no puede estar vacía.".to_string(), false, None));
    }

    // Validación de protocolo seguro para evitar transmisión de token en texto plano
    if clean_url.starts_with("http://") {
        let is_local = clean_url.starts_with("http://localhost")
            || clean_url.starts_with("http://127.0.0.1")
            || clean_url.starts_with("http://[::1]");
        if !is_local {
            return Err((
                "Por seguridad, las conexiones remotas deben utilizar HTTPS para proteger su token.".to_string(),
                false,
                None,
            ));
        }
    } else if !clean_url.starts_with("https://") {
        return Err((
            "Protocolo no válido. La URL debe comenzar con https:// (o http:// para red local).".to_string(),
            false,
            None,
        ));
    }

    let trimmed_token = token.trim().to_string();
    if trimmed_token.is_empty() {
        return Err((
            "No se ha ingresado ningún Token Key. Ingrese su token para conectar.".to_string(),
            true,
            None,
        ));
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(4500))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err((format!("Error inicializando cliente HTTP: {e}"), false, None)),
    };

    let api_base = if clean_url.ends_with("/api/v1") {
        clean_url.clone()
    } else {
        format!("{clean_url}/api/v1")
    };
    let spaces_url = format!("{api_base}/spaces");

    let start = std::time::Instant::now();
    let res = match client
        .get(&spaces_url)
        .header("Authorization", format!("Bearer {trimmed_token}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err((
                format!("No se pudo conectar con el servidor '{clean_url}': {e}"),
                false,
                None,
            ));
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    let status_code = res.status().as_u16();

    if res.status() == reqwest::StatusCode::UNAUTHORIZED || res.status() == reqwest::StatusCode::FORBIDDEN {
        return Err((
            "Token de autenticación rechazado (HTTP 401 Unauthorized). Verifique que el Token Key sea válido y no haya expirado.".to_string(),
            true,
            Some(status_code),
        ));
    }

    if !res.status().is_success() {
        return Err((
            format!("El servidor respondió con código HTTP {status_code} al consultar espacios."),
            false,
            Some(status_code),
        ));
    }

    // Procesar la respuesta JSON de espacios
    let spaces_json: serde_json::Value = res.json().await.unwrap_or_default();
    let spaces_count = parse_total(&spaces_json);
    let spaces = parse_resources(&spaces_json, CloudResourceKind::Space);

    let mut user_id: Option<i64> = None;
    let mut username: Option<String> = None;

    if let Some(arr) = spaces_json.get("data").and_then(|d| d.as_array()) {
        for item in arr {
            if let Some(last_edit) = item.get("last_edit") {
                if user_id.is_none() {
                    user_id = last_edit.get("by_id").and_then(|id| id.as_i64());
                }
                if username.is_none() {
                    if let Some(uname) = last_edit.get("by_name").and_then(|n| n.as_str()) {
                        let tu = uname.trim();
                        if !tu.is_empty() {
                            username = Some(tu.to_string());
                        }
                    }
                }
            }
            if user_id.is_some() && username.is_some() {
                break;
            }
        }
    }

    // Consultar bóvedas si es posible
    let vaults_url = format!("{api_base}/vaults");
    let mut vaults_count: Option<usize> = None;
    let mut vaults: Vec<CloudResource> = Vec::new();

    if let Ok(v_res) = client
        .get(&vaults_url)
        .header("Authorization", format!("Bearer {trimmed_token}"))
        .send()
        .await
    {
        if v_res.status().is_success() {
            if let Ok(v_json) = v_res.json::<serde_json::Value>().await {
                vaults_count = parse_total(&v_json);
                vaults = parse_resources(&v_json, CloudResourceKind::Vault);
            }
        }
    }
    let primary_vault = vaults.first().map(|v| v.name.clone());

    // Análisis estático del JWT para claims adicionales
    let jwt_data = analyze_token(&trimmed_token);
    let mut role: Option<String> = None;
    if let Some(ref j) = jwt_data {
        role = j.role.clone();
        if username.is_none() {
            username = j.sub.clone();
        }
    }

    if username.is_none() {
        username = crate::settings::load_username();
    }

    let account = ConnectedAccountInfo {
        user_id,
        username: username.clone(),
        role: role.or_else(|| Some("user".to_string())),
        spaces_count,
        vaults_count,
        primary_vault: primary_vault.clone(),
        spaces,
        vaults,
    };

    // Guardar en sesión local de Windows (%APPDATA%/kyo/session.json)
    let _ = save_full_session_config(
        &trimmed_token,
        &clean_url,
        username.as_deref(),
        None,
        primary_vault.as_deref(),
        None,
    );

    Ok((latency_ms, account))
}

/// Vista "Panel de control": puente visual entre Kyō Web y el escritorio.
#[component]
pub fn Dashboard() -> Element {
    let sidecar_state = try_use_context::<Signal<SidecarState>>();
    let mut active_view = use_context::<Signal<ActiveView>>();
    let cloud = use_kyo_cloud();
    let mut connection_status = cloud.status;
    let mut active_context = cloud.active_context;

    // Estados del formulario y credenciales
    let mut token_input = use_signal(load_auth_token);
    let server_url_input = use_signal(load_server_url);
    let mut is_verifying = use_signal(|| false);
    let mut last_error_msg = use_signal(|| Option::<String>::None);
    let flash_msg = use_signal(|| Option::<String>::None);
    let mut show_credentials = use_signal(|| false);
    let mut show_vault_explorer = use_signal(|| false);

    // Sonda al backend. `success_msg` = acción explícita del usuario (muestra aviso y
    // actualiza el estado del sidecar); `None` = verificación silenciosa al abrir la vista.
    let mut run_probe = move |success_msg: Option<&'static str>| {
        let raw_token = token_input.peek().clone();
        let raw_url = server_url_input.peek().clone();

        if raw_token.trim().is_empty() {
            last_error_msg.set(Some("Por favor ingrese un Token Key antes de conectar.".to_string()));
            connection_status.set(ConnectionStatus::Disconnected);
            return;
        }

        last_error_msg.set(None);
        is_verifying.set(true);
        // Durante una re-sincronización se mantiene visible la información actual.
        if !matches!(*connection_status.peek(), ConnectionStatus::Connected { .. }) {
            connection_status.set(ConnectionStatus::Connecting);
        }

        let mut conn_sig = connection_status;
        let mut verify_sig = is_verifying;
        let mut flash_sig = flash_msg;
        let sc_opt = sidecar_state;
        let explicit = success_msg.is_some();

        spawn(async move {
            let ok = match probe_kyo_backend(&raw_url, &raw_token).await {
                Ok((lat, acc)) => {
                    conn_sig.set(ConnectionStatus::Connected {
                        latency_ms: lat,
                        server_url: raw_url.clone(),
                        last_checked: get_current_time_str(),
                        account: Some(acc),
                    });
                    if let Some(mut sc) = sc_opt {
                        let mut w = sc.write();
                        w.latency_ms = Some(lat);
                        if w.status == SidecarConnectionStatus::Disconnected {
                            w.status = SidecarConnectionStatus::FallbackCloud;
                        }
                    }
                    true
                }
                Err((err, is_auth, code)) => {
                    conn_sig.set(ConnectionStatus::Error {
                        message: err,
                        is_auth_error: is_auth,
                        status_code: code,
                        server_url: raw_url,
                    });
                    if explicit {
                        if let Some(mut sc) = sc_opt {
                            sc.write().status = SidecarConnectionStatus::Disconnected;
                        }
                    }
                    false
                }
            };
            verify_sig.set(false);

            if let (true, Some(msg)) = (ok, success_msg) {
                flash_sig.set(Some(msg.to_string()));
                tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                flash_sig.set(None);
            }
        });
    };

    // Auto-verificación al abrir la vista si hay token guardado y aún no hay sesión activa.
    // Solo usa `peek()`, por lo que el efecto no se suscribe y corre una única vez.
    use_effect(move || {
        let has_token = !token_input.peek().trim().is_empty();
        let is_idle = matches!(*connection_status.peek(), ConnectionStatus::Disconnected);
        if has_token && is_idle {
            run_probe(None);
        }
    });

    // Acción: Desconectar y limpiar credenciales
    let mut handle_disconnect = move || {
        let _ = clear_session_auth();
        token_input.set(String::new());
        connection_status.set(ConnectionStatus::Disconnected);
        active_context.set(None);
        last_error_msg.set(None);
        show_credentials.set(false);
        show_vault_explorer.set(false);

        if let Some(mut sc) = sidecar_state {
            let mut w = sc.write();
            w.status = SidecarConnectionStatus::Disconnected;
            w.latency_ms = None;
        }
    };

    let is_loading = *is_verifying.read();
    let status_snapshot = connection_status.read().clone();
    let server_url_now = server_url_input.read().clone();

    rsx! {
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        div {
            class: "dk-root",

            // ── Cabecera Superior del Panel ─────────────────────────────────
            header {
                class: "dk-topbar",
                div {
                    class: "flex items-center gap-3",
                    div { class: "dk-topbar-icon", Icon { d: ICON_GRID } }
                    h1 { class: "dk-topbar-title", "Panel de control" }
                    span { class: "dk-chip dk-chip-mono", "Conexión Kyō Web" }
                }
                {
                    match &status_snapshot {
                        ConnectionStatus::Connected { .. } => rsx! {
                            span { class: "dk-status-pill dk-status-ok", span { class: "dk-dot dk-dot-ok animate-pulse" } "Conectado" }
                        },
                        ConnectionStatus::Connecting => rsx! {
                            span { class: "dk-status-pill dk-status-info", span { class: "dk-dot dk-dot-info animate-ping" } "Comprobando..." }
                        },
                        ConnectionStatus::Error { is_auth_error, .. } => rsx! {
                            span {
                                class: if *is_auth_error { "dk-status-pill dk-status-warn" } else { "dk-status-pill dk-status-bad" },
                                span { class: "dk-dot dk-dot-current" }
                                if *is_auth_error { "Token inválido" } else { "Servidor inaccesible" }
                            }
                        },
                        ConnectionStatus::Disconnected => rsx! {
                            span { class: "dk-status-pill dk-status-off", span { class: "dk-dot dk-dot-off" } "Desconectado" }
                        },
                    }
                }
            }

            main {
                class: "dk-scroll",
                div {
                    class: "dashboard-canvas",

                    // ════════════════════════════════════════════════════════
                    // 1. BANNER DE ESTADO
                    // ════════════════════════════════════════════════════════
                    {
                        match &status_snapshot {
                            ConnectionStatus::Connected { latency_ms, server_url, last_checked, account } => {
                                let acc = account.clone();
                                let name = acc.as_ref().and_then(|a| a.username.clone()).unwrap_or_else(|| "Usuario autenticado".to_string());
                                let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "K".to_string());
                                let uid = acc.as_ref().and_then(|a| a.user_id).map(|id| format!("#{id}")).unwrap_or_else(|| "--".to_string());
                                let role = acc.as_ref().and_then(|a| a.role.clone()).unwrap_or_else(|| "user".to_string()).to_uppercase();
                                let host = server_url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string();
                                let lat_class = if *latency_ms < 150 { "dk-lat-good" } else if *latency_ms < 400 { "dk-lat-warn" } else { "dk-lat-bad" };
                                rsx! {
                                    section {
                                        class: "dk-hero dk-hero-ok",
                                        div {
                                            class: "dk-hero-identity",
                                            div { class: "dk-avatar", "{initial}" }
                                            div {
                                                class: "dk-hero-text",
                                                div {
                                                    class: "dk-hero-title-row",
                                                    h2 { class: "dk-hero-name", "{name}" }
                                                    span { class: "dk-chip dk-chip-mono", "ID: {uid}" }
                                                    span { class: "dk-chip dk-chip-accent", "{role}" }
                                                }
                                                div {
                                                    class: "dk-hero-meta",
                                                    span { class: "dk-status-pill dk-status-ok", span { class: "dk-dot dk-dot-ok animate-pulse" } "ONLINE 200 OK" }
                                                    span { class: "dk-meta-item", title: "Latencia RTT",
                                                        Icon { d: ICON_ACTIVITY, class: "dk-ic-sm" }
                                                        span { class: "dk-lat {lat_class}", "{latency_ms} ms" }
                                                    }
                                                    span { class: "dk-meta-item",
                                                        Icon { d: ICON_CLOCK, class: "dk-ic-sm" }
                                                        "Última sincronización: {last_checked}"
                                                    }
                                                    span { class: "dk-meta-item",
                                                        Icon { d: ICON_SERVER, class: "dk-ic-sm" }
                                                        "{host}"
                                                    }
                                                }
                                            }
                                        }
                                        div {
                                            class: "dk-hero-actions",
                                            button {
                                                class: "dk-btn dk-btn-primary",
                                                disabled: is_loading,
                                                onclick: move |_| run_probe(Some("Datos sincronizados con Kyō Web.")),
                                                Icon { d: ICON_REFRESH, class: if is_loading { "dk-ic animate-spin" } else { "dk-ic" } }
                                                if is_loading { "Sincronizando..." } else { "Sincronizar datos ahora" }
                                            }
                                            button {
                                                class: "dk-btn dk-btn-secondary",
                                                onclick: move |_| active_view.set(ActiveView::Chat),
                                                "Ir al Chat de Kyō"
                                                Icon { d: ICON_ARROW_RIGHT }
                                            }
                                        }
                                    }
                                }
                            }
                            ConnectionStatus::Connecting => rsx! {
                                section {
                                    class: "dk-hero dk-hero-info",
                                    div {
                                        class: "dk-hero-identity",
                                        div { class: "dk-hero-icon dk-tone-info", Icon { d: ICON_REFRESH, class: "dk-ic-lg animate-spin" } }
                                        div {
                                            class: "dk-hero-text",
                                            h2 { class: "dk-hero-name", "Comprobando conexión con Kyō Web..." }
                                            p { class: "dk-muted", "Verificando el Token Key y midiendo la latencia de los servicios." }
                                        }
                                    }
                                }
                            },
                            ConnectionStatus::Error { message, is_auth_error, status_code, server_url } => rsx! {
                                section {
                                    class: if *is_auth_error { "dk-hero dk-hero-warn" } else { "dk-hero dk-hero-bad" },
                                    div {
                                        class: "dk-hero-identity",
                                        div {
                                            class: if *is_auth_error { "dk-hero-icon dk-tone-warn" } else { "dk-hero-icon dk-tone-bad" },
                                            Icon { d: ICON_ALERT, class: "dk-ic-lg" }
                                        }
                                        div {
                                            class: "dk-hero-text",
                                            div {
                                                class: "dk-hero-title-row",
                                                h2 { class: "dk-hero-name",
                                                    if *is_auth_error { "Error de autenticación con Kyō" } else { "Error de conectividad con el backend" }
                                                }
                                                if let Some(code) = status_code {
                                                    span { class: "dk-chip dk-chip-mono", "HTTP {code}" }
                                                }
                                            }
                                            p { class: "dk-body", "{message}" }
                                            p { class: "dk-muted dk-mono", "Servidor consultado: {server_url}" }
                                        }
                                    }
                                    div {
                                        class: "dk-hero-actions",
                                        button {
                                            class: "dk-btn dk-btn-secondary",
                                            disabled: is_loading,
                                            onclick: move |_| run_probe(Some("Conexión restablecida con Kyō Web.")),
                                            Icon { d: ICON_REFRESH, class: if is_loading { "dk-ic animate-spin" } else { "dk-ic" } }
                                            "Reintentar"
                                        }
                                    }
                                }
                            },
                            ConnectionStatus::Disconnected => rsx! {
                                section {
                                    class: "dk-hero dk-hero-off",
                                    div {
                                        class: "dk-hero-identity",
                                        div { class: "dk-hero-icon dk-tone-off", Icon { d: ICON_WIFI_OFF, class: "dk-ic-lg" } }
                                        div {
                                            class: "dk-hero-text",
                                            div {
                                                class: "dk-hero-title-row",
                                                h2 { class: "dk-hero-name", "Conecta tu escritorio con Kyō Web" }
                                                span { class: "dk-chip dk-chip-mono", "OFFLINE" }
                                            }
                                            p { class: "dk-muted", "Vincula tu cuenta para abrir tus Espacios de Trabajo y Bóvedas de la nube directamente en esta aplicación." }
                                        }
                                    }
                                }
                            },
                        }
                    }

                    if let Some(msg) = flash_msg.read().as_ref() {
                        div {
                            class: "dk-flash animate-fade-in",
                            Icon { d: ICON_CHECK }
                            span { "{msg}" }
                        }
                    }

                    // ════════════════════════════════════════════════════════
                    // 2. BENTO GRID (1.7fr | 1fr)
                    // ════════════════════════════════════════════════════════
                    {
                        if let ConnectionStatus::Connected { account, .. } = &status_snapshot {
                            let acc = account.clone().unwrap_or(ConnectedAccountInfo {
                                user_id: None, username: None, role: None, spaces_count: None,
                                vaults_count: None, primary_vault: None, spaces: Vec::new(), vaults: Vec::new(),
                            });
                            let ctx_now = active_context.read().clone();
                            let shown_vault = ctx_now
                                .clone()
                                .filter(|c| c.kind == CloudResourceKind::Vault)
                                .or_else(|| acc.primary_vault_resource());
                            let is_primary = shown_vault.as_ref().map(|v| acc.vaults.first().map(|f| f.id == v.id).unwrap_or(true)).unwrap_or(true);
                            let vault_label = if is_primary { "Bóveda principal" } else { "Bóveda activa" };
                            let vault_name = shown_vault.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| "Sin bóvedas".to_string());
                            let has_vault = shown_vault.is_some();
                            let spaces_total = acc.spaces_count.unwrap_or(acc.spaces.len());
                            let vaults_total = acc.vaults_count.unwrap_or(acc.vaults.len());
                            let space_preview: Vec<CloudResource> = acc.spaces.iter().take(4).cloned().collect();
                            let extra_spaces = spaces_total.saturating_sub(space_preview.len());
                            let vault_list = acc.vaults.clone();
                            let explorer_open = *show_vault_explorer.read();
                            let creds_open = *show_credentials.read();

                            rsx! {
                                div {
                                    class: "dk-bento",

                                    // ── Columna izquierda: Recursos de Kyō Web ──
                                    section {
                                        class: "dk-card dk-col-main",
                                        div {
                                            class: "dk-card-head",
                                            div { class: "dk-card-head-icon dk-tone-accent", Icon { d: ICON_CLOUD } }
                                            div {
                                                class: "dk-card-head-text",
                                                h3 { "Recursos Sincronizados desde Kyō Web" }
                                                p { class: "dk-muted", "Tus espacios y bóvedas de la nube, listos para usarse en el escritorio." }
                                            }
                                            button {
                                                class: "dk-btn dk-btn-ghost",
                                                onclick: move |_| open_in_browser(KYO_WEB_URL),
                                                "Abrir Kyō Web"
                                                Icon { d: ICON_EXTERNAL, class: "dk-ic-sm" }
                                            }
                                        }

                                        // Bóveda principal destacada
                                        div {
                                            class: "dk-vault",
                                            div {
                                                class: "dk-vault-main",
                                                div { class: "dk-vault-icon", Icon { d: ICON_VAULT, class: "dk-ic-lg" } }
                                                div {
                                                    class: "dk-vault-text",
                                                    span { class: "dk-eyebrow", "{vault_label}" }
                                                    h4 { class: "dk-vault-name", "{vault_name}" }
                                                    if has_vault {
                                                        span { class: "dk-sync-badge", span { class: "dk-dot dk-dot-ok animate-pulse" } "Sincronizada en tiempo real" }
                                                    }
                                                }
                                            }
                                            if has_vault {
                                                div {
                                                    class: "dk-vault-actions",
                                                    button {
                                                        class: if explorer_open { "dk-btn dk-btn-secondary is-active" } else { "dk-btn dk-btn-secondary" },
                                                        onclick: move |_| {
                                                            let cur = *show_vault_explorer.read();
                                                            show_vault_explorer.set(!cur);
                                                        },
                                                        Icon { d: ICON_FOLDER }
                                                        if explorer_open { "Cerrar explorador" } else { "Abrir Bóveda en Escritorio" }
                                                    }
                                                    {
                                                        let target = shown_vault.clone();
                                                        rsx! {
                                                            button {
                                                                class: "dk-btn dk-btn-primary",
                                                                onclick: move |_| {
                                                                    active_context.set(target.clone());
                                                                    active_view.set(ActiveView::Chat);
                                                                },
                                                                Icon { d: ICON_CHAT }
                                                                "Usar en Nuevo Chat"
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            if explorer_open {
                                                div {
                                                    class: "dk-vault-explorer animate-fade-in",
                                                    div { class: "dk-explorer-title", "Bóvedas en tu cuenta ({vaults_total})" }
                                                    if vault_list.is_empty() {
                                                        p { class: "dk-muted", "El servidor no devolvió bóvedas para esta cuenta." }
                                                    }
                                                    for v in vault_list.iter() {
                                                        {
                                                            let is_current = shown_vault.as_ref().map(|s| s.id == v.id).unwrap_or(false);
                                                            let pick = v.clone();
                                                            rsx! {
                                                                div {
                                                                    key: "{v.id}",
                                                                    class: if is_current { "dk-explorer-row is-current" } else { "dk-explorer-row" },
                                                                    span { class: "dk-explorer-name", Icon { d: ICON_VAULT, class: "dk-ic-sm" } "{v.name}" }
                                                                    if is_current {
                                                                        span { class: "dk-chip dk-chip-ok", "Activa" }
                                                                    } else {
                                                                        button {
                                                                            class: "dk-btn dk-btn-ghost dk-btn-xs",
                                                                            onclick: move |_| active_context.set(Some(pick.clone())),
                                                                            "Seleccionar"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Resumen de espacios y bóvedas en la nube
                                        div {
                                            class: "dk-res-grid",
                                            div {
                                                class: "dk-res-tile",
                                                div { class: "dk-res-head",
                                                    span { class: "dk-res-icon dk-tone-accent", Icon { d: ICON_FOLDER } }
                                                    span { class: "dk-eyebrow", "Espacios de trabajo" }
                                                }
                                                div { class: "dk-res-value", "{spaces_total}" }
                                                div { class: "dk-muted", "en la nube" }
                                                if !space_preview.is_empty() {
                                                    div {
                                                        class: "dk-res-chips",
                                                        for s in space_preview.iter() {
                                                            {
                                                                let pick = s.clone();
                                                                rsx! {
                                                                    button {
                                                                        key: "{s.id}",
                                                                        class: "dk-res-chip",
                                                                        title: "Iniciar chat con este espacio",
                                                                        onclick: move |_| {
                                                                            active_context.set(Some(pick.clone()));
                                                                            active_view.set(ActiveView::Chat);
                                                                        },
                                                                        "{s.name}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if extra_spaces > 0 {
                                                            span { class: "dk-res-chip dk-res-chip-more", "+{extra_spaces}" }
                                                        }
                                                    }
                                                }
                                            }
                                            div {
                                                class: "dk-res-tile",
                                                div { class: "dk-res-head",
                                                    span { class: "dk-res-icon dk-tone-ok", Icon { d: ICON_VAULT } }
                                                    span { class: "dk-eyebrow", "Bóvedas" }
                                                }
                                                div { class: "dk-res-value", "{vaults_total}" }
                                                div { class: "dk-muted", "en servidor" }
                                            }
                                        }

                                        div {
                                            class: "dk-res-footer",
                                            p { class: "dk-muted", "Tus espacios de la nube aparecen en la barra lateral para iniciar chats con su contexto." }
                                            button {
                                                class: "dk-btn dk-btn-secondary",
                                                disabled: is_loading,
                                                onclick: move |_| run_probe(Some("Barra lateral sincronizada con tus espacios de Kyō Web.")),
                                                Icon { d: ICON_SIDEBAR, class: if is_loading { "dk-ic animate-spin" } else { "dk-ic" } }
                                                "Sincronizar con barra lateral"
                                            }
                                        }
                                    }

                                    // ── Columna derecha: Seguridad y diagnóstico ──
                                    div {
                                        class: "dk-col-side",
                                        DiagnosticsCard { server_url: server_url_now.clone() }

                                        section {
                                            class: "dk-card dk-accordion",
                                            button {
                                                class: "dk-accordion-head",
                                                onclick: move |_| {
                                                    let cur = *show_credentials.read();
                                                    show_credentials.set(!cur);
                                                },
                                                span { class: "dk-card-head-icon dk-tone-accent", Icon { d: ICON_KEY } }
                                                span { class: "dk-accordion-title", "Configuración de Credenciales y Servidor (Token JWT)" }
                                                Icon { d: ICON_CHEVRON, class: if creds_open { "dk-ic-sm dk-chevron is-open" } else { "dk-ic-sm dk-chevron" } }
                                            }
                                            if creds_open {
                                                div {
                                                    class: "dk-accordion-body animate-fade-in",
                                                    CredentialFields {
                                                        token_input,
                                                        server_url_input,
                                                        last_error: last_error_msg,
                                                        on_submit: move |_| run_probe(Some("Token actualizado y guardado en session.json.")),
                                                    }
                                                    div {
                                                        class: "dk-row-actions",
                                                        button {
                                                            class: "dk-btn dk-btn-secondary",
                                                            disabled: is_loading,
                                                            onclick: move |_| run_probe(Some("Token actualizado y guardado en session.json.")),
                                                            Icon { d: ICON_CHECK }
                                                            "Guardar nuevo token"
                                                        }
                                                        button {
                                                            class: "dk-btn dk-btn-danger",
                                                            onclick: move |_| handle_disconnect(),
                                                            Icon { d: ICON_POWER }
                                                            "Desconectar"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Desconectado / comprobando / error: Token Key en primer plano.
                            let has_token = !token_input.read().trim().is_empty();
                            let step1 = if has_token { "dk-step is-done" } else { "dk-step is-current" };
                            let step2 = if has_token { "dk-step is-done" } else { "dk-step" };
                            let step3 = if has_token { "dk-step is-current" } else { "dk-step" };
                            rsx! {
                                div {
                                    class: "dk-bento",

                                    section {
                                        class: "dk-card dk-col-main",
                                        div {
                                            class: "dk-card-head",
                                            div { class: "dk-card-head-icon dk-tone-accent", Icon { d: ICON_KEY } }
                                            div {
                                                class: "dk-card-head-text",
                                                h3 { "Clave de Acceso (Token Key)" }
                                                p { class: "dk-muted", "Conecta este cliente de escritorio con tu cuenta de Kyō Web en tres pasos." }
                                            }
                                        }

                                        ol {
                                            class: "dk-steps",
                                            li {
                                                class: "{step1}",
                                                span { class: "dk-step-num", if has_token { Icon { d: ICON_CHECK, class: "dk-ic-sm" } } else { "1" } }
                                                div {
                                                    class: "dk-step-body",
                                                    span { class: "dk-step-title", "Obtener Token en la Web" }
                                                    span { class: "dk-muted", "Genera tu Token Key desde tu cuenta en kyo.broco.dev." }
                                                    button {
                                                        class: "dk-btn dk-btn-ghost dk-btn-xs",
                                                        onclick: move |_| open_in_browser(KYO_WEB_URL),
                                                        "Abrir kyo.broco.dev"
                                                        Icon { d: ICON_EXTERNAL, class: "dk-ic-sm" }
                                                    }
                                                }
                                            }
                                            li {
                                                class: "{step2}",
                                                span { class: "dk-step-num", if has_token { Icon { d: ICON_CHECK, class: "dk-ic-sm" } } else { "2" } }
                                                div {
                                                    class: "dk-step-body",
                                                    span { class: "dk-step-title", "Pegar Token" }
                                                    span { class: "dk-muted", "Pégalo en el campo de abajo. Se guarda solo en este equipo." }
                                                }
                                            }
                                            li {
                                                class: "{step3}",
                                                span { class: "dk-step-num", "3" }
                                                div {
                                                    class: "dk-step-body",
                                                    span { class: "dk-step-title", "Conectar y Sincronizar" }
                                                    span { class: "dk-muted", "Validamos el token y traemos tus espacios y bóvedas." }
                                                }
                                            }
                                        }

                                        CredentialFields {
                                            token_input,
                                            server_url_input,
                                            last_error: last_error_msg,
                                            on_submit: move |_| run_probe(Some("Credenciales verificadas y guardadas en session.json.")),
                                        }

                                        div {
                                            class: "dk-row-actions",
                                            button {
                                                class: "dk-btn dk-btn-primary dk-btn-lg",
                                                disabled: is_loading || !has_token,
                                                onclick: move |_| run_probe(Some("Credenciales verificadas y guardadas en session.json.")),
                                                Icon { d: if is_loading { ICON_REFRESH } else { ICON_CHECK }, class: if is_loading { "dk-ic animate-spin" } else { "dk-ic" } }
                                                if is_loading { "Verificando..." } else { "Conectar y Sincronizar" }
                                            }
                                            if has_token {
                                                button {
                                                    class: "dk-btn dk-btn-danger",
                                                    onclick: move |_| handle_disconnect(),
                                                    Icon { d: ICON_POWER }
                                                    "Olvidar token"
                                                }
                                            }
                                        }
                                    }

                                    div {
                                        class: "dk-col-side",
                                        section {
                                            class: "dk-card",
                                            div {
                                                class: "dk-card-head",
                                                div { class: "dk-card-head-icon dk-tone-ok", Icon { d: ICON_CLOUD } }
                                                div {
                                                    class: "dk-card-head-text",
                                                    h3 { "Al conectar podrás" }
                                                }
                                            }
                                            ul {
                                                class: "dk-benefits",
                                                li { Icon { d: ICON_FOLDER, class: "dk-ic-sm" } "Ver tus Espacios de Trabajo de la nube en la barra lateral." }
                                                li { Icon { d: ICON_VAULT, class: "dk-ic-sm" } "Abrir tus Bóvedas y usarlas como contexto de chat." }
                                                li { Icon { d: ICON_REFRESH, class: "dk-ic-sm" } "Sincronizar cambios con Kyō Web en un clic." }
                                            }
                                        }
                                        DiagnosticsCard { server_url: server_url_now.clone() }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Diagnóstico compacto del entorno local (endpoint, persistencia, seguridad).
#[component]
fn DiagnosticsCard(server_url: String) -> Element {
    let endpoint = format!("{}/api/v1", server_url.trim_end_matches('/'));
    rsx! {
        section {
            class: "dk-card",
            div {
                class: "dk-card-head",
                div { class: "dk-card-head-icon dk-tone-info", Icon { d: ICON_SHIELD } }
                div {
                    class: "dk-card-head-text",
                    h3 { "Diagnóstico del Entorno" }
                    p { class: "dk-muted", "Conexión local, almacenamiento y transporte" }
                }
            }
            dl {
                class: "dk-diag",
                div { class: "dk-diag-row",
                    dt { "Endpoint API" }
                    dd { class: "dk-mono", title: "{endpoint}", "{endpoint}" }
                }
                div { class: "dk-diag-row",
                    dt { "Persistencia Local" }
                    dd { class: "dk-mono", "%APPDATA%/kyo/session.json" }
                }
                div { class: "dk-diag-row",
                    dt { "Seguridad" }
                    dd { span { class: "dk-chip dk-chip-ok", Icon { d: ICON_LOCK, class: "dk-ic-xs" } "Bearer Token · TLS" } }
                }
            }
        }
    }
}

/// Campos de credenciales: Token Key (con mostrar/ocultar/limpiar) y URL del backend.
#[component]
fn CredentialFields(
    token_input: Signal<String>,
    server_url_input: Signal<String>,
    last_error: Signal<Option<String>>,
    on_submit: EventHandler<()>,
) -> Element {
    let mut token_input = token_input;
    let mut server_url_input = server_url_input;
    let mut last_error = last_error;
    let mut is_token_visible = use_signal(|| false);
    let mut show_server_config = use_signal(|| false);
    let visible = *is_token_visible.read();
    let server_open = *show_server_config.read();

    rsx! {
        div {
            class: "dk-field",
            label { class: "dk-label", r#for: "kyo-token-input", "Token Key (Bearer / JWT)" }
            div {
                class: "dk-input-wrap",
                span { class: "dk-input-icon", Icon { d: ICON_LOCK } }
                input {
                    id: "kyo-token-input",
                    r#type: if visible { "text" } else { "password" },
                    class: "dashboard-input dk-input",
                    placeholder: "Pega tu Token Key aquí (eyJhbGciOiJIUzI1Ni...)",
                    autocomplete: "off",
                    spellcheck: "false",
                    value: "{token_input}",
                    oninput: move |evt| {
                        token_input.set(evt.value());
                        last_error.set(None);
                    },
                    onkeydown: move |evt: KeyboardEvent| {
                        if evt.key() == Key::Enter {
                            on_submit.call(());
                        }
                    }
                }
                div {
                    class: "dk-input-tools",
                    if !token_input.read().is_empty() {
                        button {
                            class: "dk-icon-btn",
                            title: "Limpiar campo",
                            onclick: move |_| token_input.set(String::new()),
                            Icon { d: ICON_X }
                        }
                    }
                    button {
                        class: "dk-icon-btn",
                        title: if visible { "Ocultar token" } else { "Mostrar token" },
                        onclick: move |_| is_token_visible.set(!visible),
                        Icon { d: if visible { ICON_EYE_OFF } else { ICON_EYE } }
                    }
                }
            }
            if let Some(err) = last_error.read().as_ref() {
                p { class: "dk-field-error", "{err}" }
            }
        }

        div {
            class: "dk-field",
            button {
                class: "dk-link-btn",
                onclick: move |_| show_server_config.set(!server_open),
                Icon { d: ICON_CHEVRON, class: if server_open { "dk-ic-sm dk-chevron is-open" } else { "dk-ic-sm dk-chevron" } }
                "Servidor backend"
                span { class: "dk-mono dk-muted", "({server_url_input})" }
            }
            if server_open {
                div {
                    class: "dk-server-box",
                    div {
                        class: "flex items-center gap-2",
                        input {
                            class: "dashboard-input dk-input-plain",
                            value: "{server_url_input}",
                            oninput: move |evt| server_url_input.set(evt.value()),
                        }
                        if *server_url_input.read() != KYO_WEB_URL {
                            button {
                                class: "dk-btn dk-btn-secondary dk-btn-xs",
                                onclick: move |_| server_url_input.set(KYO_WEB_URL.to_string()),
                                "Restablecer"
                            }
                        }
                    }
                    p { class: "dk-muted", "Predeterminado: https://kyo.broco.dev. Cámbialo solo para una instancia local o de desarrollo." }
                }
            }
        }
    }
}
