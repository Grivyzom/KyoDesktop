use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const DEFAULT_SERVER_URL: &str = "https://kyo.broco.dev";
pub const OAUTH_CLIENT_ID: &str = "kyo-desktop";
const USER_AGENT: &str = "kyo-desktop/0.1.0";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct RawTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct RawMeResponse {
    id: Option<serde_json::Value>,
    email: Option<String>,
    username: Option<String>,
    name: Option<String>,
    user: Option<Box<RawMeResponse>>,
    data: Option<Box<RawMeResponse>>,
}

fn base64url_encode(bytes: &[u8]) -> String {
    let std_b64 = crate::token_crypto::base64_encode(bytes);
    std_b64
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

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
        _ => {
            return Err("El servidor debe usar https:// (http:// sólo para localhost).".to_string());
        }
    }

    let mut path = parsed.path().trim_end_matches('/').to_string();
    if let Some(stripped) = path.strip_suffix("/api/v1") {
        path = stripped.to_string();
    }
    let port_part = parsed
        .port()
        .map(|p| format!(":{p}"))
        .unwrap_or_default();
    let host_fmt = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    Ok(format!("{}://{}{}{}", parsed.scheme(), host_fmt, port_part, path))
}

fn random_b64url(len: usize) -> Result<String, String> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).map_err(|e| format!("Error generando entropía: {e}"))?;
    Ok(base64url_encode(&buf))
}

pub fn pkce_pair() -> Result<(String, String), String> {
    let verifier = random_b64url(32)?;
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64url_encode(&digest);
    Ok((verifier, challenge))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
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

fn build_authorize_url(server_url: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
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

async fn send_http_response(
    stream: &mut tokio::net::TcpStream,
    status_line: &str,
    body: &str,
) {
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

fn rfc3339_after_seconds(secs: u64) -> String {
    let target = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_add(secs);
    unix_to_rfc3339(target)
}

fn unix_to_rfc3339(ts: u64) -> String {
    // Civil-from-days algorithm (Howard Hinnant)
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

fn build_no_redirect_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {e}"))
}

async fn token_request(server_url: &str, form: &[(&str, &str)]) -> Result<TokenSet, String> {
    let client = build_no_redirect_client()?;
    let body = build_form_body(form);
    let res = client
        .post(format!("{server_url}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .body(body)
        .send()
        .await
        .map_err(|e| format!("No se pudo conectar con {server_url}: {e}"))?;

    let status = res.status();
    let parsed: RawTokenResponse = res
        .json()
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

    let expires_at = parsed.expires_in.map(rfc3339_after_seconds);
    Ok(TokenSet {
        access_token,
        refresh_token: parsed.refresh_token.filter(|t| !t.trim().is_empty()),
        expires_at,
    })
}

async fn fetch_me(server_url: &str, access_token: &str) -> (Option<String>, Option<String>) {
    let Ok(client) = build_no_redirect_client() else {
        return (None, None);
    };
    let Ok(res) = client
        .get(format!("{server_url}/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {access_token}"))
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
    let Ok(raw) = res.json::<RawMeResponse>().await else {
        return (None, None);
    };
    let inner = raw
        .user
        .as_deref()
        .or(raw.data.as_deref())
        .unwrap_or(&raw);
    let username = inner
        .username
        .as_deref()
        .or(inner.name.as_deref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = inner
        .email
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    (username, email)
}

async fn revoke_token_best_effort(server_url: &str, token: &str) {
    if token.trim().is_empty() {
        return;
    }
    if let Ok(client) = build_no_redirect_client() {
        let body = build_form_body(&[("token", token)]);
        let _ = client
            .post(format!("{server_url}/oauth/revoke"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", USER_AGENT)
            .body(body)
            .send()
            .await;
    }
}

#[tauri::command]
pub async fn oauth_login(
    app: AppHandle,
    server_url: Option<String>,
) -> Result<crate::SessionConfig, String> {
    let raw_url = server_url
        .or_else(|| crate::get_session_config().server_url)
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string());
    let base_url = normalize_server_url(&raw_url)?;

    let (verifier, challenge) = pkce_pair()?;
    let state = random_b64url(16)?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("No se pudo abrir puerto loopback OAuth: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Error obteniendo puerto local: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let authorize_url = build_authorize_url(&base_url, &redirect_uri, &challenge, &state);

    app.opener()
        .open_url(&authorize_url, None::<&str>)
        .map_err(|e| format!("No se pudo abrir el navegador: {e}"))?;

    let expected_state = state.clone();
    let code = tokio::time::timeout(LOGIN_TIMEOUT, async move {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| format!("Error en servidor callback OAuth: {e}"))?;
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
                send_http_response(&mut stream, "400 Bad Request", &page).await;
                continue;
            }

            if !param_error.is_empty() || param_code.is_empty() {
                let page = html_page(
                    "Acceso no concedido",
                    "Puedes cerrar esta pestaña y volver a Kyō Desktop.",
                );
                send_http_response(&mut stream, "200 OK", &page).await;
                let desc = if !param_error_desc.is_empty() {
                    param_error_desc
                } else if !param_error.is_empty() {
                    param_error
                } else {
                    "sin código de autorización".to_string()
                };
                return Err(format!("El servidor rechazó el login: {desc}"));
            }

            let page = html_page(
                "Listo ✓",
                "Sesión iniciada en Kyō Desktop. Ya puedes cerrar esta pestaña.",
            );
            send_http_response(&mut stream, "200 OK", &page).await;
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
    crate::save_oauth_session_internal(
        &tokens.access_token,
        tokens.refresh_token.as_deref(),
        tokens.expires_at.as_deref(),
        OAUTH_CLIENT_ID,
        &base_url,
        username.as_deref(),
        email.as_deref(),
    )?;

    Ok(crate::get_session_config())
}

#[tauri::command]
pub async fn oauth_refresh(server_url: Option<String>) -> Result<crate::SessionConfig, String> {
    let raw_url = server_url
        .or_else(|| crate::get_session_config().server_url)
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string());
    let base_url = normalize_server_url(&raw_url)?;
    let (refresh_tok, client_id) = crate::get_stored_refresh_token()
        .ok_or_else(|| "No hay refresh token guardado para renovar la sesión.".to_string())?;
    let cid = client_id.unwrap_or_else(|| OAUTH_CLIENT_ID.to_string());

    let tokens = token_request(
        &base_url,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &cid),
            ("refresh_token", &refresh_tok),
        ],
    )
    .await?;

    let (username, email) = fetch_me(&base_url, &tokens.access_token).await;
    let next_refresh = tokens.refresh_token.as_deref().or(Some(refresh_tok.as_str()));
    crate::save_oauth_session_internal(
        &tokens.access_token,
        next_refresh,
        tokens.expires_at.as_deref(),
        &cid,
        &base_url,
        username.as_deref(),
        email.as_deref(),
    )?;

    Ok(crate::get_session_config())
}

#[tauri::command]
pub async fn oauth_logout() -> Result<(), String> {
    let cfg = crate::get_session_config();
    let refresh_opt = crate::get_stored_refresh_token().map(|(t, _)| t);

    // Limpiar credenciales locales primero (aunque el servidor no responda)
    crate::clear_session_config_internal()?;

    if let Some(url) = cfg.server_url {
        if let Ok(base_url) = normalize_server_url(&url) {
            if let Some(ref rt) = refresh_opt {
                revoke_token_best_effort(&base_url, rt).await;
            }
            if let Some(ref at) = cfg.token {
                revoke_token_best_effort(&base_url, at).await;
            }
        }
    }
    Ok(())
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
            normalize_server_url("http://127.0.0.1:8080/api/v1").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert!(normalize_server_url("http://kyo.broco.dev").is_err());
        assert!(normalize_server_url("https://user:pass@kyo.broco.dev").is_err());
    }

    #[test]
    fn test_pkce_pair_s256() {
        let (verifier, challenge) = pkce_pair().unwrap();
        assert!(verifier.len() >= 43);
        assert!(!challenge.contains('='));
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        let expected = base64url_encode(&Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
    }

    #[test]
    fn test_unix_to_rfc3339() {
        assert_eq!(unix_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_rfc3339(1700000000), "2023-11-14T22:13:20Z");
    }
}
