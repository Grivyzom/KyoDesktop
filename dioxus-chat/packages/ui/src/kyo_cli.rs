//! Motor nativo de `kyo-cli` en Rust para Kyo Desktop.
//!
//! Implementa de forma 100% nativa (sin requerir Node.js ni el binario externo `kyo-cli`):
//! - Descubrimiento de repositorios Git, lectura de `remote.origin.url` sin credenciales,
//!   ramas y cálculo de huella (`git_remote` / `path_hash`).
//! - Gestión atómica y protegida contra enlaces simbólicos de `kyo-project.json`,
//!   `.kyo/credentials` y `.gitignore`.
//! - Generación del contexto para agentes (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`),
//!   registro de servidor MCP en `.mcp.json` (puente nativo `go-server mcp`, `kyo-cli mcp` o HTTP),
//!   barra de estado de Claude Code (`statusLine`) y hook `Stop` de auto-log.
//! - Registro compartido de proyectos (`projects.json` interoperable con `kyo-cli ui`).
//! - Operaciones REST `/api/v1/project-links*` con renovación automática de sesión OAuth.
//! - Cliente Streamable HTTP MCP para listar y agrupar tools activas.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::oauth::{ensure_fresh_account_token, parse_rfc3339_unix, USER_AGENT};
use crate::settings::get_kyo_data_dir;
use crate::sidecar::find_sidecar_executable;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const PROJECT_FILE: &str = "kyo-project.json";
const CRED_DIR: &str = ".kyo";
const CRED_FILE: &str = "credentials";
const STATE_REL: &str = ".kyo/hook-state.json";
const PREV_STATUSLINE_REL: &str = ".kyo/statusline-prev.json";
const CLAUDE_SETTINGS_REL: &str = ".claude/settings.local.json";
const MCP_FILE: &str = ".mcp.json";
const MCP_NAME: &str = "kyo";
const MAX_REGISTRY_ROOTS: usize = 200;
pub const RENEW_WARNING_DAYS: i64 = 5;

const BEGIN_MARKER: &str = "<!-- kyo:begin — generado por `kyo-cli project sync`; no edites dentro del bloque -->";
const END_MARKER: &str = "<!-- kyo:end -->";
const CONTEXT_FILES: [&str; 3] = ["CLAUDE.md", "AGENTS.md", "GEMINI.md"];

// ── Sanitización ─────────────────────────────────────────────────────────────

pub fn clean_text(raw: &str, max: usize) -> String {
    let mut out = String::with_capacity(raw.len().min(max));
    let mut chars_written = 0usize;
    let mut prev_ws = false;
    for ch in raw.chars() {
        let mapped = if ch == '\n' || ch == '\r' || ch == '\t' {
            ' '
        } else if ch.is_control() {
            continue;
        } else {
            ch
        };
        if mapped == ' ' {
            if prev_ws {
                continue;
            }
            prev_ws = true;
        } else {
            prev_ws = false;
        }
        if chars_written >= max {
            break;
        }
        out.push(mapped);
        chars_written += 1;
    }
    out.trim().to_string()
}

fn plain_inline(raw: &str, max: usize) -> String {
    let cleaned = clean_text(raw, max);
    let mut out = String::with_capacity(cleaned.len());
    for ch in cleaned.chars() {
        if matches!(ch, '`' | '*' | '_' | '#' | '<' | '>' | '[' | ']' | '|' | '\\') {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn safe_id(raw: &str) -> String {
    if !raw.is_empty()
        && raw.len() <= 64
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        raw.to_string()
    } else {
        "?".to_string()
    }
}

// ── Modelos de Proyecto & API ────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocationInfo {
    pub hostname: String,
    pub path: String,
    pub branch: String,
    pub remotes: Vec<RemoteInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub space_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default)]
    pub template_space_id: String,
    #[serde(default)]
    pub template_version: String,
    #[serde(default)]
    pub linked_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_log: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTool {
    #[serde(default)]
    pub row_id: String,
    #[serde(rename = "type", default)]
    pub tool_type: String,
    #[serde(default)]
    pub tool_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tools: Vec<WorkspaceTool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkInfo {
    #[serde(default)]
    pub space_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub workspace: Option<WorkspaceInfo>,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
    #[serde(default)]
    pub space_name: Option<String>,
    #[serde(default)]
    pub template_space_id: String,
    #[serde(default)]
    pub template_version: String,
    #[serde(default)]
    pub linked_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub expired: Option<bool>,
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub linked: bool,
    #[serde(default)]
    pub orphaned: Option<bool>,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub workspace: Option<WorkspaceInfo>,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
    #[serde(default)]
    pub space_name: Option<String>,
    #[serde(default)]
    pub template_space_id: Option<String>,
    #[serde(default)]
    pub template_version: Option<String>,
    #[serde(default)]
    pub linked_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub expired: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceSummary {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpBridgeMode {
    NativeDesktop,
    KyoCli,
    HttpDirect,
}

impl McpBridgeMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NativeDesktop => "Nativo KyoDesktop (go-server mcp)",
            Self::KyoCli => "CLI npm (kyo-cli mcp)",
            Self::HttpDirect => "HTTP directo (/api/v1/mcp)",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalFilesAudit {
    pub kyo_project_json: bool,
    pub credentials_file: bool,
    pub credentials_tracked_in_git: bool,
    pub claude_md: bool,
    pub agents_md: bool,
    pub gemini_md: bool,
    pub mcp_json: bool,
    pub statusline_installed: bool,
    pub auto_log_hook_installed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectState {
    Ok,
    NoSession,
    Unlinked,
    Absent,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRow {
    pub root: String,
    pub name: String,
    pub branch: String,
    pub hostname: String,
    pub state: ProjectState,
    pub auto_log: bool,
    pub has_token: bool,
    pub expires_at: Option<String>,
    pub days: Option<i64>,
    pub workspace: Option<WorkspaceInfo>,
    pub space_id: Option<String>,
    pub project_id: Option<String>,
    pub remotes: Vec<String>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub audit: LocalFilesAudit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub domain: String,
    pub dangerous: bool,
}

// ── Git & Huella ─────────────────────────────────────────────────────────────

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn find_project_root(cwd: &Path) -> PathBuf {
    if let Some(top) = run_git(cwd, &["rev-parse", "--show-toplevel"]) {
        return PathBuf::from(top);
    }
    let abs = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut cur = abs.clone();
    loop {
        if cur.join(".git").exists() {
            return cur;
        }
        if !cur.pop() {
            return abs;
        }
    }
}

pub fn strip_credentials(remote: &str) -> String {
    let trimmed = remote.trim();
    let has_scheme = trimmed
        .split_once("://")
        .map(|(sch, _)| !sch.is_empty() && sch.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)))
        .unwrap_or(false);
    if !has_scheme {
        return trimmed.to_string();
    }
    if let Ok(mut parsed) = reqwest::Url::parse(trimmed) {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        return parsed.to_string();
    }
    if let Some((left, right)) = trimmed.split_once("://") {
        if let Some((_, host_part)) = right.split_once('@') {
            return format!("{left}://{host_part}");
        }
    }
    trimmed.to_string()
}

pub fn parse_origin_from_config(config: &str) -> Option<String> {
    let mut in_origin = false;
    for raw in config.lines() {
        let line = raw.trim();
        if line.starts_with("[remote \"origin\"]") {
            in_origin = true;
        } else if line.starts_with('[') {
            in_origin = false;
        } else if in_origin && line.starts_with("url") {
            if let Some((_, val)) = line.split_once('=') {
                let t = val.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

pub fn parse_remotes_from_config(config: &str) -> Vec<RemoteInfo> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for raw in config.lines() {
        let line = raw.trim();
        if line.starts_with("[remote \"") && line.ends_with("\"]") {
            let name = &line[9..line.len() - 2];
            current = if name.is_empty() { None } else { Some(name.to_string()) };
        } else if line.starts_with('[') {
            current = None;
        } else if let Some(ref name) = current {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "url" {
                    let u = v.trim();
                    if !u.is_empty() {
                        out.push(RemoteInfo {
                            name: name.clone(),
                            url: strip_credentials(u),
                        });
                    }
                }
            }
        }
    }
    out
}

pub fn read_origin_remote(root: &Path) -> Option<String> {
    let git_path = root.join(".git");
    if git_path.is_dir() {
        let cfg = std::fs::read_to_string(git_path.join("config")).ok()?;
        return parse_origin_from_config(&cfg);
    }
    run_git(root, &["config", "--get", "remote.origin.url"])
}

fn get_hostname() -> String {
    #[cfg(windows)]
    if let Ok(h) = std::env::var("COMPUTERNAME") {
        if !h.trim().is_empty() {
            return h.trim().to_string();
        }
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.trim().is_empty() {
            return h.trim().to_string();
        }
    }
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        let t = h.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "desktop".to_string()
}

pub fn path_hash(root: &Path) -> String {
    let resolved = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let input = format!("{}|{}", get_hostname(), resolved.to_string_lossy());
    let sum = Sha256::digest(input.as_bytes());
    sum.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn project_ref(root: &Path) -> ProjectRef {
    if let Some(remote) = read_origin_remote(root) {
        ProjectRef {
            git_remote: Some(strip_credentials(&remote)),
            path_hash: None,
        }
    } else {
        ProjectRef {
            git_remote: None,
            path_hash: Some(path_hash(root)),
        }
    }
}

pub fn project_name(root: &Path, r: &ProjectRef) -> String {
    if let Some(ref remote) = r.git_remote {
        let trimmed = remote.trim_end_matches('/');
        let last = trimmed.rsplit(['/', ':']).next().unwrap_or("");
        let name = last.strip_suffix(".git").unwrap_or(last);
        if !name.is_empty() {
            return name.to_string();
        }
    }
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "proyecto".to_string())
}

pub fn current_branch(root: &Path) -> String {
    let head_path = root.join(".git").join("HEAD");
    if let Ok(head) = std::fs::read_to_string(head_path) {
        if let Some(branch) = head.trim().strip_prefix("ref: refs/heads/") {
            return branch.to_string();
        }
    }
    run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|b| b != "HEAD")
        .unwrap_or_default()
}

pub fn location_info(root: &Path) -> LocationInfo {
    let mut remotes = Vec::new();
    let git_path = root.join(".git");
    if git_path.is_dir() {
        if let Ok(cfg) = std::fs::read_to_string(git_path.join("config")) {
            remotes = parse_remotes_from_config(&cfg);
        }
    } else if let Some(out) = run_git(root, &["config", "--get-regexp", r"^remote\..*\.url$"]) {
        for line in out.lines() {
            if let Some((key, val)) = line.split_once(' ') {
                let name = key
                    .strip_prefix("remote.")
                    .unwrap_or(key)
                    .strip_suffix(".url")
                    .unwrap_or(key);
                remotes.push(RemoteInfo {
                    name: name.to_string(),
                    url: strip_credentials(val),
                });
            }
        }
    }
    let resolved = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    LocationInfo {
        hostname: get_hostname(),
        path: resolved.to_string_lossy().to_string(),
        branch: current_branch(root),
        remotes,
    }
}

// ── Archivos locales seguros contra symlinks ─────────────────────────────────

pub fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn read_regular(path: &Path) -> Option<String> {
    if is_symlink(path) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn safe_write(file: &Path, data: &str, unix_mode: u32) -> Result<(), String> {
    if is_symlink(file) {
        return Err(format!(
            "{} es un enlace simbólico: por seguridad no se escribe a través de él.",
            file.display()
        ));
    }
    let parent = file.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| format!("Error creando directorio: {e}"))?;

    let mut rnd = [0u8; 6];
    let _ = getrandom::getrandom(&mut rnd);
    let hex_rnd: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
    let base = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let tmp = parent.join(format!(".{base}.{hex_rnd}.tmp"));

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(unix_mode);
    }
    let _ = unix_mode;

    use std::io::Write;
    {
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("Error creando archivo temporal {}: {e}", tmp.display()))?;
        if let Err(e) = f.write_all(data.as_bytes()) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Error escribiendo {}: {e}", tmp.display()));
        }
    }

    if let Err(e) = std::fs::rename(&tmp, file) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Error guardando {}: {e}", file.display()));
    }
    Ok(())
}

pub fn read_project_file(root: &Path) -> Option<ProjectFile> {
    let raw = read_regular(&root.join(PROJECT_FILE))?;
    let parsed = serde_json::from_str::<ProjectFile>(&raw).ok()?;
    if parsed.space_id.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

pub fn write_project_file(root: &Path, pf: &ProjectFile) -> Result<(), String> {
    let json = serde_json::to_string_pretty(pf).map_err(|e| e.to_string())?;
    safe_write(&root.join(PROJECT_FILE), &(json + "\n"), 0o644)
}

pub fn read_project_token(root: &Path) -> Option<String> {
    let raw = read_regular(&root.join(CRED_DIR).join(CRED_FILE))?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn write_project_token(root: &Path, token: &str) -> Result<(), String> {
    let dir = root.join(CRED_DIR);
    if is_symlink(&dir) {
        return Err(format!(
            "{} es un enlace simbólico: por seguridad no se escribe a través de él.",
            dir.display()
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("Error creando .kyo: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    safe_write(&dir.join(CRED_FILE), &format!("{}\n", token.trim()), 0o600)
}

pub fn remove_project_files(root: &Path) {
    let _ = std::fs::remove_file(root.join(PROJECT_FILE));
    let _ = std::fs::remove_file(root.join(CRED_DIR).join(CRED_FILE));
}

pub fn ensure_gitignore(root: &Path) -> Result<bool, String> {
    let file = root.join(".gitignore");
    if is_symlink(&file) {
        return Err(format!(
            "{} es un enlace simbólico: por seguridad no se escribe a través de él.",
            file.display()
        ));
    }
    let content = read_regular(&file).unwrap_or_default();
    let already = content.lines().any(|l| {
        matches!(
            l.trim(),
            ".kyo" | ".kyo/" | "/.kyo" | "/.kyo/" | ".kyo/*"
        )
    });
    if already {
        return Ok(false);
    }
    let sep = if content.is_empty() || content.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let prefix = if content.is_empty() { "" } else { "\n" };
    let next = format!("{content}{sep}{prefix}# Kyō: token del vínculo (secreto)\n.kyo/\n");
    safe_write(&file, &next, 0o644)?;
    Ok(true)
}

pub fn credentials_tracked_by_git(root: &Path) -> bool {
    run_git(root, &["ls-files", "--error-unmatch", ".kyo/credentials"]).is_some()
}

// ── Contexto de Agentes, .mcp.json, StatusLine y Auto-Log Hook ───────────────

fn role_meta(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "space" => Some((
            "Espacio",
            "AQUÍ SE ESCRIBE la información del proyecto: documentación, decisiones, guía de uso, changelog. Tools: create_space_page / update_space_page.",
        )),
        "vault" => Some((
            "Bóveda",
            "AQUÍ SE ALMACENAN los archivos y datos del proyecto (specs, exports, assets, capturas). Nunca secretos ni .env reales.",
        )),
        "canvas" => Some((
            "Lienzo",
            "AQUÍ SE DIBUJA: diagramas Mermaid (arquitectura, flujos, secuencias), modelo de datos y otros nodos visuales. Tools: render_mermaid_to_canvas / er_model_to_canvas.",
        )),
        _ => None,
    }
}

pub fn render_agent_block(
    project_id: Option<&str>,
    title: Option<&str>,
    auto_log: bool,
    tools: &[WorkspaceTool],
) -> String {
    let kinds = ["space", "vault", "canvas"];
    let mut rows = Vec::new();
    let mut missing = Vec::new();

    for k in kinds {
        if let Some(tool) = tools.iter().find(|t| t.tool_type == k) {
            if let Some((label, use_desc)) = role_meta(k) {
                rows.push(format!(
                    "| {label} \"{}\" | `{}` | {use_desc} |",
                    plain_inline(&tool.title, 80),
                    safe_id(&tool.tool_id)
                ));
            }
        } else if let Some((label, _)) = role_meta(k) {
            missing.push(label);
        }
    }

    let proj_title = title
        .map(|t| plain_inline(t, 80))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "sin nombre".to_string());
    let pid_part = project_id
        .map(|id| format!(" (project_id `{}`)", safe_id(id)))
        .unwrap_or_default();

    let mut lines = vec![
        BEGIN_MARKER.to_string(),
        "## Grozy (Kyō): workspace de este proyecto".to_string(),
        String::new(),
        format!(
            "Este repositorio está vinculado al Proyecto \"{proj_title}\" de Grozy{pid_part}. \
             Trabaja en ese workspace por tu cuenta, sin esperar a que el usuario te lo pida, \
             con las tools del MCP `kyo` (si tu cliente lo llama distinto, es el mismo servidor Grozy)."
        ),
        String::new(),
    ];

    if !rows.is_empty() {
        lines.push("| Herramienta | id | Para qué |".to_string());
        lines.push("|---|---|---|".to_string());
        lines.extend(rows);
        lines.push(String::new());
    }
    if !missing.is_empty() {
        lines.push(format!(
            "Este proyecto no tiene: {}. Pregunta al usuario antes de crearlos (bootstrap_project_workspace con project_id).",
            missing.join(", ")
        ));
        lines.push(String::new());
    }

    lines.push("Reglas:".to_string());
    lines.push("- Al empezar una sesión, llama `get_project` con el project_id para ver el estado y las herramientas.".to_string());
    lines.push("- Tras un cambio relevante (arquitectura, modelo de datos, API, decisiones), actualiza la documentación en el Espacio y los diagramas en el Lienzo, sin esperar a que te lo pidan.".to_string());
    lines.push("- Si el usuario pide documentar, registrar/anotar cambios, un changelog, una decisión (ADR), una guía o \"dejar constancia\" de algo de este software, el destino es el Espacio de este proyecto (create_space_page / update_space_page), NO archivos .md del repo ni comentarios en el código, salvo que pida explícitamente un archivo del repo (README.md, docstrings). Antes de crear, revisa las páginas del Espacio y actualiza la que corresponda en vez de duplicar. Escapa `<` y `>` como `&lt;` `&gt;` y relee la página con get_space_page para verificar.".to_string());
    if auto_log {
        lines.push("- REGISTRO OBLIGATORIO: al terminar CUALQUIER implementación en este software (feature, fix, refactor, migración, configuración, dependencias) y antes de dar la tarea por cerrada, registra el cambio en el Espacio de este proyecto, sin que te lo pidan. Añade una entrada arriba de la página \"Changelog\" (créala si falta) con fecha, qué cambió, por qué, archivos clave y cómo verificarlo, redactada desde `git diff`/`git log` reales. Si hubo una decisión de diseño, agrégala también como página de decisión; si cambió la arquitectura o el modelo de datos, actualiza el Lienzo. No aplica a consultas, exploración ni cambios que sólo tocan documentación. Si el MCP `kyo` no responde, dilo al usuario en vez de omitir el registro en silencio.".to_string());
    }
    lines.push("- Los diagramas van al Lienzo como Mermaid (o ER), no como texto pegado en el Espacio.".to_string());
    lines.push("- Nunca guardes secretos, tokens ni `.env` reales en Grozy.".to_string());
    lines.push("- Si el MCP `kyo` no responde, avisa al usuario: probablemente falta iniciar sesión en Kyo Desktop o `kyo-cli login`.".to_string());
    lines.push(END_MARKER.to_string());

    lines.join("\n")
}

fn upsert_managed_block(content: Option<&str>, block: &str) -> String {
    let Some(c) = content else {
        return format!("{block}\n");
    };
    if let (Some(i), Some(j)) = (c.find("<!-- kyo:begin"), c.find(END_MARKER)) {
        if j > i {
            return format!("{}{}{}", &c[..i], block, &c[j + END_MARKER.len()..]);
        }
    }
    let sep = if c.is_empty() || c.ends_with("\n\n") {
        ""
    } else if c.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{c}{sep}{block}\n")
}

fn strip_managed_block(content: &str) -> Option<String> {
    let i = content.find("<!-- kyo:begin")?;
    let j = content.find(END_MARKER)?;
    if j < i {
        return None;
    }
    let before = content[..i].trim_end_matches('\n');
    let after = content[j + END_MARKER.len()..].trim_start_matches('\n');
    let joined = if before.is_empty() {
        after.to_string()
    } else if after.is_empty() {
        format!("{before}\n")
    } else {
        format!("{before}\n\n{after}")
    };
    Some(joined)
}

fn resolve_cli_command_prefix() -> String {
    if let Some(sidecar_bin) = find_sidecar_executable() {
        return sidecar_bin.to_string_lossy().to_string();
    }
    "kyo-cli".to_string()
}

fn sh_quote(s: &str) -> String {
    #[cfg(windows)]
    {
        format!("\"{}\"", s.replace('"', "\\\""))
    }
    #[cfg(not(windows))]
    {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn mcp_server_entry(mode: McpBridgeMode, server_url: &str) -> serde_json::Value {
    match mode {
        McpBridgeMode::NativeDesktop => {
            if let Some(sidecar_bin) = find_sidecar_executable() {
                serde_json::json!({
                    "command": sidecar_bin.to_string_lossy().to_string(),
                    "args": ["mcp"]
                })
            } else {
                serde_json::json!({
                    "command": "kyo-cli",
                    "args": ["mcp"]
                })
            }
        }
        McpBridgeMode::KyoCli => serde_json::json!({
            "command": "kyo-cli",
            "args": ["mcp"]
        }),
        McpBridgeMode::HttpDirect => serde_json::json!({
            "type": "http",
            "url": format!("{}/api/v1/mcp", server_url.trim_end_matches('/'))
        }),
    }
}

pub fn upsert_mcp_server(
    root: &Path,
    remove: bool,
    mode: McpBridgeMode,
    server_url: &str,
) -> Result<&'static str, String> {
    let file = root.join(MCP_FILE);
    if is_symlink(&file) {
        return Ok("skipped");
    }
    let raw = read_regular(&file);
    let mut obj: serde_json::Map<String, serde_json::Value> = match raw.as_deref() {
        Some(r) if !r.trim().is_empty() => match serde_json::from_str::<serde_json::Value>(r) {
            Ok(serde_json::Value::Object(m)) => m,
            _ => return Ok("skipped"),
        },
        _ => serde_json::Map::new(),
    };

    let mut servers = obj
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    if remove {
        if servers.remove(MCP_NAME).is_none() {
            return Ok("skipped");
        }
        if servers.is_empty() && obj.len() == 1 {
            let _ = std::fs::remove_file(&file);
        } else {
            obj.insert("mcpServers".to_string(), serde_json::Value::Object(servers));
            let next = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
                .map_err(|e| e.to_string())?
                + "\n";
            safe_write(&file, &next, 0o644)?;
        }
        return Ok("removed");
    }

    servers.insert(MCP_NAME.to_string(), mcp_server_entry(mode, server_url));
    obj.insert("mcpServers".to_string(), serde_json::Value::Object(servers));
    let next = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| e.to_string())?
        + "\n";
    if raw.as_deref() == Some(next.as_str()) {
        return Ok("skipped");
    }
    safe_write(&file, &next, 0o644)?;
    Ok("written")
}

fn ignore_local_claude_settings(root: &Path) {
    let file = root.join(".gitignore");
    if is_symlink(&file) {
        return;
    }
    let content = read_regular(&file).unwrap_or_default();
    let already = content.lines().any(|l| {
        matches!(
            l.trim(),
            ".claude/settings.local.json" | "/.claude/settings.local.json" | ".claude/*.local.json"
        )
    });
    if already {
        return;
    }
    let sep = if content.is_empty() || content.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let _ = safe_write(&file, &format!("{content}{sep}.claude/settings.local.json\n"), 0o644);
}

fn is_our_command(cmd: &str, needle: &str) -> bool {
    cmd.contains(needle)
}

pub fn install_status_line(root: &Path) -> Option<String> {
    let file = root.join(CLAUDE_SETTINGS_REL);
    if is_symlink(&file) || is_symlink(&root.join(".claude")) {
        return None;
    }
    let mut settings: serde_json::Map<String, serde_json::Value> = match read_regular(&file) {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw).ok()?,
        _ => serde_json::Map::new(),
    };
    if let Some(cur) = settings
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())
    {
        if is_our_command(cur, "statusline") && (cur.contains("kyo-cli") || cur.contains("go-server")) {
            return None;
        }
    }

    let prefix = resolve_cli_command_prefix();
    let ours = if prefix.contains(' ') {
        format!("{} statusline", sh_quote(&prefix))
    } else {
        format!("{prefix} statusline")
    };

    // Buscar statusLine ajeno (local > proyecto > usuario)
    let sources = [
        (root.join(CLAUDE_SETTINGS_REL), true),
        (root.join(".claude").join("settings.json"), false),
        (
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".claude")
                .join("settings.json"),
            false,
        ),
    ];
    let mut foreign_cmd: Option<String> = None;
    let mut foreign_padding: Option<i64> = None;
    for (src, from_local) in sources {
        if let Some(raw) = read_regular(&src) {
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(sl) = m.get("statusLine").and_then(|v| v.as_object()) {
                    if let Some(cmd) = sl.get("command").and_then(|c| c.as_str()) {
                        if !cmd.contains("kyo-cli statusline") && !cmd.contains("go-server") {
                            foreign_cmd = Some(cmd.to_string());
                            foreign_padding = sl.get("padding").and_then(|p| p.as_i64());
                            if from_local {
                                let _ = std::fs::create_dir_all(root.join(CRED_DIR));
                                let _ = safe_write(
                                    &root.join(PREV_STATUSLINE_REL),
                                    &(serde_json::to_string_pretty(sl).unwrap_or_default() + "\n"),
                                    0o600,
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    let final_cmd = match foreign_cmd {
        Some(f) => format!("{ours} --then {}", sh_quote(&f)),
        None => ours,
    };
    let mut sl_obj = serde_json::Map::new();
    sl_obj.insert("type".to_string(), serde_json::json!("command"));
    sl_obj.insert("command".to_string(), serde_json::json!(final_cmd));
    if let Some(p) = foreign_padding {
        sl_obj.insert("padding".to_string(), serde_json::json!(p));
    }
    settings.insert("statusLine".to_string(), serde_json::Value::Object(sl_obj));

    let _ = std::fs::create_dir_all(root.join(".claude"));
    let out = serde_json::to_string_pretty(&serde_json::Value::Object(settings)).ok()? + "\n";
    safe_write(&file, &out, 0o644).ok()?;
    ignore_local_claude_settings(root);
    Some(CLAUDE_SETTINGS_REL.to_string())
}

pub fn remove_status_line(root: &Path) -> Option<String> {
    let file = root.join(CLAUDE_SETTINGS_REL);
    if is_symlink(&file) {
        return None;
    }
    let raw = read_regular(&file)?;
    let mut settings: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).ok()?;
    let cmd = settings
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|c| c.as_str())?;
    if !cmd.contains("statusline") || (!cmd.contains("kyo-cli") && !cmd.contains("go-server")) {
        return None;
    }

    let prev_file = root.join(PREV_STATUSLINE_REL);
    if let Some(prev_raw) = read_regular(&prev_file) {
        if let Ok(prev_val) = serde_json::from_str::<serde_json::Value>(&prev_raw) {
            settings.insert("statusLine".to_string(), prev_val);
        } else {
            settings.remove("statusLine");
        }
    } else {
        settings.remove("statusLine");
    }
    let _ = std::fs::remove_file(&prev_file);

    if settings.is_empty() {
        let _ = std::fs::remove_file(&file);
    } else {
        let out = serde_json::to_string_pretty(&serde_json::Value::Object(settings)).ok()? + "\n";
        safe_write(&file, &out, 0o644).ok()?;
    }
    Some(CLAUDE_SETTINGS_REL.to_string())
}

pub fn install_auto_log_hook(root: &Path) -> Option<String> {
    let file = root.join(CLAUDE_SETTINGS_REL);
    if is_symlink(&file) || is_symlink(&root.join(".claude")) {
        return None;
    }
    let mut settings: serde_json::Map<String, serde_json::Value> = match read_regular(&file) {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str(&raw).ok()?,
        _ => serde_json::Map::new(),
    };
    let mut hooks = settings
        .get("hooks")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let mut stop_arr = hooks
        .get("Stop")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let already = stop_arr.iter().any(|g| {
        g.get("hooks")
            .and_then(|h| h.as_array())
            .map(|arr| {
                arr.iter().any(|item| {
                    item.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c.contains("hook stop"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });
    if already {
        return None;
    }

    let prefix = resolve_cli_command_prefix();
    let hook_cmd = if prefix.contains(' ') {
        format!("{} hook stop", sh_quote(&prefix))
    } else {
        format!("{prefix} hook stop")
    };
    stop_arr.push(serde_json::json!({
        "hooks": [{ "type": "command", "command": hook_cmd }]
    }));
    hooks.insert("Stop".to_string(), serde_json::Value::Array(stop_arr));
    settings.insert("hooks".to_string(), serde_json::Value::Object(hooks));

    let _ = std::fs::create_dir_all(root.join(".claude"));
    let out = serde_json::to_string_pretty(&serde_json::Value::Object(settings)).ok()? + "\n";
    safe_write(&file, &out, 0o644).ok()?;
    ignore_local_claude_settings(root);
    Some(CLAUDE_SETTINGS_REL.to_string())
}

pub fn remove_auto_log_hook(root: &Path) -> Option<String> {
    let file = root.join(CLAUDE_SETTINGS_REL);
    if is_symlink(&file) {
        return None;
    }
    let raw = read_regular(&file)?;
    let mut settings: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).ok()?;
    let mut hooks = settings.get("hooks")?.as_object()?.clone();
    let stop_arr = hooks.get("Stop")?.as_array()?.clone();

    let mut removed = false;
    let mut kept_groups = Vec::new();
    for g in stop_arr {
        if let Some(mut g_obj) = g.as_object().cloned() {
            if let Some(arr) = g_obj.get("hooks").and_then(|v| v.as_array()) {
                let kept: Vec<serde_json::Value> = arr
                    .iter()
                    .filter(|item| {
                        let ours = item
                            .get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c.contains("hook stop"))
                            .unwrap_or(false);
                        if ours {
                            removed = true;
                        }
                        !ours
                    })
                    .cloned()
                    .collect();
                if !kept.is_empty() {
                    g_obj.insert("hooks".to_string(), serde_json::Value::Array(kept));
                    kept_groups.push(serde_json::Value::Object(g_obj));
                }
            }
        }
    }
    if !removed {
        return None;
    }
    if kept_groups.is_empty() {
        hooks.remove("Stop");
    } else {
        hooks.insert("Stop".to_string(), serde_json::Value::Array(kept_groups));
    }
    if hooks.is_empty() {
        settings.remove("hooks");
    } else {
        settings.insert("hooks".to_string(), serde_json::Value::Object(hooks));
    }
    let _ = std::fs::remove_file(root.join(STATE_REL));
    if settings.is_empty() {
        let _ = std::fs::remove_file(&file);
    } else {
        let out = serde_json::to_string_pretty(&serde_json::Value::Object(settings)).ok()? + "\n";
        safe_write(&file, &out, 0o644).ok()?;
    }
    Some(CLAUDE_SETTINGS_REL.to_string())
}

pub fn write_agent_context(
    root: &Path,
    project_id: Option<&str>,
    title: Option<&str>,
    auto_log: bool,
    tools: &[WorkspaceTool],
    mcp_mode: McpBridgeMode,
    server_url: &str,
) -> Result<Vec<String>, String> {
    let block = render_agent_block(project_id, title, auto_log, tools);
    let mut touched = Vec::new();
    for name in CONTEXT_FILES {
        let file = root.join(name);
        if is_symlink(&file) {
            continue;
        }
        let current = read_regular(&file);
        let next = upsert_managed_block(current.as_deref(), &block);
        if current.as_deref() != Some(next.as_str()) {
            safe_write(&file, &next, 0o644)?;
            touched.push(name.to_string());
        }
    }
    if upsert_mcp_server(root, false, mcp_mode, server_url)? == "written" {
        touched.push(MCP_FILE.to_string());
    }
    Ok(touched)
}

pub fn remove_agent_context(root: &Path) -> Vec<String> {
    let mut touched = Vec::new();
    for name in CONTEXT_FILES {
        let file = root.join(name);
        if is_symlink(&file) {
            continue;
        }
        let Some(current) = read_regular(&file) else {
            continue;
        };
        let Some(next) = strip_managed_block(&current) else {
            continue;
        };
        if next.trim().is_empty() {
            let _ = std::fs::remove_file(&file);
        } else {
            let _ = safe_write(&file, &next, 0o644);
        }
        touched.push(name.to_string());
    }
    if upsert_mcp_server(root, true, McpBridgeMode::NativeDesktop, "").unwrap_or("skipped") == "removed" {
        touched.push(MCP_FILE.to_string());
    }
    touched
}

pub fn inspect_local_files(root: &Path) -> LocalFilesAudit {
    let claude_settings_raw = read_regular(&root.join(CLAUDE_SETTINGS_REL)).unwrap_or_default();
    let statusline_installed = claude_settings_raw.contains("statusline")
        && (claude_settings_raw.contains("kyo-cli") || claude_settings_raw.contains("go-server"));
    let auto_log_hook_installed = claude_settings_raw.contains("hook stop");
    LocalFilesAudit {
        kyo_project_json: read_project_file(root).is_some(),
        credentials_file: read_project_token(root).is_some(),
        credentials_tracked_in_git: credentials_tracked_by_git(root),
        claude_md: read_regular(&root.join("CLAUDE.md"))
            .map(|c| c.contains("<!-- kyo:begin"))
            .unwrap_or(false),
        agents_md: read_regular(&root.join("AGENTS.md"))
            .map(|c| c.contains("<!-- kyo:begin"))
            .unwrap_or(false),
        gemini_md: read_regular(&root.join("GEMINI.md"))
            .map(|c| c.contains("<!-- kyo:begin"))
            .unwrap_or(false),
        mcp_json: read_regular(&root.join(MCP_FILE))
            .and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok())
            .and_then(|v| v.get("mcpServers").and_then(|s| s.get(MCP_NAME)).cloned())
            .is_some(),
        statusline_installed,
        auto_log_hook_installed,
    }
}

// ── Registro de proyectos compartido (kyo/projects.json + kyo-cli/projects.json)

fn cli_registry_path() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("KYO_CONFIG_DIR") {
        if !d.trim().is_empty() {
            return Some(PathBuf::from(d).join("projects.json"));
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Some(PathBuf::from(appdata).join("kyo-cli").join("projects.json"));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("kyo-cli").join("projects.json"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".config").join("kyo-cli").join("projects.json"));
    }
    None
}

fn desktop_registry_path() -> Option<PathBuf> {
    get_kyo_data_dir().map(|d| d.join("projects.json"))
}

fn read_roots_from_file(path: &Path) -> Vec<String> {
    let Some(raw) = read_regular(path) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    val.get("roots")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter(|s| Path::new(s).is_absolute())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_registered_roots() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in [desktop_registry_path(), cli_registry_path()].into_iter().flatten() {
        for r in read_roots_from_file(&path) {
            if seen.insert(r.clone()) {
                out.push(r);
                if out.len() >= MAX_REGISTRY_ROOTS {
                    return out;
                }
            }
        }
    }
    out
}

fn canonical_str(root: &Path) -> String {
    std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn save_roots_everywhere(roots: &[String]) {
    let truncated: Vec<String> = roots.iter().take(MAX_REGISTRY_ROOTS).cloned().collect();
    let payload = serde_json::to_string_pretty(&serde_json::json!({ "roots": truncated }))
        .unwrap_or_default()
        + "\n";
    for path in [desktop_registry_path(), cli_registry_path()].into_iter().flatten() {
        let _ = safe_write(&path, &payload, 0o600);
    }
}

pub fn register_project_root(root: &Path) {
    let r = canonical_str(root);
    let mut roots = list_registered_roots();
    if roots.first().map(|f| f == &r).unwrap_or(false) {
        return;
    }
    roots.retain(|x| x != &r);
    roots.insert(0, r);
    save_roots_everywhere(&roots);
}

pub fn forget_project_root(root: &Path) {
    let c = canonical_str(root);
    let raw = root.to_string_lossy().to_string();
    let mut roots = list_registered_roots();
    roots.retain(|x| x != &c && x != &raw);
    save_roots_everywhere(&roots);
}

// ── Cliente API REST con Auto-Refresh OAuth ──────────────────────────────────

fn api_client_no_redirect() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {e}"))
}

async fn api_request_json<T: for<'de> Deserialize<'de>>(
    method: reqwest::Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, (u16, String, Option<String>)> {
    let (server_url, mut token) = ensure_fresh_account_token(false)
        .await
        .map_err(|e| (401, e, None))?;
    let client = api_client_no_redirect().map_err(|e| (0, e, None))?;

    let send_once = |tok: &str| {
        let url = format!("{server_url}/api/v1{path}");
        let mut req = client
            .request(method.clone(), &url)
            .header("Authorization", format!("Bearer {tok}"))
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT);
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send()
    };

    let mut res = send_once(&token)
        .await
        .map_err(|e| (0, format!("No se pudo conectar con {server_url}: {e}"), None))?;

    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        if let Ok((_, fresh_tok)) = ensure_fresh_account_token(true).await {
            if fresh_tok != token {
                token = fresh_tok;
                res = send_once(&token)
                    .await
                    .map_err(|e| (0, format!("No se pudo conectar con {server_url}: {e}"), None))?;
            }
        }
    }

    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    if status < 200 || status >= 300 {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let code = parsed
            .get("code")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let msg = parsed
            .get("error_description")
            .or_else(|| parsed.get("error"))
            .or_else(|| parsed.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| clean_text(s, 400))
            .unwrap_or_else(|| {
                if status == 401 {
                    "La sesión venció o fue revocada. Inicia sesión nuevamente en Kyō.".to_string()
                } else {
                    format!("HTTP {status}")
                }
            });
        return Err((status, msg, code));
    }

    serde_json::from_str::<T>(&text)
        .map_err(|e| (status, format!("Respuesta inesperada del servidor: {e}"), None))
}

// ── Operaciones de vínculo de Proyecto (equivalente a project-core.ts) ──────

#[derive(Clone, Debug)]
pub struct LinkOptions {
    pub name: Option<String>,
    pub project_id: Option<String>,
    pub template_space_id: Option<String>,
    pub no_tools: bool,
    pub force: bool,
    pub agent_files: bool,
    pub status_line: bool,
    pub auto_log: bool,
    pub mcp_mode: McpBridgeMode,
}

impl Default for LinkOptions {
    fn default() -> Self {
        Self {
            name: None,
            project_id: None,
            template_space_id: None,
            no_tools: false,
            force: false,
            agent_files: true,
            status_line: true,
            auto_log: false,
            mcp_mode: McpBridgeMode::NativeDesktop,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LinkOutcome {
    pub link: LinkInfo,
    pub gitignore_updated: bool,
    pub credentials_tracked: bool,
    pub agent_files: Vec<String>,
}

pub fn days_left(expires_at: Option<&str>) -> Option<i64> {
    let exp_sec = parse_rfc3339_unix(expires_at?)?;
    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = exp_sec - now_sec;
    if diff < 0 {
        Some(-1)
    } else {
        Some((diff + 86399) / 86400)
    }
}

fn sync_agent_files_internal(
    root: &Path,
    link: &LinkInfo,
    enabled: bool,
    status_line: bool,
    mcp_mode: McpBridgeMode,
    server_url: &str,
) -> Result<Vec<String>, String> {
    if !enabled {
        return Ok(Vec::new());
    }
    let auto_log = read_project_file(root)
        .and_then(|f| f.auto_log)
        .unwrap_or(false);
    let tools = link
        .workspace
        .as_ref()
        .map(|w| w.tools.as_slice())
        .unwrap_or(&[]);
    let mut touched = write_agent_context(
        root,
        link.project_id.as_deref(),
        link.workspace.as_ref().map(|w| w.title.as_str()),
        auto_log,
        tools,
        mcp_mode,
        server_url,
    )?;
    if auto_log {
        if let Some(f) = install_auto_log_hook(root) {
            if !touched.contains(&f) {
                touched.push(f);
            }
        }
    }
    if status_line {
        if let Some(f) = install_status_line(root) {
            touched.push(f);
        }
    }
    Ok(touched)
}

fn write_local_link_files(
    root: &Path,
    server_url: &str,
    link: &LinkInfo,
    auto_log: Option<bool>,
) -> Result<(), String> {
    let token = link
        .token
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "El servidor no devolvió el token del vínculo.".to_string())?;

    let prev_auto = read_project_file(root).and_then(|f| f.auto_log);
    let resolved_auto = match auto_log.or(prev_auto) {
        Some(true) => Some(true),
        _ => None,
    };
    let pf = ProjectFile {
        space_id: link.space_id.clone(),
        project_id: link.project_id.clone().filter(|s| !s.is_empty()),
        template_space_id: link.template_space_id.clone(),
        template_version: link.template_version.clone(),
        linked_at: link.linked_at.clone(),
        server_url: Some(server_url.to_string()),
        auto_log: resolved_auto,
    };
    write_project_file(root, &pf)?;
    write_project_token(root, token)?;
    Ok(())
}

pub async fn list_remote_projects() -> Result<Vec<ProjectSummary>, String> {
    let list: Vec<ProjectSummary> = api_request_json(reqwest::Method::GET, "/projects", None)
        .await
        .map_err(|(_, msg, _)| msg)?;
    Ok(list
        .into_iter()
        .filter(|p| p.status.as_deref() != Some("archivado"))
        .collect())
}

pub async fn list_remote_spaces() -> Result<Vec<SpaceSummary>, String> {
    let val: serde_json::Value = api_request_json(reqwest::Method::GET, "/spaces", None)
        .await
        .map_err(|(_, msg, _)| msg)?;
    let arr = val
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| val.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(arr
        .into_iter()
        .filter_map(|v| serde_json::from_value::<SpaceSummary>(v).ok())
        .collect())
}

pub async fn link_project(dir: &Path, opts: LinkOptions) -> Result<LinkOutcome, String> {
    if opts.project_id.is_some() && (opts.name.is_some() || opts.no_tools) {
        return Err("Con un proyecto existente no se usan nombre ni --no-tools.".to_string());
    }
    let root = find_project_root(dir);
    if read_project_file(&root).is_some() && !opts.force {
        return Err("Esta carpeta ya está vinculada a un proyecto. Activa 'Revincular (--force)' si deseas reemplazar el vínculo.".to_string());
    }

    let r = project_ref(&root);
    let loc = location_info(&root);
    let (server_url, _) = ensure_fresh_account_token(false).await?;

    let mut payload = serde_json::Map::new();
    if let Some(ref gr) = r.git_remote {
        payload.insert("git_remote".to_string(), serde_json::json!(gr));
    }
    if let Some(ref ph) = r.path_hash {
        payload.insert("path_hash".to_string(), serde_json::json!(ph));
    }
    if let Some(ref pid) = opts.project_id {
        payload.insert("project_id".to_string(), serde_json::json!(pid));
    } else {
        let chosen_name = opts
            .name
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| project_name(&root, &r));
        payload.insert("name".to_string(), serde_json::json!(chosen_name));
        payload.insert("no_tools".to_string(), serde_json::json!(opts.no_tools));
        if let Some(ref tpl) = opts.template_space_id.as_deref().filter(|s| !s.trim().is_empty()) {
            payload.insert("template_space_id".to_string(), serde_json::json!(tpl));
        } else {
            payload.insert("blank".to_string(), serde_json::json!(!opts.no_tools));
        }
    }
    payload.insert("force".to_string(), serde_json::json!(opts.force));
    payload.insert("location".to_string(), serde_json::json!(loc));

    let link: LinkInfo = api_request_json(
        reqwest::Method::POST,
        "/project-links",
        Some(&serde_json::Value::Object(payload)),
    )
    .await
    .map_err(|(_, msg, code)| match code.as_deref() {
        Some("storage_required") => format!(
            "{msg}\n→ Configura almacenamiento (hosting Kyō, Drive u OneDrive) en {server_url}/boveda"
        ),
        Some("project_name_taken") => {
            format!("{msg}\n→ Usa otro nombre o vincula la carpeta a ese proyecto existente.")
        }
        _ => msg,
    })?;

    write_local_link_files(&root, &server_url, &link, opts.auto_log.then_some(true))?;
    let gitignore_updated = ensure_gitignore(&root)?;
    let agent_files = sync_agent_files_internal(
        &root,
        &link,
        opts.agent_files,
        opts.status_line,
        opts.mcp_mode,
        &server_url,
    )?;
    register_project_root(&root);

    Ok(LinkOutcome {
        link,
        gitignore_updated,
        credentials_tracked: credentials_tracked_by_git(&root),
        agent_files,
    })
}

pub async fn sync_project(
    root: &Path,
    agent_files: bool,
    status_line: bool,
    mcp_mode: McpBridgeMode,
) -> Result<(LinkInfo, Vec<String>), String> {
    let r = project_ref(root);
    let loc = location_info(root);
    let (server_url, _) = ensure_fresh_account_token(false).await?;
    let mut payload = serde_json::Map::new();
    if let Some(ref gr) = r.git_remote {
        payload.insert("git_remote".to_string(), serde_json::json!(gr));
    }
    if let Some(ref ph) = r.path_hash {
        payload.insert("path_hash".to_string(), serde_json::json!(ph));
    }
    payload.insert("location".to_string(), serde_json::json!(loc));

    let link: LinkInfo = api_request_json(
        reqwest::Method::POST,
        "/project-links/sync",
        Some(&serde_json::Value::Object(payload)),
    )
    .await
    .map_err(|(code, msg, _)| {
        if code == 404 {
            "Esta carpeta no está vinculada en el servidor.".to_string()
        } else {
            msg
        }
    })?;

    let touched = sync_agent_files_internal(root, &link, agent_files, status_line, mcp_mode, &server_url)?;
    register_project_root(root);
    Ok((link, touched))
}

pub async fn renew_project(root: &Path) -> Result<(LinkInfo, bool), String> {
    let r = project_ref(root);
    let loc = location_info(root);
    let (server_url, _) = ensure_fresh_account_token(false).await?;
    let mut payload = serde_json::Map::new();
    if let Some(ref gr) = r.git_remote {
        payload.insert("git_remote".to_string(), serde_json::json!(gr));
    }
    if let Some(ref ph) = r.path_hash {
        payload.insert("path_hash".to_string(), serde_json::json!(ph));
    }
    payload.insert("location".to_string(), serde_json::json!(loc));

    let link: LinkInfo = api_request_json(
        reqwest::Method::POST,
        "/project-links/renew",
        Some(&serde_json::Value::Object(payload)),
    )
    .await
    .map_err(|(_, msg, _)| msg)?;

    write_local_link_files(root, &server_url, &link, None)?;
    let _ = ensure_gitignore(root);
    register_project_root(root);
    Ok((link, credentials_tracked_by_git(root)))
}

pub async fn unlink_project(root: &Path) -> Result<Vec<String>, String> {
    let r = project_ref(root);
    let body = serde_json::to_value(&r).map_err(|e| e.to_string())?;
    let _: serde_json::Value = api_request_json(reqwest::Method::POST, "/project-links/unlink", Some(&body))
        .await
        .map_err(|(_, msg, _)| msg)?;

    remove_project_files(root);
    forget_project_root(root);
    let mut touched = remove_agent_context(root);
    if let Some(sl) = remove_status_line(root) {
        touched.push(sl);
    }
    if let Some(hk) = remove_auto_log_hook(root) {
        if !touched.contains(&hk) {
            touched.push(hk);
        }
    }
    Ok(touched)
}

pub async fn set_auto_log(root: &Path, enabled: bool, mcp_mode: McpBridgeMode) -> Result<(), String> {
    let mut file = read_project_file(root)
        .ok_or_else(|| "Esta carpeta no está vinculada. Vincúlala primero.".to_string())?;
    file.auto_log = if enabled { Some(true) } else { None };
    write_project_file(root, &file)?;
    if !enabled {
        let _ = remove_auto_log_hook(root);
    }
    let _ = sync_project(root, true, true, mcp_mode).await?;
    Ok(())
}

pub async fn fetch_project_status(root: &Path) -> Result<StatusResponse, (u16, String)> {
    let r = project_ref(root);
    let mut qs = Vec::new();
    if let Some(ref gr) = r.git_remote {
        qs.push(format!("git_remote={}", url_encode_qs(gr)));
    }
    if let Some(ref ph) = r.path_hash {
        qs.push(format!("path_hash={}", url_encode_qs(ph)));
    }
    let path = format!("/project-links/status?{}", qs.join("&"));
    api_request_json(reqwest::Method::GET, &path, None)
        .await
        .map_err(|(code, msg, _)| (code, msg))
}

fn url_encode_qs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Carga el estado local + remoto de una carpeta vinculada para el panel de Proyectos.
pub async fn load_project_row(root_str: &str, has_session: bool) -> ProjectRow {
    let root = PathBuf::from(root_str);
    let fallback_name = root
        .file_name()
        .map(|n| clean_text(&n.to_string_lossy(), 80))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| clean_text(root_str, 80));

    let empty_audit = LocalFilesAudit {
        kyo_project_json: false,
        credentials_file: false,
        credentials_tracked_in_git: false,
        claude_md: false,
        agents_md: false,
        gemini_md: false,
        mcp_json: false,
        statusline_installed: false,
        auto_log_hook_installed: false,
    };

    if is_symlink(&root) || !root.exists() {
        return ProjectRow {
            root: root_str.to_string(),
            name: fallback_name,
            branch: String::new(),
            hostname: String::new(),
            state: ProjectState::Absent,
            auto_log: false,
            has_token: false,
            expires_at: None,
            days: None,
            workspace: None,
            space_id: None,
            project_id: None,
            remotes: Vec::new(),
            warnings: Vec::new(),
            error: None,
            audit: empty_audit,
        };
    }

    let audit = inspect_local_files(&root);
    let Some(file) = read_project_file(&root) else {
        return ProjectRow {
            root: root_str.to_string(),
            name: fallback_name,
            branch: current_branch(&root),
            hostname: get_hostname(),
            state: ProjectState::Unlinked,
            auto_log: false,
            has_token: audit.credentials_file,
            expires_at: None,
            days: None,
            workspace: None,
            space_id: None,
            project_id: None,
            remotes: Vec::new(),
            warnings: Vec::new(),
            error: None,
            audit,
        };
    };

    let loc = location_info(&root);
    let remotes: Vec<String> = loc
        .remotes
        .iter()
        .map(|r| clean_text(&format!("{} {}", r.name, r.url), 200))
        .collect();
    let auto_log = file.auto_log == Some(true);
    let has_token = audit.credentials_file;

    if !has_session {
        return ProjectRow {
            root: root_str.to_string(),
            name: fallback_name,
            branch: clean_text(&loc.branch, 80),
            hostname: clean_text(&loc.hostname, 80),
            state: ProjectState::NoSession,
            auto_log,
            has_token,
            expires_at: None,
            days: None,
            workspace: None,
            space_id: Some(file.space_id),
            project_id: file.project_id,
            remotes,
            warnings: Vec::new(),
            error: None,
            audit,
        };
    }

    match fetch_project_status(&root).await {
        Ok(remote) => {
            let ws = remote.workspace.clone();
            let title = ws
                .as_ref()
                .map(|w| w.title.as_str())
                .filter(|t| !t.trim().is_empty())
                .or(remote.space_name.as_deref())
                .unwrap_or(&fallback_name);
            let d = if remote.expired == Some(true) {
                Some(-1)
            } else {
                days_left(remote.expires_at.as_deref())
            };
            let mut warns: Vec<String> = remote
                .warnings
                .unwrap_or_default()
                .into_iter()
                .map(|w| clean_text(&w, 200))
                .collect();
            if audit.credentials_tracked_in_git {
                warns.push(".kyo/credentials está versionado en git: sácalo con `git rm --cached .kyo/credentials` y renueva el token.".to_string());
            }
            if !has_token {
                warns.push("Falta .kyo/credentials en esta máquina: usa Renovar token.".to_string());
            }
            ProjectRow {
                root: root_str.to_string(),
                name: clean_text(title, 80),
                branch: clean_text(&loc.branch, 80),
                hostname: clean_text(&loc.hostname, 80),
                state: if remote.linked {
                    ProjectState::Ok
                } else {
                    ProjectState::Unlinked
                },
                auto_log,
                has_token,
                expires_at: remote.expires_at,
                days: d,
                workspace: ws,
                space_id: remote.space_id.or(Some(file.space_id)),
                project_id: remote.project_id.or(file.project_id),
                remotes,
                warnings: warns,
                error: None,
                audit,
            }
        }
        Err((_, err_msg)) => ProjectRow {
            root: root_str.to_string(),
            name: fallback_name,
            branch: clean_text(&loc.branch, 80),
            hostname: clean_text(&loc.hostname, 80),
            state: ProjectState::Error,
            auto_log,
            has_token,
            expires_at: None,
            days: None,
            workspace: None,
            space_id: Some(file.space_id),
            project_id: file.project_id,
            remotes,
            warnings: Vec::new(),
            error: Some(clean_text(&err_msg, 200)),
            audit,
        },
    }
}

// ── Cliente Streamable HTTP MCP para listar Tools MCP activas ────────────────

pub fn domain_of_tool(tool_name: &str) -> &'static str {
    let n = tool_name;
    if n.contains("_er_")
        || n.contains("_uml_")
        || n.contains("_dfd_")
        || n.contains("_state_")
        || n.contains("datamodel")
        || n.contains("mermaid")
        || n.starts_with("er_")
        || n.starts_with("uml_")
        || n.starts_with("dfd_")
    {
        return "Diagramas";
    }
    if n.contains("space") {
        return "Espacio";
    }
    if n.contains("canvas") {
        return "Lienzo";
    }
    if n.contains("vault") {
        return "Bóveda";
    }
    if n.contains("apunte") {
        return "Apuntes";
    }
    if n.contains("book") {
        return "Libros";
    }
    if n.contains("task") || n.contains("routine") || n.contains("schedule") {
        return "Tareas";
    }
    if n.contains("semester") || n.contains("subject") || n.contains("attendance") {
        return "Académico";
    }
    if n.contains("tag") {
        return "Etiquetas";
    }
    if n.contains("graph") || n.contains("entity") || n.contains("search") || n.contains("reindex") {
        return "Grafo y búsqueda";
    }
    if n.contains("project") {
        return "Proyecto";
    }
    if n.contains("avatar") {
        return "Avatar";
    }
    if n.contains("hosting") {
        return "Hosting";
    }
    "Otras"
}

pub fn is_dangerous_tool(name: &str) -> bool {
    name.starts_with("delete_")
        || name.starts_with("merge_")
        || name.starts_with("restructure_")
        || name.starts_with("archive_")
}

fn extract_mcp_json_from_response(content_type: &str, body: &str) -> String {
    if content_type.to_lowercase().contains("text/event-stream")
        || body.trim_start().starts_with("event:")
        || body.trim_start().starts_with("data:")
    {
        let mut last = None;
        for line in body.lines() {
            let trimmed = line.trim();
            if let Some(d) = trimmed.strip_prefix("data:") {
                let d = d.trim();
                if !d.is_empty() && d != "[DONE]" {
                    last = Some(d.to_string());
                }
            }
        }
        if let Some(l) = last {
            return l;
        }
    }
    body.to_string()
}

pub async fn fetch_mcp_tools() -> Result<Vec<McpToolInfo>, String> {
    let (server_url, token) = ensure_fresh_account_token(false).await?;
    let endpoint = format!("{server_url}/api/v1/mcp");
    let client = api_client_no_redirect()?;

    // 1. Initialize handshake
    let init_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "kyo-desktop", "version": "0.1.0" }
        }
    });
    let init_res = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json, text/event-stream")
        .header("User-Agent", USER_AGENT)
        .json(&init_body)
        .send()
        .await
        .map_err(|e| format!("No se pudo conectar con {endpoint}: {e}"))?;

    if !init_res.status().is_success() {
        return Err(format!("MCP respondió HTTP {}", init_res.status().as_u16()));
    }
    let session_id = init_res
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // 2. Paginated tools/list
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for page in 0..20 {
        let params = match cursor {
            Some(ref c) => serde_json::json!({ "cursor": c }),
            None => serde_json::json!({}),
        };
        let req_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": page + 2,
            "method": "tools/list",
            "params": params
        });
        let mut req = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json, text/event-stream")
            .header("User-Agent", USER_AGENT)
            .json(&req_body);
        if let Some(ref sid) = session_id {
            req = req.header("Mcp-Session-Id", sid);
        }
        let res = req.send().await.map_err(|e| format!("Error listando tools MCP: {e}"))?;
        let ct = res
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = res.text().await.unwrap_or_default();
        let json_str = extract_mcp_json_from_response(&ct, &text);
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|e| format!("Respuesta MCP inválida: {e}"))?;
        let result = parsed.get("result").unwrap_or(&parsed);

        if let Some(arr) = result.get("tools").and_then(|t| t.as_array()) {
            for t in arr {
                let name = t
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| clean_text(s, 80))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let desc = t
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| clean_text(s, 600))
                    .unwrap_or_default();
                out.push(McpToolInfo {
                    domain: domain_of_tool(&name).to_string(),
                    dangerous: is_dangerous_tool(&name),
                    name,
                    description: desc,
                });
            }
        }
        cursor = result
            .get("nextCursor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        if cursor.is_none() {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_credentials() {
        assert_eq!(
            strip_credentials("https://user:ghp_secret@github.com/Grivyzom/KyoDesktop.git"),
            "https://github.com/Grivyzom/KyoDesktop.git"
        );
        assert_eq!(
            strip_credentials("git@github.com:Grivyzom/KyoDesktop.git"),
            "git@github.com:Grivyzom/KyoDesktop.git"
        );
    }

    #[test]
    fn test_parse_origin_and_remotes() {
        let cfg = r#"[core]
	bare = false
[remote "upstream"]
	url = https://tok:x@gitlab.com/a/b.git
[remote "origin"]
	url = git@github.com:Grivyzom/KyoDesktop.git
"#;
        assert_eq!(
            parse_origin_from_config(cfg).as_deref(),
            Some("git@github.com:Grivyzom/KyoDesktop.git")
        );
        let remotes = parse_remotes_from_config(cfg);
        assert_eq!(remotes.len(), 2);
        assert_eq!(remotes[0].url, "https://gitlab.com/a/b.git");
    }

    #[test]
    fn test_agent_block_upsert_and_strip() {
        let tools = vec![
            WorkspaceTool {
                row_id: "1".into(),
                tool_type: "space".into(),
                tool_id: "sp_123".into(),
                title: "Docs `App`".into(),
                url: "".into(),
            },
            WorkspaceTool {
                row_id: "2".into(),
                tool_type: "vault".into(),
                tool_id: "vt_456".into(),
                title: "Vault".into(),
                url: "".into(),
            },
        ];
        let block = render_agent_block(Some("proj_1"), Some("Mi App"), true, &tools);
        assert!(block.contains("REGISTRO OBLIGATORIO"));
        assert!(block.contains("sp_123"));
        assert!(block.contains("Lienzo")); // missing canvas warning

        let existing = "# Mi Proyecto\n\nContenido propio.\n";
        let updated = upsert_managed_block(Some(existing), &block);
        assert!(updated.starts_with("# Mi Proyecto"));
        assert!(updated.contains("<!-- kyo:begin"));

        let stripped = strip_managed_block(&updated).unwrap();
        assert_eq!(stripped.trim(), "# Mi Proyecto\n\nContenido propio.");
    }

    #[test]
    fn test_ensure_gitignore_and_safe_write() {
        let base = std::env::temp_dir().join(format!("kyo_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        assert!(ensure_gitignore(&base).unwrap());
        assert!(!ensure_gitignore(&base).unwrap()); // idempotent
        let gi = std::fs::read_to_string(base.join(".gitignore")).unwrap();
        assert!(gi.contains(".kyo/"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_domain_and_dangerous() {
        assert_eq!(domain_of_tool("create_space_page"), "Espacio");
        assert_eq!(domain_of_tool("render_mermaid_to_canvas"), "Diagramas");
        assert_eq!(domain_of_tool("upload_vault_file"), "Bóveda");
        assert!(is_dangerous_tool("delete_space_page"));
        assert!(!is_dangerous_tool("get_space_page"));
    }
}
