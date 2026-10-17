//! Panel de control exclusivo para la conexión con la plataforma Web y Backend de Kyō.
//!
//! Este componente proporciona un único flujo centrado:
//! - Estado en tiempo real de la conexión con el backend de Kyō (https://kyo.broco.dev).
//! - Gestión, validación e inspección del Token Key (JWT).
//! - Diagnóstico de latencia RTT (ms), protocolo y cuenta vinculada.
//! - Persistencia directa en `%APPDATA%/kyo/session.json`.

use dioxus::prelude::*;
use crate::models::ActiveView;
use crate::settings::{
    analyze_token, clear_session_auth, load_auth_token, load_server_url,
    save_full_session_config,
};
use crate::sidecar::{SidecarConnectionStatus, SidecarState};

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");

/// Información de la cuenta y recursos detectados en el backend de Kyō.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedAccountInfo {
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub role: Option<String>,
    pub spaces_count: Option<usize>,
    pub vaults_count: Option<usize>,
    pub primary_vault: Option<String>,
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
fn open_in_browser(url: &str) {
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

    let spaces_url = if clean_url.ends_with("/api/v1") {
        format!("{clean_url}/spaces")
    } else {
        format!("{clean_url}/api/v1/spaces")
    };

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
    let spaces_count = spaces_json
        .get("total")
        .and_then(|t| t.as_u64())
        .map(|c| c as usize)
        .or_else(|| {
            spaces_json
                .get("data")
                .and_then(|d| d.as_array())
                .map(|a| a.len())
        });

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
    let vaults_url = if clean_url.ends_with("/api/v1") {
        format!("{clean_url}/vaults")
    } else {
        format!("{clean_url}/api/v1/vaults")
    };

    let mut vaults_count: Option<usize> = None;
    let mut primary_vault: Option<String> = None;

    if let Ok(v_res) = client
        .get(&vaults_url)
        .header("Authorization", format!("Bearer {trimmed_token}"))
        .send()
        .await
    {
        if v_res.status().is_success() {
            if let Ok(v_json) = v_res.json::<serde_json::Value>().await {
                vaults_count = v_json
                    .get("total")
                    .and_then(|t| t.as_u64())
                    .map(|c| c as usize)
                    .or_else(|| {
                        v_json
                            .get("data")
                            .and_then(|d| d.as_array())
                            .map(|a| a.len())
                    });

                if let Some(arr) = v_json.get("data").and_then(|d| d.as_array()) {
                    if let Some(first) = arr.first() {
                        primary_vault = first
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }
    }

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

/// Vista "Panel de control": flujo exclusivo para conectar y monitorear el backend de Kyō.
#[component]
pub fn Dashboard() -> Element {
    let sidecar_state = try_use_context::<Signal<SidecarState>>();
    let mut active_view = use_context::<Signal<ActiveView>>();

    // Estados del formulario y credenciales
    let mut token_input = use_signal(load_auth_token);
    let mut server_url_input = use_signal(load_server_url);
    let mut is_token_visible = use_signal(|| false);
    let mut is_verifying = use_signal(|| false);
    let mut show_server_config = use_signal(|| false);
    let save_feedback_visible = use_signal(|| false);
    let mut last_error_msg = use_signal(|| Option::<String>::None);

    // Estado principal de la conexión
    let mut connection_status = use_signal(|| ConnectionStatus::Disconnected);

    // Auto-verificación al cargar la vista si ya existe un token almacenado
    let mut has_run_initial_check = use_signal(|| false);
    use_effect(move || {
        if *has_run_initial_check.read() {
            return;
        }
        has_run_initial_check.set(true);

        let saved_token = token_input.read().clone();
        let saved_url = server_url_input.read().clone();

        if !saved_token.trim().is_empty() {
            is_verifying.set(true);
            connection_status.set(ConnectionStatus::Connecting);

            let mut conn_sig = connection_status;
            let mut verify_sig = is_verifying;
            let sc_opt = sidecar_state;

            dioxus::prelude::spawn(async move {
                match probe_kyo_backend(&saved_url, &saved_token).await {
                    Ok((lat, acc)) => {
                        let time_str = get_current_time_str();
                        conn_sig.set(ConnectionStatus::Connected {
                            latency_ms: lat,
                            server_url: saved_url.clone(),
                            last_checked: time_str,
                            account: Some(acc),
                        });
                        if let Some(mut sc) = sc_opt {
                            let mut w = sc.write();
                            w.latency_ms = Some(lat);
                            if w.status == SidecarConnectionStatus::Disconnected {
                                w.status = SidecarConnectionStatus::FallbackCloud;
                            }
                        }
                    }
                    Err((err, is_auth, code)) => {
                        conn_sig.set(ConnectionStatus::Error {
                            message: err,
                            is_auth_error: is_auth,
                            status_code: code,
                            server_url: saved_url,
                        });
                    }
                }
                verify_sig.set(false);
            });
        }
    });

    // Acción: Comprobar conexión y guardar token
    let mut handle_connect_and_save = move || {
        let raw_token = token_input.read().clone();
        let raw_url = server_url_input.read().clone();

        if raw_token.trim().is_empty() {
            last_error_msg.set(Some("Por favor ingrese un Token Key antes de conectar.".to_string()));
            connection_status.set(ConnectionStatus::Disconnected);
            return;
        }

        last_error_msg.set(None);
        is_verifying.set(true);
        connection_status.set(ConnectionStatus::Connecting);

        let mut conn_sig = connection_status;
        let mut verify_sig = is_verifying;
        let mut feedback_sig = save_feedback_visible;
        let mut err_sig = last_error_msg;
        let sc_opt = sidecar_state;

        dioxus::prelude::spawn(async move {
            match probe_kyo_backend(&raw_url, &raw_token).await {
                Ok((lat, acc)) => {
                    let time_str = get_current_time_str();
                    conn_sig.set(ConnectionStatus::Connected {
                        latency_ms: lat,
                        server_url: raw_url.clone(),
                        last_checked: time_str,
                        account: Some(acc),
                    });
                    if let Some(mut sc) = sc_opt {
                        let mut w = sc.write();
                        w.latency_ms = Some(lat);
                        if w.status == SidecarConnectionStatus::Disconnected {
                            w.status = SidecarConnectionStatus::FallbackCloud;
                        }
                    }
                    feedback_sig.set(true);
                    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                    feedback_sig.set(false);
                }
                Err((err, is_auth, code)) => {
                    err_sig.set(Some(err.clone()));
                    conn_sig.set(ConnectionStatus::Error {
                        message: err,
                        is_auth_error: is_auth,
                        status_code: code,
                        server_url: raw_url,
                    });
                    if let Some(mut sc) = sc_opt {
                        sc.write().status = SidecarConnectionStatus::Disconnected;
                    }
                }
            }
            verify_sig.set(false);
        });
    };

    // Acción: Desconectar y limpiar credenciales
    let mut handle_disconnect = move || {
        let _ = clear_session_auth();
        token_input.set(String::new());
        connection_status.set(ConnectionStatus::Disconnected);
        last_error_msg.set(None);

        if let Some(mut sc) = sidecar_state {
            let mut w = sc.write();
            w.status = SidecarConnectionStatus::Disconnected;
            w.latency_ms = None;
        }
    };

    let is_loading = *is_verifying.read();
    let is_connected = matches!(*connection_status.read(), ConnectionStatus::Connected { .. });

    rsx! {
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        div {
            class: "flex flex-col h-full w-full antialiased font-sans select-none overflow-hidden relative",
            style: "background-color: var(--kyo-surface-0); color: var(--kyo-fg-primary);",

            // ── Cabecera Superior del Panel ─────────────────────────────────
            header {
                class: "flex-none h-14 border-b px-8 flex items-center justify-between z-10",
                style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                div {
                    class: "flex items-center gap-3.5",
                    div {
                        class: "w-8 h-8 rounded-xl flex items-center justify-center bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 shadow-sm",
                        svg {
                            class: "w-4 h-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            rect { x: "3", y: "3", width: "7", height: "7", rx: "1" }
                            rect { x: "14", y: "3", width: "7", height: "7", rx: "1" }
                            rect { x: "14", y: "14", width: "7", height: "7", rx: "1" }
                            rect { x: "3", y: "14", width: "7", height: "7", rx: "1" }
                        }
                    }
                    div {
                        class: "flex items-center gap-2.5",
                        h1 {
                            class: "font-semibold text-sm tracking-tight",
                            style: "color: var(--kyo-fg-primary);",
                            "Panel de control"
                        }
                        span {
                            class: "text-[10px] font-mono px-2.5 py-0.5 rounded-full uppercase tracking-wider bg-zinc-800 text-zinc-400 border border-zinc-700/60",
                            "Conexión Kyō Web"
                        }
                    }
                }

                // Indicador de Estado y Botón de Comprobación Rápida
                div {
                    class: "flex items-center gap-3",
                    {
                        match *connection_status.read() {
                            ConnectionStatus::Connected { latency_ms, .. } => rsx! {
                                span {
                                    class: "px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 bg-emerald-500/10 text-emerald-400 border border-emerald-500/30",
                                    span { class: "w-2 h-2 rounded-full bg-emerald-400 animate-pulse shadow-sm" }
                                    "Conectado ({latency_ms} ms)"
                                }
                            },
                            ConnectionStatus::Connecting => rsx! {
                                span {
                                    class: "px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 bg-indigo-500/10 text-indigo-300 border border-indigo-500/30",
                                    span { class: "w-2 h-2 rounded-full bg-indigo-400 animate-ping" }
                                    "Comprobando..."
                                }
                            },
                            ConnectionStatus::Error { is_auth_error, .. } => rsx! {
                                span {
                                    class: if is_auth_error {
                                        "px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 bg-amber-500/10 text-amber-400 border border-amber-500/30"
                                    } else {
                                        "px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 bg-rose-500/10 text-rose-400 border border-rose-500/30"
                                    },
                                    span { class: "w-2 h-2 rounded-full bg-current" }
                                    if is_auth_error { "Token Inválido" } else { "Servidor Inaccesible" }
                                }
                            },
                            ConnectionStatus::Disconnected => rsx! {
                                span {
                                    class: "px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 bg-zinc-800 text-zinc-400 border border-zinc-700/60",
                                    span { class: "w-2 h-2 rounded-full bg-zinc-500" }
                                    "Desconectado"
                                }
                            },
                        }
                    }

                    // Botón para refrescar / re-probar
                    button {
                        class: "flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium transition-all cursor-pointer bg-zinc-800/80 hover:bg-zinc-700 border border-zinc-700/60 text-zinc-200 disabled:opacity-50",
                        disabled: is_loading,
                        onclick: move |_| handle_connect_and_save(),
                        svg {
                            class: if is_loading { "w-3.5 h-3.5 animate-spin" } else { "w-3.5 h-3.5" },
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" }
                        }
                        span { if is_loading { "Verificando..." } else { "Comprobar estado" } }
                    }
                }
            }

            // ── Contenido Principal Scrollable con Canvas Espacioso ─────────
            main {
                class: "flex-1 overflow-y-auto w-full",

                div {
                    class: "dashboard-canvas flex flex-col gap-7",

                    // 1. HERO TELEMÉTRICO DE ESTADO EN TIEMPO REAL
                    {
                        match &*connection_status.read() {
                            ConnectionStatus::Connected { latency_ms, server_url, last_checked, account } => {
                                let spaces_txt = account.as_ref().and_then(|a| a.spaces_count).map(|c| format!("{c} espacios")).unwrap_or_else(|| "--".to_string());
                                let vaults_txt = account.as_ref().and_then(|a| a.vaults_count).map(|c| format!("{c} bóvedas")).unwrap_or_else(|| "--".to_string());
                                let latency_color = if *latency_ms < 150 { "text-emerald-400" } else if *latency_ms < 400 { "text-amber-400" } else { "text-rose-400" };

                                rsx! {
                                    div {
                                        class: "dashboard-hero dashboard-hero-connected p-6 rounded-2xl border transition-all relative overflow-hidden shadow-md",
                                        div {
                                            class: "flex flex-col md:flex-row md:items-center justify-between gap-6 relative z-10",
                                            div {
                                                class: "flex items-start gap-4",
                                                div {
                                                    class: "w-12 h-12 rounded-2xl flex items-center justify-center bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 flex-none shadow-sm",
                                                    svg {
                                                        class: "w-6 h-6",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2.5",
                                                        path { d: "M20 6L9 17l-5-5" }
                                                    }
                                                }
                                                div {
                                                    class: "space-y-1",
                                                    div {
                                                        class: "flex items-center gap-2.5 flex-wrap",
                                                        h2 {
                                                            class: "text-base font-bold tracking-tight text-zinc-100",
                                                            style: "color: var(--kyo-fg-primary);",
                                                            "Conexión activa con el Backend de Kyō"
                                                        }
                                                        span { class: "px-2.5 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40", "ONLINE 200 OK" }
                                                    }
                                                    p {
                                                        class: "text-xs text-zinc-400 leading-relaxed",
                                                        "Tu cliente está autenticado y sincronizado con el backend oficial de Kyō ({server_url})."
                                                    }
                                                }
                                            }

                                            // Métricas telemétricas en tiempo real
                                            div {
                                                class: "flex items-center gap-3 pt-3 md:pt-0 border-t md:border-t-0 border-zinc-800 flex-wrap sm:flex-nowrap",
                                                div {
                                                    class: "px-4 py-2.5 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[96px] flex-1 sm:flex-none shadow-sm",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400 tracking-wider", "Latencia RTT" }
                                                    div { class: "text-sm font-bold font-mono {latency_color} mt-0.5", "{latency_ms} ms" }
                                                }
                                                div {
                                                    class: "px-4 py-2.5 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[96px] flex-1 sm:flex-none shadow-sm",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400 tracking-wider", "Espacios" }
                                                    div { class: "text-xs font-semibold text-zinc-200 mt-1", "{spaces_txt}" }
                                                }
                                                div {
                                                    class: "px-4 py-2.5 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[96px] flex-1 sm:flex-none shadow-sm",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400 tracking-wider", "Bóvedas" }
                                                    div { class: "text-xs font-semibold text-zinc-200 mt-1 truncate max-w-[100px]", "{vaults_txt}" }
                                                }
                                            }
                                        }
                                        div {
                                            class: "mt-4 pt-3.5 border-t border-zinc-800/80 flex flex-wrap items-center justify-between gap-2 text-[11px] text-zinc-400 font-mono",
                                            span { class: "flex items-center gap-1.5",
                                                svg { class: "w-3 h-3 text-emerald-400", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", circle { cx: "12", cy: "12", r: "10" } polyline { points: "12 6 12 12 16 14" } }
                                                "Última comprobación: {last_checked}"
                                            }
                                            span { class: "flex items-center gap-1.5",
                                                svg { class: "w-3 h-3 text-indigo-400", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", rect { x: "3", y: "11", width: "18", height: "11", rx: "2" } path { d: "M7 11V7a5 5 0 0 1 10 0v4" } }
                                                "Protocolo: HTTPS / REST API v1"
                                            }
                                        }
                                    }
                                }
                            },
                            ConnectionStatus::Connecting => rsx! {
                                div {
                                    class: "dashboard-hero dashboard-hero-connecting p-6 rounded-2xl border transition-all flex items-center gap-4 shadow-sm",
                                    div {
                                        class: "w-12 h-12 rounded-2xl flex items-center justify-center bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 flex-none",
                                        svg {
                                            class: "w-6 h-6 animate-spin",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" }
                                        }
                                    }
                                    div {
                                        class: "space-y-1",
                                        h2 {
                                            class: "text-base font-bold text-zinc-100",
                                            style: "color: var(--kyo-fg-primary);",
                                            "Comprobando conexión con el backend de Kyō..."
                                        }
                                        p { class: "text-xs text-zinc-400 leading-relaxed", "Verificando el Token Key y midiendo la latencia de respuesta de los servicios." }
                                    }
                                }
                            },
                            ConnectionStatus::Error { message, is_auth_error, status_code, server_url } => rsx! {
                                div {
                                    class: if *is_auth_error {
                                        "dashboard-hero dashboard-hero-warning p-6 rounded-2xl border space-y-3 shadow-sm"
                                    } else {
                                        "dashboard-hero dashboard-hero-error p-6 rounded-2xl border space-y-3 shadow-sm"
                                    },
                                    div {
                                        class: "flex items-start gap-4",
                                        div {
                                            class: if *is_auth_error {
                                                "w-12 h-12 rounded-2xl flex items-center justify-center bg-amber-500/10 text-amber-400 border border-amber-500/30 flex-none"
                                            } else {
                                                "w-12 h-12 rounded-2xl flex items-center justify-center bg-rose-500/10 text-rose-400 border border-rose-500/30 flex-none"
                                            },
                                            svg {
                                                class: "w-6 h-6",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                circle { cx: "12", cy: "12", r: "10" }
                                                line { x1: "12", y1: "8", x2: "12", y2: "12" }
                                                line { x1: "12", y1: "16", x2: "12.01", y2: "16" }
                                            }
                                        }
                                        div {
                                            class: "space-y-1.5 flex-1",
                                            div {
                                                class: "flex items-center gap-2.5 flex-wrap",
                                                h2 {
                                                    class: "text-base font-bold text-zinc-100",
                                                    style: "color: var(--kyo-fg-primary);",
                                                    if *is_auth_error { "Error de Autenticación con Kyō" } else { "Error de Conectividad con el Backend" }
                                                }
                                                if let Some(code) = status_code {
                                                    span { class: "px-2 py-0.5 rounded text-[10px] font-mono bg-zinc-800 text-zinc-300 border border-zinc-700", "HTTP {code}" }
                                                }
                                            }
                                            p { class: "text-xs text-zinc-300 leading-relaxed", "{message}" }
                                            p {
                                                class: "text-[11px] text-zinc-400 font-mono mt-1",
                                                "Servidor consultado: {server_url}. Asegúrate de ingresar un Token Key válido generado desde la web de Kyō."
                                            }
                                        }
                                    }
                                }
                            },
                            ConnectionStatus::Disconnected => rsx! {
                                div {
                                    class: "dashboard-hero dashboard-hero-disconnected p-6 rounded-2xl border relative overflow-hidden shadow-sm",
                                    div {
                                        class: "flex flex-col sm:flex-row sm:items-center justify-between gap-5",
                                        div {
                                            class: "flex items-start gap-4",
                                            div {
                                                class: "w-12 h-12 rounded-2xl flex items-center justify-center bg-zinc-800 text-zinc-400 border border-zinc-700/80 flex-none",
                                                svg {
                                                    class: "w-6 h-6",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    path { d: "M1 1l22 22M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.58 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01" }
                                                }
                                            }
                                            div {
                                                class: "space-y-1",
                                                div {
                                                    class: "flex items-center gap-2",
                                                    h2 {
                                                        class: "text-base font-bold text-zinc-100",
                                                        style: "color: var(--kyo-fg-primary);",
                                                        "Desconectado de Kyō Web"
                                                    }
                                                    span { class: "px-2 py-0.5 rounded text-[10px] font-mono bg-zinc-800 text-zinc-400 border border-zinc-700", "OFFLINE" }
                                                }
                                                p { class: "text-xs text-zinc-400 leading-relaxed", "Introduce tu Token Key a continuación para conectar tu software de escritorio con tu cuenta y habilitar la sincronización en vivo." }
                                            }
                                        }
                                        button {
                                            class: "flex-none px-4 py-2.5 rounded-xl text-xs font-semibold text-indigo-400 hover:text-indigo-300 bg-indigo-600/10 hover:bg-indigo-600/20 border border-indigo-500/30 flex items-center gap-2 transition-all cursor-pointer",
                                            onclick: move |_| open_in_browser("https://kyo.broco.dev"),
                                            span { "Obtener Token en la Web" }
                                            svg { class: "w-3.5 h-3.5", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", path { d: "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3" } }
                                        }
                                    }
                                }
                            },
                        }
                    }

                    // 2. GRID PRINCIPAL DE GESTIÓN Y TELEMETRÍA (2 Columnas Equilibradas)
                    div {
                        class: "grid grid-cols-1 lg:grid-cols-2 gap-7 items-stretch",

                        // Columna Izquierda: Tarjeta 1 - Autenticación por Token Key
                        div {
                            class: "dashboard-card p-6 flex flex-col justify-between gap-5",

                            div {
                                class: "space-y-5",

                                // Header de la Tarjeta
                                div {
                                    class: "space-y-1 pb-4 border-b border-zinc-800",
                                    h3 {
                                        class: "text-sm font-bold text-zinc-100 flex items-center gap-2.5",
                                        style: "color: var(--kyo-fg-primary);",
                                        div {
                                            class: "w-7 h-7 rounded-lg flex items-center justify-center bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 flex-none",
                                            svg {
                                                class: "w-4 h-4",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                path { d: "M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" }
                                            }
                                        }
                                        "Clave de Acceso (Token Key)"
                                    }
                                    p { class: "text-xs text-zinc-400 leading-relaxed pl-9.5",
                                        "Ingresa el token generado en tu cuenta web de Kyō para conectar este cliente de escritorio con el backend."
                                    }
                                }

                                // Campo de entrada del Token con Icono de Candado, Toggle Mostrar/Ocultar y Botón Limpiar
                                div {
                                    class: "space-y-2",
                                    label {
                                        class: "block text-xs font-semibold text-zinc-300",
                                        "Token Key (Bearer / JWT):"
                                    }
                                    div {
                                        class: "relative flex items-center w-full",

                                        // Icono de candado a la izquierda para alejar el texto del borde y centrarlo visualmente
                                        div {
                                            class: "absolute left-4 flex items-center pointer-events-none text-zinc-400",
                                            svg {
                                                class: "w-4 h-4",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                rect { x: "3", y: "11", width: "18", height: "11", rx: "2", ry: "2" }
                                                path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                                            }
                                        }

                                        input {
                                            r#type: if *is_token_visible.read() { "text" } else { "password" },
                                            class: "w-full py-3.5 rounded-xl font-mono text-xs focus:outline-none transition-all shadow-sm",
                                            style: "background-color: var(--kyo-surface-0); border: 1px solid var(--kyo-border-default); color: var(--kyo-fg-primary); padding-left: 44px; padding-right: 84px;",
                                            placeholder: "Pega tu Token Key aquí (eyJhbGciOiJIUzI1Ni...)",
                                            value: "{token_input}",
                                            oninput: move |evt| {
                                                token_input.set(evt.value());
                                                last_error_msg.set(None);
                                            },
                                            onkeydown: move |evt: KeyboardEvent| {
                                                if evt.key() == Key::Enter {
                                                    handle_connect_and_save();
                                                }
                                            }
                                        }

                                        // Botones internos (Limpiar y Ojo Mostrar/Ocultar)
                                        div {
                                            class: "absolute right-3.5 flex items-center gap-1.5",
                                            if !token_input.read().is_empty() {
                                                button {
                                                    class: "p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer",
                                                    title: "Limpiar campo",
                                                    onclick: move |_| token_input.set(String::new()),
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
                                            button {
                                                class: "p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer",
                                                title: if *is_token_visible.read() { "Ocultar token" } else { "Mostrar token" },
                                                onclick: move |_| {
                                                    let cur = *is_token_visible.read();
                                                    is_token_visible.set(!cur);
                                                },
                                                if *is_token_visible.read() {
                                                    svg {
                                                        class: "w-4 h-4",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        path { d: "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" }
                                                        line { x1: "1", y1: "1", x2: "23", y2: "23" }
                                                    }
                                                } else {
                                                    svg {
                                                        class: "w-4 h-4",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        path { d: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" }
                                                        circle { cx: "12", cy: "12", r: "3" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Botones de Acción (Espaciosos, Modernos y Cómodos)
                                div {
                                    class: "flex flex-col sm:flex-row sm:items-center justify-between gap-3.5 pt-3 border-t border-zinc-800/60",
                                    div {
                                        class: "flex items-center gap-3 flex-wrap",
                                        button {
                                            class: "px-6 py-3 rounded-xl text-xs sm:text-sm font-semibold bg-indigo-600 hover:bg-indigo-500 active:bg-indigo-700 text-white shadow-md shadow-indigo-600/30 flex items-center justify-center gap-2.5 transition-all cursor-pointer disabled:opacity-50",
                                            disabled: is_loading,
                                            onclick: move |_| handle_connect_and_save(),
                                            svg {
                                                class: if is_loading { "w-4 h-4 animate-spin" } else { "w-4 h-4" },
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2.5",
                                                stroke_linecap: "round",
                                                stroke_linejoin: "round",
                                                path { d: "M20 6L9 17l-5-5" }
                                            }
                                            span { if is_loading { "Verificando..." } else { "Conectar y Guardar" } }
                                        }

                                        if is_connected || !token_input.read().trim().is_empty() {
                                            button {
                                                class: "px-5 py-3 rounded-xl text-xs sm:text-sm font-medium text-rose-400 hover:text-white hover:bg-rose-600 border border-rose-500/30 flex items-center justify-center gap-2 transition-all cursor-pointer",
                                                onclick: move |_| handle_disconnect(),
                                                svg {
                                                    class: "w-4 h-4",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    path { d: "M18.36 6.64a9 9 0 1 1-12.73 0M12 2v10" }
                                                }
                                                span { "Desconectar" }
                                            }
                                        }
                                    }

                                    // Botón para abrir la web de Kyō
                                    button {
                                        class: "px-4 py-2.5 rounded-xl text-xs font-medium text-indigo-400 hover:text-indigo-300 bg-indigo-500/10 hover:bg-indigo-500/20 border border-indigo-500/25 flex items-center justify-center gap-2 transition-all cursor-pointer self-start sm:self-auto",
                                        onclick: move |_| open_in_browser("https://kyo.broco.dev"),
                                        span { "Obtener Token en la Web" }
                                        svg {
                                            class: "w-3.5 h-3.5",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6M15 3h6v6M10 14L21 3" }
                                        }
                                    }
                                }

                                // Mensaje de éxito
                                if *save_feedback_visible.read() {
                                    div {
                                        class: "p-3.5 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 text-xs flex items-center gap-2.5 animate-fade-in",
                                        svg {
                                            class: "w-4 h-4 flex-none",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M20 6L9 17l-5-5" }
                                        }
                                        span { "✓ Credenciales verificadas y guardadas correctamente en session.json." }
                                    }
                                }
                            }

                            // Configuración avanzada del servidor backend (Acordeón)
                            div {
                                class: "pt-3 border-t border-zinc-800/80 space-y-3",
                                button {
                                    class: "flex items-center gap-2 text-xs font-medium text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer",
                                    onclick: move |_| {
                                        let c = *show_server_config.read();
                                        show_server_config.set(!c);
                                    },
                                    svg {
                                        class: if *show_server_config.read() { "w-3.5 h-3.5 rotate-90 transition-transform" } else { "w-3.5 h-3.5 transition-transform" },
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        path { d: "M9 18l6-6-6-6" }
                                    }
                                    span { "Configuración avanzada del servidor backend" }
                                    span { class: "text-[10px] font-mono text-zinc-400", "({server_url_input})" }
                                }

                                if *show_server_config.read() {
                                    div {
                                        class: "p-4 rounded-xl bg-zinc-900/60 border border-zinc-800 space-y-3",
                                        label { class: "block text-[11px] font-medium text-zinc-400", "URL del Backend Remoto:" }
                                        div {
                                            class: "flex items-center gap-2",
                                            input {
                                                class: "flex-1 px-3 py-2 rounded-lg font-mono text-xs focus:outline-none focus:ring-1 focus:ring-indigo-500",
                                                style: "background-color: var(--kyo-surface-0); border: 1px solid var(--kyo-border-default); color: var(--kyo-fg-primary);",
                                                value: "{server_url_input}",
                                                oninput: move |evt| server_url_input.set(evt.value()),
                                            }
                                            if *server_url_input.read() != "https://kyo.broco.dev" {
                                                button {
                                                    class: "px-3 py-2 rounded-lg text-xs font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-300 border border-zinc-700 cursor-pointer",
                                                    onclick: move |_| server_url_input.set("https://kyo.broco.dev".to_string()),
                                                    "Restablecer"
                                                }
                                            }
                                        }
                                        p { class: "text-[11px] text-zinc-400 leading-relaxed", "El valor predeterminado es https://kyo.broco.dev. Solo cámbialo si estás ejecutando una instancia local o un entorno de desarrollo dedicado." }
                                    }
                                }
                            }
                        }

                        // Columna Derecha: Tarjeta 2 - Cuenta de Usuario en Kyō
                        div {
                            class: "dashboard-card p-6 flex flex-col justify-between gap-4",

                            div {
                                class: "space-y-4",

                                div {
                                    class: "flex items-center gap-3 pb-3.5 border-b border-zinc-800",
                                    div {
                                        class: "w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-500 to-violet-600 flex items-center justify-center text-white font-bold text-sm shadow-sm",
                                        svg {
                                            class: "w-5 h-5",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" }
                                            circle { cx: "12", cy: "7", r: "4" }
                                        }
                                    }
                                    div {
                                        h3 {
                                            class: "text-sm font-bold text-zinc-100",
                                            style: "color: var(--kyo-fg-primary);",
                                            "Cuenta en Kyō"
                                        }
                                        p { class: "text-xs text-zinc-400", "Identidad y recursos vinculados al token" }
                                    }
                                }

                                // Datos de la cuenta
                                {
                                    if let ConnectionStatus::Connected { account: Some(acc), .. } = &*connection_status.read() {
                                        let name_str = acc.username.clone().unwrap_or_else(|| "Usuario Autenticado".to_string());
                                        let uid_str = acc.user_id.map(|id| format!("#{id}")).unwrap_or_else(|| "--".to_string());
                                        let role_str = acc.role.clone().unwrap_or_else(|| "user".to_string());
                                        let sp_count = acc.spaces_count.unwrap_or(0);
                                        let vl_count = acc.vaults_count.unwrap_or(0);
                                        let primary_v = acc.primary_vault.clone().unwrap_or_else(|| "Mi bóveda".to_string());

                                        rsx! {
                                            div {
                                                class: "space-y-4",

                                                // Tarjeta resumen del usuario
                                                div {
                                                    class: "p-4 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-between",
                                                    div {
                                                        class: "space-y-0.5",
                                                        div {
                                                            class: "text-sm font-bold text-zinc-100",
                                                            style: "color: var(--kyo-fg-primary);",
                                                            "{name_str}"
                                                        }
                                                        div { class: "text-[11px] font-mono text-zinc-400", "ID: {uid_str}" }
                                                    }
                                                    span {
                                                        class: "px-2.5 py-0.5 rounded-full text-[10px] font-mono uppercase bg-indigo-500/20 text-indigo-300 border border-indigo-500/30",
                                                        "{role_str}"
                                                    }
                                                }

                                                // Mini-grid de recursos vinculados
                                                div {
                                                    class: "grid grid-cols-2 gap-3",
                                                    div {
                                                        class: "p-3.5 rounded-xl bg-zinc-900/60 border border-zinc-800 text-center",
                                                        div { class: "text-[10px] font-mono uppercase text-zinc-400 tracking-wider", "Espacios" }
                                                        div {
                                                            class: "text-sm font-bold text-zinc-100 mt-1",
                                                            style: "color: var(--kyo-fg-primary);",
                                                            "{sp_count}"
                                                        }
                                                        div { class: "text-[10px] text-zinc-400 mt-0.5", "en la nube" }
                                                    }
                                                    div {
                                                        class: "p-3.5 rounded-xl bg-zinc-900/60 border border-zinc-800 text-center",
                                                        div { class: "text-[10px] font-mono uppercase text-zinc-400 tracking-wider", "Bóvedas" }
                                                        div {
                                                            class: "text-sm font-bold text-zinc-100 mt-1",
                                                            style: "color: var(--kyo-fg-primary);",
                                                            "{vl_count}"
                                                        }
                                                        div { class: "text-[10px] text-zinc-400 mt-0.5", "en servidor" }
                                                    }
                                                }

                                                // Filas de detalle
                                                div {
                                                    class: "space-y-2 text-xs",
                                                    div {
                                                        class: "flex items-center justify-between p-3 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                        span { class: "text-zinc-400", "Bóveda Principal:" }
                                                        span { class: "font-semibold text-emerald-400", "{primary_v}" }
                                                    }
                                                    div {
                                                        class: "flex items-center justify-between p-3 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                        span { class: "text-zinc-400", "Sincronización:" }
                                                        span { class: "flex items-center gap-1.5 text-zinc-300",
                                                            span { class: "w-2 h-2 rounded-full bg-emerald-400 animate-pulse" }
                                                            "En tiempo real"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        rsx! {
                                            div {
                                                class: "p-5 rounded-xl bg-zinc-900/40 border border-zinc-800/80 text-center space-y-2 text-xs text-zinc-400 my-auto",
                                                div {
                                                    class: "w-10 h-10 rounded-full bg-zinc-800/80 text-zinc-400 mx-auto flex items-center justify-center",
                                                    svg { class: "w-5 h-5", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.5", path { d: "M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" } circle { cx: "8.5", cy: "7", r: "4" } line { x1: "20", y1: "8", x2: "20", y2: "14" } line { x1: "23", y1: "11", x2: "17", y2: "11" } }
                                                }
                                                p { class: "font-medium text-zinc-300", "Sin cuenta vinculada" }
                                                p { class: "text-[11px] text-zinc-400 leading-relaxed", "Ingresa tu Token Key para ver los detalles de tu usuario y sincronizar tus espacios." }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 3. BARRA HORIZONTAL: Diagnóstico del Entorno & Acceso al Chat (Full width)
                    div {
                        class: "dashboard-card p-6 flex flex-col gap-5",

                        div {
                            class: "flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-zinc-800",
                            div {
                                class: "flex items-center gap-3",
                                div {
                                    class: "w-8 h-8 rounded-xl flex items-center justify-center bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 flex-none",
                                    svg {
                                        class: "w-4 h-4",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        path { d: "M5 12h14M12 5l7 7-7 7" }
                                    }
                                }
                                div {
                                    h3 {
                                        class: "text-sm font-bold text-zinc-100",
                                        style: "color: var(--kyo-fg-primary);",
                                        "Diagnóstico del Entorno"
                                    }
                                    p { class: "text-xs text-zinc-400", "Parámetros de conexión local, almacenamiento persistente y seguridad de transporte" }
                                }
                            }
                            button {
                                class: "px-4.5 py-2.5 rounded-xl text-xs font-semibold bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 flex items-center gap-2 transition-all cursor-pointer shadow-sm self-start sm:self-auto",
                                onclick: move |_| active_view.set(ActiveView::Chat),
                                span { "Ir al Chat de Kyō" }
                                svg {
                                    class: "w-3.5 h-3.5",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    path { d: "M5 12h14M12 5l7 7-7 7" }
                                }
                            }
                        }

                        div {
                            class: "grid grid-cols-1 md:grid-cols-3 gap-3.5 font-mono text-[11px]",
                            div {
                                class: "p-3.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex flex-col gap-1.5",
                                span { class: "text-[10px] uppercase font-sans text-zinc-400 tracking-wider", "Endpoint API" }
                                span { class: "text-xs text-zinc-200 truncate font-mono", "{server_url_input}/api/v1" }
                            }
                            div {
                                class: "p-3.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex flex-col gap-1.5",
                                span { class: "text-[10px] uppercase font-sans text-zinc-400 tracking-wider", "Persistencia Local" }
                                span { class: "text-xs text-emerald-400 font-medium font-sans", "%APPDATA%/kyo/session.json" }
                            }
                            div {
                                class: "p-3.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex flex-col gap-1.5",
                                span { class: "text-[10px] uppercase font-sans text-zinc-400 tracking-wider", "Seguridad de Transporte" }
                                span { class: "text-xs text-indigo-300 font-sans", "Bearer Token TLS Encrypted" }
                            }
                        }
                    }
                }
            }
        }
    }
}

