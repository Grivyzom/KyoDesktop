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

    // Análisis estático del JWT en tiempo real
    let jwt_info = use_memo(move || {
        let t = token_input.read();
        analyze_token(&t)
    });

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
                class: "flex-none h-14 border-b px-6 flex items-center justify-between z-10",
                style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                div {
                    class: "flex items-center gap-3",
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
                        class: "flex items-center gap-2",
                        h1 {
                            class: "font-semibold text-sm tracking-tight",
                            style: "color: var(--kyo-fg-primary);",
                            "Panel de control"
                        }
                        span {
                            class: "text-[10px] font-mono px-2 py-0.5 rounded-full uppercase tracking-wider bg-zinc-800 text-zinc-400 border border-zinc-700/60",
                            "Conexión Kyō Web"
                        }
                    }
                }

                // Indicador de Estado y Botón de Comprobación Rápida
                div {
                    class: "flex items-center gap-2.5",
                    {
                        match *connection_status.read() {
                            ConnectionStatus::Connected { latency_ms, .. } => rsx! {
                                span {
                                    class: "px-2.5 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 bg-emerald-500/10 text-emerald-400 border border-emerald-500/30",
                                    span { class: "w-2 h-2 rounded-full bg-emerald-400 animate-pulse shadow-sm shadow-emerald-500/50" }
                                    "Conectado ({latency_ms} ms)"
                                }
                            },
                            ConnectionStatus::Connecting => rsx! {
                                span {
                                    class: "px-2.5 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 bg-indigo-500/10 text-indigo-300 border border-indigo-500/30",
                                    span { class: "w-2 h-2 rounded-full bg-indigo-400 animate-ping" }
                                    "Comprobando..."
                                }
                            },
                            ConnectionStatus::Error { is_auth_error, .. } => rsx! {
                                span {
                                    class: if is_auth_error {
                                        "px-2.5 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 bg-amber-500/10 text-amber-400 border border-amber-500/30"
                                    } else {
                                        "px-2.5 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 bg-rose-500/10 text-rose-400 border border-rose-500/30"
                                    },
                                    span { class: "w-2 h-2 rounded-full bg-current" }
                                    if is_auth_error { "Token Inválido" } else { "Servidor Inaccesible" }
                                }
                            },
                            ConnectionStatus::Disconnected => rsx! {
                                span {
                                    class: "px-2.5 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 bg-zinc-800 text-zinc-400 border border-zinc-700/60",
                                    span { class: "w-2 h-2 rounded-full bg-zinc-500" }
                                    "Desconectado"
                                }
                            },
                        }
                    }

                    // Botón para refrescar / re-probar
                    button {
                        class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all cursor-pointer bg-zinc-800/80 hover:bg-zinc-700 border border-zinc-700/60 text-zinc-200 disabled:opacity-50",
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
                        span { if is_loading { "Verificando..." } else { "Comprobar ahora" } }
                    }
                }
            }

            // ── Contenido Principal Scrollable ──────────────────────────────
            main {
                class: "flex-1 overflow-y-auto px-6 py-8 space-y-6 max-w-5xl mx-auto w-full",

                // 1. BANNER PRINCIPAL DE ESTADO EN TIEMPO REAL
                {
                    match &*connection_status.read() {
                        ConnectionStatus::Connected { latency_ms, server_url, last_checked, account } => {
                            let spaces_txt = account.as_ref().and_then(|a| a.spaces_count).map(|c| format!("{c} espacios")).unwrap_or_else(|| "--".to_string());
                            let vaults_txt = account.as_ref().and_then(|a| a.vaults_count).map(|c| format!("{c} bóvedas")).unwrap_or_else(|| "--".to_string());

                            rsx! {
                                div {
                                    class: "p-6 rounded-2xl border transition-all duration-300 relative overflow-hidden bg-gradient-to-r from-emerald-950/20 via-zinc-900 to-zinc-900/90 border-emerald-500/30 shadow-lg shadow-emerald-500/5",
                                    div {
                                        class: "flex flex-col md:flex-row md:items-center justify-between gap-5 relative z-10",
                                        div {
                                            class: "flex items-start gap-4",
                                            div {
                                                class: "w-12 h-12 rounded-2xl flex items-center justify-center bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 flex-none",
                                                svg {
                                                    class: "w-6 h-6",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    path { d: "M20 6L9 17l-5-5" }
                                                }
                                            }
                                            div {
                                                class: "space-y-1",
                                                div {
                                                    class: "flex items-center gap-2",
                                                    h2 { class: "text-lg font-bold tracking-tight text-white", "Conexión activa con el Backend de Kyō" }
                                                    span { class: "px-2 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40", "ONLINE 200 OK" }
                                                }
                                                p {
                                                    class: "text-xs text-zinc-400 leading-relaxed",
                                                    "Tu software está autenticado y sincronizado con el backend oficial de Kyō ({server_url})."
                                                }
                                            }
                                        }

                                        // Métricas rápidas
                                        div {
                                            class: "flex items-center gap-3 pt-2 md:pt-0 border-t md:border-t-0 border-zinc-800",
                                            div {
                                                class: "px-3.5 py-2 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[90px]",
                                                div { class: "text-[10px] font-mono uppercase text-zinc-400", "Latencia RTT" }
                                                div { class: "text-sm font-bold font-mono text-emerald-400", "{latency_ms} ms" }
                                            }
                                            div {
                                                class: "px-3.5 py-2 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[90px]",
                                                div { class: "text-[10px] font-mono uppercase text-zinc-400", "Espacios" }
                                                div { class: "text-xs font-semibold text-zinc-200 mt-0.5", "{spaces_txt}" }
                                            }
                                            div {
                                                class: "px-3.5 py-2 rounded-xl bg-zinc-900/80 border border-zinc-800 text-center min-w-[90px]",
                                                div { class: "text-[10px] font-mono uppercase text-zinc-400", "Bóvedas" }
                                                div { class: "text-xs font-semibold text-zinc-200 mt-0.5 truncate max-w-[100px]", "{vaults_txt}" }
                                            }
                                        }
                                    }
                                    div {
                                        class: "mt-4 pt-3 border-t border-zinc-800/80 flex items-center justify-between text-[11px] text-zinc-400 font-mono",
                                        span { "Última sincronización comprobada: {last_checked}" }
                                        span { "Protocolo: HTTPS / REST API v1" }
                                    }
                                }
                            }
                        },
                        ConnectionStatus::Connecting => rsx! {
                            div {
                                class: "p-6 rounded-2xl border transition-all duration-300 bg-indigo-950/20 border-indigo-500/30 flex items-center gap-4",
                                div {
                                    class: "w-10 h-10 rounded-xl flex items-center justify-center bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 flex-none",
                                    svg {
                                        class: "w-5 h-5 animate-spin",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        path { d: "M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" }
                                    }
                                }
                                div {
                                    h2 { class: "text-sm font-bold text-white", "Comprobando conexión con el backend de Kyō..." }
                                    p { class: "text-xs text-zinc-400 mt-0.5", "Verificando el Token Key y midiendo la latencia de respuesta de los servicios." }
                                }
                            }
                        },
                        ConnectionStatus::Error { message, is_auth_error, status_code, server_url } => rsx! {
                            div {
                                class: if *is_auth_error {
                                    "p-6 rounded-2xl border bg-amber-950/20 border-amber-500/30 space-y-3"
                                } else {
                                    "p-6 rounded-2xl border bg-rose-950/20 border-rose-500/30 space-y-3"
                                },
                                div {
                                    class: "flex items-start gap-4",
                                    div {
                                        class: if *is_auth_error {
                                            "w-10 h-10 rounded-xl flex items-center justify-center bg-amber-500/10 text-amber-400 border border-amber-500/30 flex-none"
                                        } else {
                                            "w-10 h-10 rounded-xl flex items-center justify-center bg-rose-500/10 text-rose-400 border border-rose-500/30 flex-none"
                                        },
                                        svg {
                                            class: "w-5 h-5",
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
                                        class: "space-y-1",
                                        div {
                                            class: "flex items-center gap-2",
                                            h2 {
                                                class: "text-sm font-bold text-white",
                                                if *is_auth_error { "Error de Autenticación con Kyō" } else { "Error de Conectividad con el Backend" }
                                            }
                                            if let Some(code) = status_code {
                                                span { class: "px-2 py-0.5 rounded text-[10px] font-mono bg-zinc-800 text-zinc-300", "HTTP {code}" }
                                            }
                                        }
                                        p { class: "text-xs text-zinc-300 leading-relaxed", "{message}" }
                                        p {
                                            class: "text-[11px] text-zinc-400 mt-1",
                                            "Servidor consultado: {server_url}. Asegúrate de ingresar un Token Key válido generado desde la web de Kyō."
                                        }
                                    }
                                }
                            }
                        },
                        ConnectionStatus::Disconnected => rsx! {
                            div {
                                class: "p-6 rounded-2xl border bg-zinc-900/60 border-zinc-800 space-y-2",
                                div {
                                    class: "flex items-start gap-4",
                                    div {
                                        class: "w-10 h-10 rounded-xl flex items-center justify-center bg-zinc-800 text-zinc-400 border border-zinc-700 flex-none",
                                        svg {
                                            class: "w-5 h-5",
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M1 1l22 22M16.72 11.06A10.94 10.94 0 0 1 19 12.55M5 12.55a10.94 10.94 0 0 1 5.17-2.39M10.71 5.05A16 16 0 0 1 22.58 9M1.42 9a15.91 15.91 0 0 1 4.7-2.88M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01" }
                                        }
                                    }
                                    div {
                                        h2 { class: "text-sm font-bold text-white", "Desconectado de Kyō Web" }
                                        p { class: "text-xs text-zinc-400 leading-relaxed mt-0.5", "Introduce tu Token Key a continuación para autenticar este software con tu cuenta y habilitar la sincronización en vivo." }
                                    }
                                }
                            }
                        },
                    }
                }

                // 2. GRID DE GESTIÓN DE TOKEN Y CUENTA
                div {
                    class: "grid grid-cols-1 lg:grid-cols-12 gap-6",

                    // COLUMNA IZQUIERDA: Flujo de conexión vía Token Key (7 columnas)
                    div {
                        class: "lg:col-span-7 space-y-6",

                        // Tarjeta: Autenticación por Token Key
                        div {
                            class: "p-6 rounded-2xl border space-y-5",
                            style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                            div {
                                class: "space-y-1 pb-3 border-b border-zinc-800",
                                h3 { class: "text-sm font-bold text-white flex items-center gap-2",
                                    svg {
                                        class: "w-4 h-4 text-indigo-400",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        path { d: "M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" }
                                    }
                                    "Clave de Acceso (Token Key)"
                                }
                                p { class: "text-xs text-zinc-400 leading-relaxed",
                                    "Ingresa el token generado en tu cuenta web de Kyō para conectar este cliente de escritorio con el backend."
                                }
                            }

                            // Campo de entrada del Token con Toggle Mostrar/Ocultar
                            div {
                                class: "space-y-2",
                                label {
                                    class: "block text-xs font-semibold text-zinc-300",
                                    "Token Key (Bearer / JWT):"
                                }
                                div {
                                    class: "relative flex items-center",
                                    input {
                                        r#type: if *is_token_visible.read() { "text" } else { "password" },
                                        class: "w-full pl-3 pr-24 py-2.5 rounded-xl font-mono text-xs focus:outline-none focus:ring-1 focus:ring-indigo-500/50 transition-all",
                                        style: "background-color: var(--kyo-surface-0); border: 1px solid var(--kyo-border-hover); color: var(--kyo-fg-primary);",
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

                                    // Botones dentro del input (Ojo y Limpiar)
                                    div {
                                        class: "absolute right-2 flex items-center gap-1",
                                        if !token_input.read().is_empty() {
                                            button {
                                                class: "p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer",
                                                title: "Limpiar campo",
                                                onclick: move |_| token_input.set(String::new()),
                                                svg {
                                                    class: "w-3.5 h-3.5",
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
                                                    class: "w-3.5 h-3.5",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    path { d: "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" }
                                                    line { x1: "1", y1: "1", x2: "23", y2: "23" }
                                                }
                                            } else {
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
                                    }
                                }
                            }

                            // Botones de Acción del Flujo de Conexión
                            div {
                                class: "flex flex-wrap items-center justify-between gap-3 pt-2",
                                div {
                                    class: "flex items-center gap-2",
                                    button {
                                        class: "px-4 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-sm shadow-indigo-600/30 flex items-center gap-2 transition-all cursor-pointer disabled:opacity-50",
                                        disabled: is_loading,
                                        onclick: move |_| handle_connect_and_save(),
                                        svg {
                                            class: if is_loading { "w-3.5 h-3.5 animate-spin" } else { "w-3.5 h-3.5" },
                                            view_box: "0 0 24 24",
                                            fill: "none",
                                            stroke: "currentColor",
                                            stroke_width: "2",
                                            path { d: "M5 13l4 4L19 7" }
                                        }
                                        span { if is_loading { "Verificando..." } else { "Conectar y Guardar" } }
                                    }

                                    if is_connected || !token_input.read().trim().is_empty() {
                                        button {
                                            class: "px-3 py-2 rounded-xl text-xs font-medium text-rose-400 hover:text-white hover:bg-rose-600/20 border border-rose-500/20 transition-all cursor-pointer",
                                            onclick: move |_| handle_disconnect(),
                                            "Desconectar"
                                        }
                                    }
                                }

                                // Botón para abrir la web y conseguir token
                                button {
                                    class: "text-xs font-medium text-indigo-400 hover:text-indigo-300 flex items-center gap-1.5 transition-colors cursor-pointer",
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

                            // Mensaje de éxito o retroalimentación
                            if *save_feedback_visible.read() {
                                div {
                                    class: "p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 text-xs flex items-center gap-2 animate-fade-in",
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

                            // Configuración avanzada de la URL del Servidor (Colapsable)
                            div {
                                class: "pt-2 border-t border-zinc-800/80 space-y-3",
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
                                        p { class: "text-[11px] text-zinc-400", "El valor predeterminado es https://kyo.broco.dev. Solo cámbialo si estás ejecutando una instancia local o un entorno de desarrollo dedicado." }
                                    }
                                }
                            }
                        }

                        // Tarjeta: Inspector de Token JWT en vivo
                        div {
                            class: "p-6 rounded-2xl border space-y-4",
                            style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                            div {
                                class: "flex items-center justify-between pb-3 border-b border-zinc-800",
                                h3 { class: "text-sm font-bold text-white flex items-center gap-2",
                                    svg {
                                        class: "w-4 h-4 text-emerald-400",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        rect { x: "2", y: "4", width: "20", height: "16", rx: "2" }
                                        path { d: "M6 8h4M6 12h8M6 16h6" }
                                    }
                                    "Inspector de Token en Vivo"
                                }
                                if let Some(ref j) = *jwt_info.read() {
                                    span {
                                        class: if j.is_expired {
                                            "px-2 py-0.5 rounded text-[10px] font-mono bg-rose-500/20 text-rose-300 border border-rose-500/30"
                                        } else {
                                            "px-2 py-0.5 rounded text-[10px] font-mono bg-emerald-500/20 text-emerald-300 border border-emerald-500/30"
                                        },
                                        if j.is_expired { "EXPIRADO" } else { "VÁLIDO" }
                                    }
                                }
                            }

                            if let Some(ref j) = *jwt_info.read() {
                                div {
                                    class: "space-y-3",
                                    {
                                        let alg_val = j.alg.as_deref().unwrap_or("HS256");
                                        let role_val = j.role.as_deref().unwrap_or("user");
                                        let fmt_val = if j.is_jwt { "JWT (RFC 7519)" } else { "Opaco / Bearer" };
                                        rsx! {
                                            div {
                                                class: "grid grid-cols-2 md:grid-cols-3 gap-2.5",
                                                div {
                                                    class: "p-3 rounded-xl bg-zinc-900/80 border border-zinc-800",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400", "Formato" }
                                                    div { class: "text-xs font-semibold text-zinc-200 mt-0.5", "{fmt_val}" }
                                                }
                                                div {
                                                    class: "p-3 rounded-xl bg-zinc-900/80 border border-zinc-800",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400", "Algoritmo" }
                                                    div { class: "text-xs font-semibold font-mono text-indigo-400 mt-0.5", "{alg_val}" }
                                                }
                                                div {
                                                    class: "p-3 rounded-xl bg-zinc-900/80 border border-zinc-800",
                                                    div { class: "text-[10px] font-mono uppercase text-zinc-400", "Rol Declarado" }
                                                    div { class: "text-xs font-semibold text-zinc-200 mt-0.5 capitalize", "{role_val}" }
                                                }
                                            }
                                        }
                                    }

                                    // Vigencia / Expiración
                                    if let Some(ref exp_str) = j.expiration_str {
                                        div {
                                            class: "p-3 rounded-xl bg-zinc-900/80 border border-zinc-800 flex items-center justify-between text-xs",
                                            span { class: "text-zinc-400", "Vigencia del Token:" }
                                            span {
                                                class: if j.is_expired { "font-mono font-semibold text-rose-400" } else { "font-mono font-semibold text-emerald-400" },
                                                "{exp_str}"
                                            }
                                        }
                                    }

                                    // Advertencia si existe
                                    if let Some(ref w) = j.warning {
                                        div {
                                            class: "p-3 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs flex items-center gap-2",
                                            svg {
                                                class: "w-4 h-4 flex-none",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0zM12 9v4m0 4h.01" }
                                            }
                                            span { "{w}" }
                                        }
                                    }
                                }
                            } else {
                                div {
                                    class: "p-4 rounded-xl bg-zinc-900/40 border border-zinc-800/80 text-center text-xs text-zinc-400",
                                    "Pega tu Token Key en el campo superior para inspeccionar sus claims y tiempo de vigencia en tiempo real."
                                }
                            }
                        }
                    }

                    // COLUMNA DERECHA: Estado de la Cuenta & Diagnóstico (5 columnas)
                    div {
                        class: "lg:col-span-5 space-y-6",

                        // Tarjeta: Cuenta de Usuario Autenticada
                        div {
                            class: "p-6 rounded-2xl border space-y-4",
                            style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                            div {
                                class: "flex items-center gap-3 pb-3 border-b border-zinc-800",
                                div {
                                    class: "w-10 h-10 rounded-full bg-gradient-to-tr from-indigo-500 to-violet-600 flex items-center justify-center text-white font-bold text-sm shadow-md shadow-indigo-500/20",
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
                                    h3 { class: "text-sm font-bold text-white", "Cuenta en Kyō" }
                                    p { class: "text-xs text-zinc-400", "Identidad y recursos asociados al token" }
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
                                            class: "space-y-3 text-xs",
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "Usuario:" }
                                                span { class: "font-semibold text-white", "{name_str}" }
                                            }
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "ID de Cuenta:" }
                                                span { class: "font-mono text-zinc-300", "{uid_str}" }
                                            }
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "Rol:" }
                                                span { class: "px-2 py-0.5 rounded text-[10px] font-mono uppercase bg-indigo-500/20 text-indigo-300 border border-indigo-500/30", "{role_str}" }
                                            }
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "Bóveda Principal:" }
                                                span { class: "font-medium text-emerald-400", "{primary_v}" }
                                            }
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "Espacios Vinculados:" }
                                                span { class: "font-semibold text-white", "{sp_count} espacios en la nube" }
                                            }
                                            div {
                                                class: "flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800",
                                                span { class: "text-zinc-400", "Bóvedas en Servidor:" }
                                                span { class: "font-semibold text-white", "{vl_count} bóvedas" }
                                            }
                                        }
                                    }
                                } else {
                                    rsx! {
                                        div {
                                            class: "p-4 rounded-xl bg-zinc-900/40 border border-zinc-800/80 text-center space-y-2 text-xs text-zinc-400",
                                            p { "No hay ninguna cuenta autenticada en este momento." }
                                            p { class: "text-[11px] text-zinc-400", "Conecta tu Token Key para ver los detalles de tu usuario y sincronizar tus espacios." }
                                        }
                                    }
                                }
                            }
                        }

                        // Tarjeta: Diagnóstico de Red & Conectividad
                        div {
                            class: "p-6 rounded-2xl border space-y-3 text-xs",
                            style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-default);",

                            h3 { class: "text-sm font-bold text-white flex items-center gap-2",
                                svg {
                                    class: "w-4 h-4 text-cyan-400",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    path { d: "M5 12h14M12 5l7 7-7 7" }
                                }
                                "Diagnóstico del Entorno"
                            }

                            div {
                                class: "space-y-2 text-zinc-300 font-mono text-[11px]",
                                div {
                                    class: "p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex items-center justify-between",
                                    span { class: "text-zinc-400 font-sans", "Endpoint API:" }
                                    span { class: "text-zinc-300 truncate max-w-[200px]", "{server_url_input}/api/v1" }
                                }
                                div {
                                    class: "p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex items-center justify-between",
                                    span { class: "text-zinc-400 font-sans", "Persistencia:" }
                                    span { class: "text-emerald-400 font-sans", "%APPDATA%/kyo/session.json" }
                                }
                                div {
                                    class: "p-2.5 rounded-xl bg-zinc-900/60 border border-zinc-800 flex items-center justify-between",
                                    span { class: "text-zinc-400 font-sans", "Seguridad:" }
                                    span { class: "text-indigo-300 font-sans", "Bearer Token TLS Encrypted" }
                                }
                            }

                            // Botón para ir al chat
                            div {
                                class: "pt-2",
                                button {
                                    class: "w-full py-2.5 rounded-xl text-xs font-semibold bg-zinc-800 hover:bg-zinc-700 text-zinc-200 border border-zinc-700 flex items-center justify-center gap-2 transition-all cursor-pointer",
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
                        }
                    }
                }
            }
        }
    }
}
