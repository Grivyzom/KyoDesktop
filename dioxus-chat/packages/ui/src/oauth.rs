//! Autenticación nativa OAuth 2.1 + PKCE (S256) con servidor loopback efímero para Kyo Desktop.
//!
//! Equivalente nativo en Rust a `packages/kyo-cli/src/oauth.ts` + `commands/auth.ts`:
//! - Levanta un servidor HTTP temporal en `127.0.0.1:0` (RFC 8252 §7.3).
//! - Abre el navegador del sistema sin shell injection (`rundll32` en Windows, `open` en macOS, `xdg-open` en Linux).
//! - Valida `state` en tiempo constante y responde una página endurecida (`Cache-Control: no-store`).
//! - Canjea el código por `access_token` + `refresh_token` con `redirect::Policy::none()`.
//! - Renueva automáticamente el token ante vencimiento próximo (<3 días) o ante 401.
//! - Revoca tokens al cerrar sesión (`POST /oauth/revoke`).

use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::settings::{
    clear_session_auth, load_access_expires_at, load_auth_token, load_oauth_client_id,
    load_refresh_token, load_server_url, save_oauth_session,
};

pub const DEFAULT_SERVER_URL: &str = "https://kyo.broco.dev";
pub const OAUTH_CLIENT_ID: &str = "kyo-desktop";
pub const USER_AGENT: &str = "kyo-desktop/0.1.0";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const REFRESH_MARGIN_SECS: i64 = 3 * 24 * 60 * 60;

static REFRESH_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OAuthLoginOutcome {
    pub server_url: String,
    pub tokens: OAuthTokenSet,
    pub username: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RawTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Normaliza la URL base del servidor Kyō (exige `https://`, permite `http://` sólo en loopback).
pub fn normalize_server_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_SERVER_URL.to_string());
    }
    let with_scheme = if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = reqwest::Url::parse(&with_scheme)
        .map_err(|_| format!("URL de servidor inválida: {trimmed}"))?;

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("No se permiten credenciales embebidas en la URL.".to_string());
    }

    let host = parsed.host_str().unwrap_or("");
    let is_loopback = host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]";
    match parsed.scheme() {
        "https" => {}
        "http" if is_loopback => {}
        _ => return Err("El servidor debe usar https:// (http:// sólo para localhost).".to_string()),
    }

    let mut path = parsed.path().trim_end_matches('/').to_string();
    if let Some(stripped) = path.strip_suffix("/api/v1") {
        path = stripped.to_string();
    }
    let port_part = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
    let host_fmt = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    Ok(format!("{}://{}{}{}", parsed.scheme(), host_fmt, port_part, path))
}

fn random_b64url(num_bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; num_bytes];
    getrandom::getrandom(&mut buf).map_err(|e| format!("Error generando entropía: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(&buf))
}

pub fn pkce_pair() -> Result<(String, String), String> {
    let verifier = random_b64url(32)?;
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);
    Ok((verifier, challenge))
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn url_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn url_decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn build_form_body(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode_component(k), url_encode_component(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn build_authorize_url(server_url: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
    let query = build_form_body(&[
        ("response_type", "code"),
        ("client_id", OAUTH_CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("scope", "mcp:all"),
    ]);
    format!("{server_url}/oauth/authorize?{query}")
}

/// Abre una URL en el navegador predeterminado sin pasar por un shell (previene inyección de `&` en `cmd`).
pub fn open_browser_safe(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|e| format!("No se pudo abrir el navegador: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("No se pudo abrir el navegador: {e}"))?;
        return Ok(());
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("No se pudo abrir el navegador: {e}"))?;
        Ok(())
    }
}

pub fn unix_to_rfc3339(ts: u64) -> String {
    let days = (ts / 86400) as i64;
    let sod = ts % 86400;
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let ss = sod % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convierte un timestamp RFC 3339 básico (`YYYY-MM-DDTHH:MM:SSZ` o con `.sssZ` / `+00:00`) a epoch seconds.
pub fn parse_rfc3339_unix(iso: &str) -> Option<i64> {
    let s = iso.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let hh: i64 = s.get(11..13)?.parse().ok()?;
    let mm: i64 = s.get(14..16)?.parse().ok()?;
    let ss: i64 = s.get(17..19)?.parse().ok()?;

    // Inverse civil-from-days (Howard Hinnant)
    let y_adj = y - if m <= 2 { 1 } else { 0 };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hh * 3600 + mm * 60 + ss)
}

pub fn expires_soon(iso: Option<&str>) -> bool {
    let Some(s) = iso else {
        return false;
    };
    let Some(exp_sec) = parse_rfc3339_unix(s) else {
        return false;
    };
    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    exp_sec - now_sec < REFRESH_MARGIN_SECS
}

fn no_redirect_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {e}"))
}

async fn token_request(server_url: &str, form: &[(&str, &str)]) -> Result<OAuthTokenSet, String> {
    let client = no_redirect_client()?;
    let res = client
        .post(format!("{server_url}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .body(build_form_body(form))
        .send()
        .await
        .map_err(|e| format!("No se pudo conectar con {server_url}: {e}"))?;

    let status = res.status();
    let parsed = res
        .json::<RawTokenResponse>()
        .await
        .unwrap_or(RawTokenResponse {
            access_token: None,
            refresh_token: None,
            expires_in: None,
            error: None,
            error_description: None,
        });

    if !status.is_success() {
        let msg = parsed
            .error_description
            .or(parsed.error)
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(format!("No se pudo obtener el token: {msg}"));
    }

    let access_token = parsed
        .access_token
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "El servidor no devolvió access_token".to_string())?;

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let expires_at = parsed
        .expires_in
        .map(|s| unix_to_rfc3339(now_sec.saturating_add(s)));

    Ok(OAuthTokenSet {
        access_token,
        refresh_token: parsed.refresh_token.filter(|t| !t.trim().is_empty()),
        expires_at,
    })
}

async fn fetch_me(server_url: &str, token: &str) -> (Option<String>, Option<String>) {
    let Ok(client) = no_redirect_client() else {
        return (None, None);
    };
    let Ok(res) = client
        .get(format!("{server_url}/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send()
        .await
    else {
        return (None, None);
    };
    if !res.status().is_success() {
        return (None, None);
    }
    let Ok(val) = res.json::<serde_json::Value>().await else {
        return (None, None);
    };
    let obj = val.get("user").or_else(|| val.get("data")).unwrap_or(&val);
    let username = obj
        .get("username")
        .or_else(|| obj.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = obj
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    (username, email)
}

fn html_page(title: &str, msg: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Kyō Desktop</title>\
         <body style=\"font-family:system-ui,-apple-system,sans-serif;display:grid;place-items:center;height:100vh;margin:0;background:#09090b;color:#f4f4f5\">\
         <div style=\"text-align:center;max-width:440px;padding:32px;border:1px solid #27272a;border-radius:16px;background:#18181b\">\
         <h1 style=\"font-weight:600;margin:0 0 8px;font-size:20px\">{title}</h1>\
         <p style=\"margin:0;color:#a1a1aa;font-size:14px;line-height:1.5\">{msg}</p>\
         </div></body>"
    )
}

async fn send_http_html(stream: &mut tokio::net::TcpStream, status_line: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status_line}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Referrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Ejecuta el flujo completo de inicio de sesión OAuth 2.1 + PKCE en navegador y guarda la sesión.
pub async fn browser_login(raw_server_url: &str) -> Result<OAuthLoginOutcome, String> {
    let base_url = normalize_server_url(raw_server_url)?;
    let (verifier, challenge) = pkce_pair()?;
    let state = random_b64url(16)?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("No se pudo iniciar el servidor loopback OAuth: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("No se pudo leer el puerto loopback: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let authorize_url = build_authorize_url(&base_url, &redirect_uri, &challenge, &state);

    open_browser_safe(&authorize_url)?;

    let expected_state = state.clone();
    let code = tokio::time::timeout(LOGIN_TIMEOUT, async move {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| format!("Error recibiendo callback OAuth: {e}"))?;
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                continue;
            }
            let req_str = String::from_utf8_lossy(&buf[..n]);
            let first_line = req_str.lines().next().unwrap_or("");
            let mut parts = first_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("/");
            let (path, query) = target.split_once('?').unwrap_or((target, ""));

            if method != "GET" || path != "/callback" {
                let _ = stream
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
                continue;
            }

            let mut param_state = String::new();
            let mut param_code = String::new();
            let mut param_error = String::new();
            let mut param_error_desc = String::new();
            for kv in query.split('&') {
                if let Some((k, v)) = kv.split_once('=') {
                    let val = url_decode_component(v);
                    match k {
                        "state" => param_state = val,
                        "code" => param_code = val,
                        "error" => param_error = val,
                        "error_description" => param_error_desc = val,
                        _ => {}
                    }
                }
            }

            if !constant_time_eq(&param_state, &expected_state) {
                let page = html_page(
                    "Respuesta inválida",
                    "Esta pestaña no corresponde al inicio de sesión en curso.",
                );
                send_http_html(&mut stream, "400 Bad Request", &page).await;
                continue;
            }

            if !param_error.is_empty() || param_code.is_empty() {
                let page = html_page(
                    "Acceso no concedido",
                    "Puedes cerrar esta pestaña y volver a Kyō Desktop.",
                );
                send_http_html(&mut stream, "200 OK", &page).await;
                let detail = if !param_error_desc.is_empty() {
                    param_error_desc
                } else if !param_error.is_empty() {
                    param_error
                } else {
                    "sin código de autorización".to_string()
                };
                return Err(format!("El servidor rechazó el login: {detail}"));
            }

            let page = html_page(
                "Listo ✓",
                "Sesión iniciada en Kyō Desktop. Ya puedes cerrar esta pestaña.",
            );
            send_http_html(&mut stream, "200 OK", &page).await;
            return Ok(param_code);
        }
    })
    .await
    .map_err(|_| "Se agotó el tiempo de espera del login en navegador (5 min).".to_string())??;

    let tokens = token_request(
        &base_url,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", OAUTH_CLIENT_ID),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect_uri),
        ],
    )
    .await?;

    let (username, email) = fetch_me(&base_url, &tokens.access_token).await;
    save_oauth_session(
        &tokens.access_token,
        tokens.refresh_token.as_deref(),
        tokens.expires_at.as_deref(),
        OAUTH_CLIENT_ID,
        &base_url,
        username.as_deref(),
        email.as_deref(),
    )?;

    Ok(OAuthLoginOutcome {
        server_url: base_url,
        tokens,
        username,
        email,
    })
}

/// Renueva el access token OAuth si está por expirar (<3 días) o si `force` es `true`.
pub async fn ensure_fresh_account_token(force: bool) -> Result<(String, String), String> {
    let lock = REFRESH_MUTEX.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    let server_url = normalize_server_url(&load_server_url())?;
    let current_token = load_auth_token();
    if current_token.is_empty() {
        return Err("No has iniciado sesión. Inicia sesión con Kyō desde el Panel de control.".to_string());
    }

    let exp = load_access_expires_at();
    let rt_opt = load_refresh_token();
    if let Some(rt) = rt_opt {
        if force || expires_soon(exp.as_deref()) {
            let cid = load_oauth_client_id().unwrap_or_else(|| OAUTH_CLIENT_ID.to_string());
            if let Ok(fresh) = token_request(
                &server_url,
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", &cid),
                    ("refresh_token", &rt),
                ],
            )
            .await
            {
                let (u, e) = fetch_me(&server_url, &fresh.access_token).await;
                let next_rt = fresh.refresh_token.as_deref().or(Some(rt.as_str()));
                let _ = save_oauth_session(
                    &fresh.access_token,
                    next_rt,
                    fresh.expires_at.as_deref(),
                    &cid,
                    &server_url,
                    u.as_deref(),
                    e.as_deref(),
                );
                return Ok((server_url, fresh.access_token));
            }
        }
    }

    Ok((server_url, current_token))
}

/// Cierra la sesión local inmediatamente y revoca los tokens OAuth en el servidor (best effort).
pub async fn logout_and_revoke() {
    let server_url = normalize_server_url(&load_server_url()).ok();
    let access_tok = load_auth_token();
    let refresh_tok = load_refresh_token();

    let _ = clear_session_auth();

    let Some(base_url) = server_url else {
        return;
    };
    let Ok(client) = no_redirect_client() else {
        return;
    };

    for tok in [refresh_tok.as_deref(), (!access_tok.is_empty()).then_some(access_tok.as_str())]
        .into_iter()
        .flatten()
    {
        let _ = client
            .post(format!("{base_url}/oauth/revoke"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", USER_AGENT)
            .body(build_form_body(&[("token", tok)]))
            .send()
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_server_url() {
        assert_eq!(
            normalize_server_url("https://kyo.broco.dev/api/v1/").unwrap(),
            "https://kyo.broco.dev"
        );
        assert_eq!(
            normalize_server_url("http://localhost:8080/api/v1").unwrap(),
            "http://localhost:8080"
        );
        assert!(normalize_server_url("http://kyo.broco.dev").is_err());
        assert!(normalize_server_url("https://u:p@kyo.broco.dev").is_err());
    }

    #[test]
    fn test_pkce_s256() {
        let (v, c) = pkce_pair().unwrap();
        assert!(v.len() >= 43);
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes()));
        assert_eq!(c, expected);
    }

    #[test]
    fn test_rfc3339_roundtrip() {
        let ts = 1_760_000_000u64;
        let iso = unix_to_rfc3339(ts);
        assert_eq!(parse_rfc3339_unix(&iso), Some(ts as i64));
        assert_eq!(unix_to_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
