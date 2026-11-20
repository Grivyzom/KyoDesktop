//! Vista de Bóveda (Vault View) para explorar y gestionar el contenido de una bóveda en la app desktop.
//!
//! Permite al usuario:
//! - Visualizar métricas y metadatos de la bóveda (archivos, tamaño, carpetas, proveedor, estado).
//! - Alternar entre diferentes bóvedas vinculadas a su cuenta.
//! - Navegar por la estructura jerárquica de carpetas con migas de pan (breadcrumbs).
//! - Buscar y filtrar archivos en tiempo real por texto o categoría (documentos, imágenes, hojas, código, otros).
//! - Alternar entre vista en cuadrícula (grid) y vista en lista detallada.
//! - Inspeccionar archivos en un modal emergente con metadatos completos y visor de contenido de texto.
//! - Descargar archivos directamente a la carpeta Descargas de la máquina.
//! - Usar la bóveda o archivos específicos como contexto para iniciar una conversación con el agente de IA.

use dioxus::prelude::*;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::models::{ActiveView, IsSettingsOpen};
use crate::transfer_progress::{
    TransferDirection, TransferProgress, TransferProgressBar, TransferStatus,
};
use crate::dashboard::{
    use_kyo_cloud, CloudResource, CloudResourceKind, Icon,
    ICON_ALERT, ICON_CHAT, ICON_CHECK,
    ICON_EXTERNAL, ICON_FOLDER, ICON_GRID, ICON_REFRESH, ICON_VAULT, ICON_X,
    open_in_browser,
};
use crate::chat::resolve_jwt_token;
use crate::settings::{
    load_server_url, load_active_profile_name,
    load_downloads_dir, save_downloads_dir, get_default_os_downloads_dir, SettingsTab,
};

// ── Iconos SVG adicionales para el explorador ────────────────────────────────
pub(crate) const ICON_FILE_TEXT: &str = "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M16 13H8 M16 17H8 M10 9H8";
pub(crate) const ICON_IMAGE: &str = "M21 19V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2z M8.5 10a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z M21 15l-5-5L5 21";
pub(crate) const ICON_CODE: &str = "M16 18l6-6-6-6 M8 6l-6 6 6 6";
pub(crate) const ICON_SHEET: &str = "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M8 13h8 M8 17h8 M12 9v12";
pub(crate) const ICON_FILE: &str = "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6";
pub(crate) const ICON_DOWNLOAD: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3";
pub(crate) const ICON_SEARCH: &str = "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z M21 21l-4.35-4.35";
pub(crate) const ICON_LIST: &str = "M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01";
pub(crate) const ICON_ARROW_LEFT: &str = "M19 12H5 M12 19l-7-7 7-7";
pub(crate) const ICON_PIN: &str = "M12 17v5 M9 2h6l1 7h-8z M5 9h14";
pub(crate) const ICON_PLUS: &str = "M12 5v14 M5 12h14";
pub(crate) const ICON_HARD_DRIVE: &str = "M22 12H2 M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z M6 16h.01 M10 16h.01";
pub(crate) const ICON_SPARKLES: &str = "M12 3l1.9 5.8a2 2 0 0 0 1.3 1.3L21 12l-5.8 1.9a2 2 0 0 0-1.3 1.3L12 21l-1.9-5.8a2 2 0 0 0-1.3-1.3L3 12l5.8-1.9a2 2 0 0 0 1.3-1.3L12 3z";
pub(crate) const ICON_TRASH: &str = "M3 6h18 M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2";
pub(crate) const ICON_UPLOAD: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M17 8l-5-5-5 5 M12 3v12";

// ── Modelos de datos de la API de Bóvedas y Bóvedas Locales ──────────────────

/// Registro persistente de una bóveda creada localmente en el equipo del usuario (`%APPDATA%/kyo/local_vaults.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalVaultRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub pinned_files: Vec<String>,
    #[serde(default)]
    pub file_tags: std::collections::HashMap<String, Vec<String>>,
}

/// Estado resumido del Modo Bóveda Local para Web y Desktop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalVaultModeStatus {
    /// Solo es `true` cuando el usuario tiene creada al menos una bóveda en modo local.
    pub has_local_vault: bool,
    pub local_vault_mode_enabled: bool,
    pub local_vaults_count: usize,
    pub total_files: usize,
    pub total_bytes: u64,
}

/// Información de un disco o unidad montada en el sistema donde el usuario puede alojar bóvedas locales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiskDriveInfo {
    /// Punto de montaje raíz (por ejemplo `"C:\\"` o `"V:\\"`).
    pub mount_point: String,
    /// Código corto del disco (por ejemplo `"C:"` o `"V:"`).
    pub code: String,
    /// Etiqueta del volumen o descripción legible.
    pub label: String,
    /// Espacio libre disponible en bytes.
    pub free_bytes: u64,
    /// Capacidad total del disco en bytes.
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverviewSourceFilter {
    All,
    LocalOnly,
    CloudOnly,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultKindStat {
    pub kind: String,
    pub count: usize,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub provider: Option<String>,
    pub storage_account_name: Option<String>,
    pub storage_account_email: Option<String>,
    pub status: Option<String>,
    pub file_count: Option<usize>,
    pub vault_size_bytes: Option<u64>,
    pub size_bytes: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub kinds: Option<Vec<VaultKindStat>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultFolder {
    pub id: String,
    pub vault_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub position: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultFile {
    pub id: String,
    pub vault_id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub ext: String,
    pub kind: String,
    pub size: u64,
    pub hash: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VaultPageMode {
    Overview,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverviewSortMode {
    Recent,
    SizeDesc,
    FilesDesc,
    NameAsc,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortMode {
    Recent,
    NameAsc,
    NameDesc,
    SizeDesc,
    SizeAsc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadToast {
    pub message: String,
    pub path: Option<PathBuf>,
    pub is_error: bool,
}

// ── Funciones utilitarias ───────────────────────────────────────────────────

/// Formatea bytes en una representación legible (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Sanitiza un nombre de archivo para evitar vulnerabilidades de path traversal y caracteres inválidos en el SO.
pub fn sanitize_filename_for_os(raw_name: &str) -> String {
    let name = raw_name.trim();
    let base = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name);

    let mut cleaned: String = base
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    cleaned = cleaned.trim_matches(&[' ', '.'] as &[_]).to_string();

    let stem = std::path::Path::new(&cleaned)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_uppercase();

    let reserved = [
        "CON", "PRN", "AUX", "NUL",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    if reserved.contains(&stem.as_str()) {
        cleaned = format!("_{cleaned}");
    }

    if cleaned.is_empty() {
        "archivo_descargado".to_string()
    } else {
        cleaned
    }
}

/// Formatea una fecha ISO 8601 a un formato legible corto de forma segura contra UTF-8.
pub fn format_short_date(iso_str: &str) -> String {
    let trimmed = iso_str.trim();
    if trimmed.is_empty() {
        return "-".to_string();
    }
    if let Some((date_part, rest)) = trimmed.split_once('T') {
        let time_part = rest.split(['Z', '+', '-']).next().unwrap_or(rest);
        let time_short: String = time_part.chars().take(5).collect();
        if !time_short.is_empty() {
            format!("{date_part} {time_short}")
        } else {
            date_part.to_string()
        }
    } else {
        trimmed.chars().take(10).collect()
    }
}

/// Clasifica un archivo y devuelve (icono, clase_color, nombre_tipo).
pub fn get_file_kind_meta(kind: &str, ext: &str) -> (&'static str, &'static str, &'static str) {
    let lower_ext = ext.to_lowercase();
    let lower_kind = kind.to_lowercase();

    if lower_kind == "image" || matches!(lower_ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp") {
        (ICON_IMAGE, "vk-file-icon-image", "Imagen")
    } else if lower_kind == "sheet" || matches!(lower_ext.as_str(), "xlsx" | "xls" | "csv" | "tsv" | "ods") {
        (ICON_SHEET, "vk-file-icon-sheet", "Hoja de cálculo")
    } else if lower_kind == "code" || matches!(lower_ext.as_str(), "rs" | "go" | "ts" | "js" | "py" | "html" | "css" | "json" | "yaml" | "yml" | "toml" | "sql" | "sh" | "bat" | "ps1") {
        (ICON_CODE, "vk-file-icon-code", "Código")
    } else if lower_kind == "doc" || matches!(lower_ext.as_str(), "doc" | "docx" | "pdf" | "txt" | "md" | "rtf" | "odt") {
        (ICON_FILE_TEXT, "vk-file-icon-doc", "Documento")
    } else {
        (ICON_FILE, "vk-file-icon-other", "Archivo")
    }
}

/// Determina si un archivo es de texto plano y se puede previsualizar en el modal.
pub fn is_previewable_text(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "txt" | "md" | "markdown" | "json" | "js" | "jsx" | "ts" | "tsx" | "rs" | "go" | "py" |
        "css" | "html" | "yaml" | "yml" | "toml" | "xml" | "sql" | "log" | "csv" | "env" | "ini" |
        "sh" | "bat" | "ps1" | "vue" | "svelte" | "graphql" | "svg" | "conf" | "cfg"
    )
}

/// Abre el archivo o directorio en el Explorador de archivos del sistema operativo.
fn open_path_in_explorer(path: &std::path::Path) {
    crate::settings::open_path_in_explorer(path);
}

// ── Servicios HTTP de Bóveda ────────────────────────────────────────────────

/// Resuelve la URL base de la API de Kyō Cloud (remota o configurada en ajustes/session.json).
/// Los recursos de bóvedas, carpetas y descargas residen siempre en la plataforma Cloud.
pub fn resolve_cloud_api_base() -> String {
    let base = load_server_url();
    let clean = base.trim().trim_end_matches('/');
    if clean.ends_with("/api/v1") {
        clean.to_string()
    } else {
        format!("{clean}/api/v1")
    }
}

/// Parsea un objeto JSON de bóveda en `VaultDetail` (reutilizado por listado y detalle).
fn parse_vault_detail(v_obj: &serde_json::Value, fallback_id: &str) -> VaultDetail {
    let id = v_obj.get("id").and_then(|i| i.as_str()).unwrap_or(fallback_id).to_string();
    let name = v_obj
        .get("name")
        .or_else(|| v_obj.get("title"))
        .and_then(|n| n.as_str())
        .unwrap_or("Bóveda")
        .to_string();
    let description = v_obj.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
    let color = v_obj.get("color").and_then(|c| c.as_str()).map(|s| s.to_string());
    let icon = v_obj.get("icon").and_then(|i| i.as_str()).map(|s| s.to_string());
    let provider = v_obj.get("provider").and_then(|p| p.as_str()).map(|s| s.to_string());
    let storage_account_name = v_obj.get("storage_account_name").and_then(|s| s.as_str()).map(|s| s.to_string());
    let storage_account_email = v_obj.get("storage_account_email").and_then(|s| s.as_str()).map(|s| s.to_string());
    let status = v_obj.get("status").and_then(|s| s.as_str()).map(|s| s.to_string());
    let file_count = v_obj.get("file_count").and_then(|c| c.as_u64()).map(|c| c as usize);
    let vault_size_bytes = v_obj.get("vault_size_bytes").or_else(|| v_obj.get("size_bytes")).and_then(|s| s.as_u64());
    let created_at = v_obj.get("created_at").and_then(|c| c.as_str()).map(|s| s.to_string());
    let updated_at = v_obj.get("updated_at").and_then(|u| u.as_str()).map(|s| s.to_string());

    let kinds = v_obj.get("kinds").and_then(|k| k.as_array()).map(|arr| {
        arr.iter().filter_map(|item| {
            let kind = item.get("kind")?.as_str()?.to_string();
            let count = item.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
            let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
            Some(VaultKindStat { kind, count, size })
        }).collect()
    });

    VaultDetail {
        id,
        name,
        description,
        color,
        icon,
        provider,
        storage_account_name,
        storage_account_email,
        status,
        file_count,
        vault_size_bytes,
        size_bytes: vault_size_bytes,
        created_at,
        updated_at,
        kinds,
    }
}

/// Obtiene la lista completa con metadatos de todas las bóvedas accesibles (`GET /api/v1/vaults`).
pub async fn fetch_all_vaults_detailed(
    api_base: &str,
    token: &str,
) -> Result<Vec<VaultDetail>, String> {
    let clean_base = api_base.trim_end_matches('/');
    let url = format!("{clean_base}/vaults?page=1&page_size=100");

    let client = crate::settings::shared_api_client();
    let mut req = client.get(&url).header("Accept", "application/json");
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let res = req.send().await.map_err(|e| format!("Error de conexión al consultar bóvedas: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("El servidor respondió con código {}", res.status()));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("Error procesando JSON de bóvedas: {e}"))?;
    let arr_opt = json
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| json.get("vaults").and_then(|v| v.as_array()))
        .or_else(|| json.as_array());

    let list = arr_opt
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let detail = parse_vault_detail(item, "");
                    if detail.name.trim().is_empty() {
                        None
                    } else {
                        Some(detail)
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(list)
}

/// Obtiene la lista completa de todas las bóvedas disponibles para el usuario (`GET /api/v1/vaults`).
pub async fn fetch_all_vaults(
    api_base: &str,
    token: &str,
) -> Result<Vec<CloudResource>, String> {
    let detailed = fetch_all_vaults_detailed(api_base, token).await?;
    Ok(detailed
        .into_iter()
        .map(|d| CloudResource {
            id: d.id,
            name: d.name,
            kind: CloudResourceKind::Vault,
        })
        .collect())
}

/// Obtiene los detalles de la bóveda y la lista de carpetas (`GET /api/v1/vaults/:id`).
pub async fn fetch_vault_info(
    api_base: &str,
    token: &str,
    vault_id: &str,
) -> Result<(VaultDetail, Vec<VaultFolder>), String> {
    let clean_base = api_base.trim_end_matches('/');
    let clean_id = vault_id.trim();
    let url = format!("{clean_base}/vaults/{clean_id}");

    let client = crate::settings::shared_api_client();

    let mut req = client.get(&url).header("Accept", "application/json");
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let res = req.send().await.map_err(|e| format!("Fallo de conexión al consultar bóveda: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("El servidor respondió con código {}", res.status()));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("Error leyendo JSON: {e}"))?;

    // Parsear carpetas
    let folders: Vec<VaultFolder> = json.get("folders")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_str()?.to_string();
                let vault_id = item.get("vault_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let parent_id = item.get("parent_id").and_then(|p| p.as_str()).map(|s| s.to_string());
                let name = item.get("name")?.as_str()?.to_string();
                let position = item.get("position").and_then(|p| p.as_i64());
                let created_at = item.get("created_at").and_then(|c| c.as_str()).map(|s| s.to_string());
                Some(VaultFolder { id, vault_id, parent_id, name, position, created_at })
            }).collect()
        })
        .unwrap_or_default();

    // Parsear objeto vault
    let v_obj = json.get("vault").unwrap_or(&json);
    let detail = parse_vault_detail(v_obj, vault_id);

    Ok((detail, folders))
}

/// Obtiene la lista de archivos de la bóveda (`GET /api/v1/vaults/:id/files`).
pub async fn fetch_vault_files(
    api_base: &str,
    token: &str,
    vault_id: &str,
) -> Result<Vec<VaultFile>, String> {
    let clean_base = api_base.trim_end_matches('/');
    let clean_id = vault_id.trim();
    let url = format!("{clean_base}/vaults/{clean_id}/files?page=1&page_size=150");

    let client = crate::settings::shared_api_client();

    let mut req = client.get(&url).header("Accept", "application/json");
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let res = req.send().await.map_err(|e| format!("Error obteniendo archivos: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("El servidor respondió con código {}", res.status()));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("Error procesando JSON de archivos: {e}"))?;

    let files: Vec<VaultFile> = json.get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_str()?.to_string();
                let vault_id = item.get("vault_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let folder_id = item.get("folder_id").and_then(|f| f.as_str()).map(|s| s.to_string());
                let name = item.get("name")?.as_str()?.to_string();
                let ext = item.get("ext").and_then(|e| e.as_str()).unwrap_or("").to_string();
                let kind = item.get("kind").and_then(|k| k.as_str()).unwrap_or("other").to_string();
                let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                let hash = item.get("hash").and_then(|h| h.as_str()).map(|s| s.to_string());
                let tags = item.get("tags").and_then(|t| t.as_array()).map(|tags_arr| {
                    tags_arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect()
                });
                let pinned = item.get("pinned").and_then(|p| p.as_bool());
                let created_at = item.get("created_at").and_then(|c| c.as_str()).map(|s| s.to_string());
                let updated_at = item.get("updated_at").and_then(|u| u.as_str()).map(|s| s.to_string());

                Some(VaultFile {
                    id,
                    vault_id,
                    folder_id,
                    name,
                    ext,
                    kind,
                    size,
                    hash,
                    tags,
                    pinned,
                    created_at,
                    updated_at,
                })
            }).collect()
        })
        .unwrap_or_default();

    Ok(files)
}

/// Resuelve un nombre de destino evitando colisiones mediante numeración incremental `(1)`, `(2)`, etc.
fn resolve_collision_name(dir: &std::path::Path, file_name: &str) -> PathBuf {
    let target = dir.join(file_name);
    if !target.exists() {
        return target;
    }

    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archivo");
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let mut counter = 1;
    loop {
        let candidate = if ext.is_empty() {
            format!("{stem} ({counter})")
        } else {
            format!("{stem} ({counter}).{ext}")
        };
        let next_target = dir.join(candidate);
        if !next_target.exists() {
            return next_target;
        }
        counter += 1;
        if counter > 9999 {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            return dir.join(format!("{stem}_{ts}.{ext}"));
        }
    }
}

/// Descarga el contenido binario o de texto de un archivo y lo guarda en el equipo.
/// Si `target_destination` es `None`, se utiliza la ruta predeterminada configurada en Ajustes.
/// Realiza streaming por bloques a un archivo temporal `.part` para un uso de memoria constante O(1)
/// y protección contra descargas corruptas.
pub async fn download_file_to_desktop(
    api_base: &str,
    token: &str,
    vault_id: &str,
    file_id: &str,
    file_name: &str,
    target_destination: Option<PathBuf>,
) -> Result<PathBuf, String> {
    download_file_to_desktop_with_progress(
        api_base,
        token,
        vault_id,
        file_id,
        file_name,
        target_destination,
        None,
        |_, _| {},
    )
    .await
}

/// Descarga un archivo con reporte en tiempo real de bytes transferidos y tamaño total.
pub async fn download_file_to_desktop_with_progress<F>(
    api_base: &str,
    token: &str,
    vault_id: &str,
    file_id: &str,
    file_name: &str,
    target_destination: Option<PathBuf>,
    known_size: Option<u64>,
    mut on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, Option<u64>),
{
    let clean_base = api_base.trim_end_matches('/');
    let clean_vault = vault_id.trim();
    let clean_file = file_id.trim();
    let url = format!("{clean_base}/vaults/{clean_vault}/files/{clean_file}/download");

    let client = crate::settings::shared_stream_client();

    let mut req = client.get(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let mut res = req.send().await.map_err(|e| format!("Error en la descarga: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await.unwrap_or_default();
        let short_err = if !error_body.is_empty() && error_body.len() < 160 {
            format!(" - {error_body}")
        } else {
            String::new()
        };
        return Err(format!("El servidor rechazó la descarga con código HTTP {status}{short_err}"));
    }

    // Sanitizar nombre de archivo contra path traversal y nombres reservados en Windows
    let safe_file_name = sanitize_filename_for_os(file_name);
    let file_ext = std::path::Path::new(&safe_file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();

    let final_target: PathBuf = match target_destination {
        Some(explicit_path) => {
            if explicit_path.is_dir() {
                if let Err(_) = std::fs::create_dir_all(&explicit_path) {
                    let fallback = get_default_os_downloads_dir();
                    let _ = std::fs::create_dir_all(&fallback);
                    resolve_collision_name(&fallback, &safe_file_name)
                } else {
                    resolve_collision_name(&explicit_path, &safe_file_name)
                }
            } else {
                // Conservar extensión si el usuario la omitió en el cuadro Guardar como
                let final_path = if explicit_path.extension().is_none() && !file_ext.is_empty() {
                    explicit_path.with_extension(&file_ext)
                } else {
                    explicit_path
                };
                if let Some(parent) = final_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                final_path
            }
        }
        None => {
            let mut downloads_dir = load_downloads_dir();
            if let Err(_) = std::fs::create_dir_all(&downloads_dir) {
                downloads_dir = get_default_os_downloads_dir();
                let _ = std::fs::create_dir_all(&downloads_dir);
            }
            resolve_collision_name(&downloads_dir, &safe_file_name)
        }
    };

    // Archivo temporal para garantizar escritura atómica y no dejar restos corruptos si se interrumpe
    let temp_target = {
        let parent = final_target.parent().unwrap_or_else(|| std::path::Path::new("."));
        let stem = final_target.file_stem().and_then(|s| s.to_str()).unwrap_or("descarga");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        parent.join(format!(".{stem}_{ts}.part"))
    };

    let total_bytes = res.content_length().or_else(|| known_size.filter(|&s| s > 0));
    on_progress(0, total_bytes);

    let stream_res: Result<(), String> = async {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp_target)
            .map_err(|e| format!("Error creando archivo local: {e}"))?;

        let mut downloaded: u64 = 0;
        let mut last_report_bytes: u64 = 0;
        let mut last_report_at = std::time::Instant::now();

        while let Some(chunk) = res.chunk().await.map_err(|e| format!("Error en transferencia de datos: {e}"))? {
            file.write_all(&chunk).map_err(|e| format!("Error escribiendo en disco: {e}"))?;
            downloaded += chunk.len() as u64;
            if downloaded - last_report_bytes >= 65_536 || last_report_at.elapsed().as_millis() >= 50 {
                on_progress(downloaded, total_bytes);
                last_report_bytes = downloaded;
                last_report_at = std::time::Instant::now();
            }
        }
        on_progress(downloaded, total_bytes.or(Some(downloaded)));
        file.flush().map_err(|e| format!("Error finalizando escritura: {e}"))?;
        Ok(())
    }.await;

    if let Err(e) = stream_res {
        let _ = std::fs::remove_file(&temp_target);
        return Err(e);
    }

    // Renombrar atómicamente el archivo temporal al destino final
    if let Err(_) = std::fs::rename(&temp_target, &final_target) {
        if let Err(copy_err) = std::fs::copy(&temp_target, &final_target) {
            let _ = std::fs::remove_file(&temp_target);
            return Err(format!("Error guardando archivo final: {copy_err}"));
        }
        let _ = std::fs::remove_file(&temp_target);
    }

    Ok(final_target)
}

/// Descarga una vista previa de texto plano para un archivo inspeccionado con truncamiento seguro UTF-8.
pub async fn fetch_text_preview(
    api_base: &str,
    token: &str,
    vault_id: &str,
    file_id: &str,
) -> Result<String, String> {
    let clean_base = api_base.trim_end_matches('/');
    let clean_vault = vault_id.trim();
    let clean_file = file_id.trim();
    let url = format!("{clean_base}/vaults/{clean_vault}/files/{clean_file}/download");

    let client = crate::settings::shared_api_client();

    let mut req = client.get(&url);
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let res = req.send().await.map_err(|e| format!("Error al obtener vista previa: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }

    let text = res.text().await.map_err(|e| format!("Error decodificando texto: {e}"))?;
    // Limitar tamaño de preview a 30,000 bytes respetando los límites de caracteres UTF-8
    if text.len() > 30000 {
        let mut end = 30000;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        Ok(format!("{}\n\n--- [Vista previa truncada a 30 KB] ---", &text[..end]))
    } else {
        Ok(text)
    }
}

// ── Motor de Bóvedas Locales (Persistencia, Escaneo y Organización) ──────────

/// Determina si un ID de bóveda pertenece a una bóveda local en disco (`local-...`).
pub fn is_local_vault_id(vault_id: &str) -> bool {
    vault_id.trim().starts_with("local-")
}

/// Obtiene el directorio base de configuración de Kyō (`%APPDATA%/kyo`).
fn get_kyo_config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("kyo");
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("kyo");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("kyo");
    }
    std::env::temp_dir().join("kyo")
}

/// Ruta del archivo de registro de bóvedas locales (`%APPDATA%/kyo/local_vaults.json`).
pub fn get_local_vaults_registry_path() -> PathBuf {
    get_kyo_config_dir().join("local_vaults.json")
}

/// Directorio predeterminado donde se crean las carpetas físicas de las bóvedas locales (`%APPDATA%/kyo/vaults`).
pub fn get_default_local_vaults_root_dir() -> PathBuf {
    get_kyo_config_dir().join("vaults")
}

/// Enumera los discos o unidades montadas disponibles en el equipo para que el usuario elija en qué disco crear la bóveda.
pub fn list_available_disks() -> Vec<DiskDriveInfo> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn GetLogicalDrives() -> u32;
            fn GetDiskFreeSpaceExW(
                lp_directory_name: *const u16,
                lp_free_bytes_available_to_caller: *mut u64,
                lp_total_number_of_bytes: *mut u64,
                lp_total_number_of_free_bytes: *mut u64,
            ) -> i32;
            fn GetVolumeInformationW(
                lp_root_path_name: *const u16,
                lp_volume_name_buffer: *mut u16,
                n_volume_name_size: u32,
                lp_volume_serial_number: *mut u32,
                lp_maximum_component_length: *mut u32,
                lp_file_system_flags: *mut u32,
                lp_file_system_name_buffer: *mut u16,
                n_file_system_name_size: u32,
            ) -> i32;
        }

        let mask = unsafe { GetLogicalDrives() };
        let mut disks = Vec::new();

        for i in 0..26u8 {
            if (mask & (1 << i)) == 0 {
                continue;
            }
            let letter = (b'A' + i) as char;
            let mount_point = format!("{letter}:\\");
            let code = format!("{letter}:");
            let wide_root: Vec<u16> = std::ffi::OsStr::new(&mount_point)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut free_avail: u64 = 0;
            let mut total_bytes: u64 = 0;
            let mut total_free: u64 = 0;
            let space_ok = unsafe {
                GetDiskFreeSpaceExW(
                    wide_root.as_ptr(),
                    &mut free_avail,
                    &mut total_bytes,
                    &mut total_free,
                )
            };
            if space_ok == 0 || total_bytes == 0 {
                continue;
            }

            let mut vol_buf = [0u16; 128];
            let vol_ok = unsafe {
                GetVolumeInformationW(
                    wide_root.as_ptr(),
                    vol_buf.as_mut_ptr(),
                    vol_buf.len() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            };

            let raw_label = if vol_ok != 0 {
                let end = vol_buf.iter().position(|&c| c == 0).unwrap_or(vol_buf.len());
                String::from_utf16_lossy(&vol_buf[..end]).trim().to_string()
            } else {
                String::new()
            };

            let label = if raw_label.is_empty() {
                format!("Disco Local ({code})")
            } else {
                format!("{raw_label} ({code})")
            };

            disks.push(DiskDriveInfo {
                mount_point,
                code,
                label,
                free_bytes: free_avail,
                total_bytes,
            });
        }

        if disks.is_empty() {
            disks.push(DiskDriveInfo {
                mount_point: "C:\\".to_string(),
                code: "C:".to_string(),
                label: "Disco Local (C:)".to_string(),
                free_bytes: 0,
                total_bytes: 0,
            });
        }

        disks
    }
    #[cfg(not(windows))]
    {
        vec![DiskDriveInfo {
            mount_point: "/".to_string(),
            code: "/".to_string(),
            label: "Disco Principal (/)".to_string(),
            free_bytes: 0,
            total_bytes: 0,
        }]
    }
}

/// Resuelve el directorio raíz de bóvedas para un disco seleccionado (`<Disco>\KyoVaults` o `%APPDATA%/kyo/vaults`).
pub fn resolve_disk_vaults_root_dir(selected_disk_mount: &str) -> PathBuf {
    let trimmed = selected_disk_mount.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("appdata") {
        return get_default_local_vaults_root_dir();
    }
    PathBuf::from(trimmed).join("KyoVaults")
}

/// Formatea un `SystemTime` en cadena ISO 8601 aproximada legible (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_system_time_iso(st: std::time::SystemTime) -> String {
    let secs = st
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if secs == 0 {
        return "-".to_string();
    }
    // Conversión civil (algoritmo de Howard Hinnant) sin dependencias externas
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Sincroniza el indicador `has_local_vault` y el resumen de bóvedas locales en `%APPDATA%/kyo/session.json`
/// para que tanto Desktop como el modo Web puedan consultar si el usuario tiene creada al menos una bóveda en modo local.
fn sync_local_vault_status_to_session(records: &[LocalVaultRecord]) {
    let session_file = get_kyo_config_dir().join("session.json");
    let mut map: serde_json::Map<String, serde_json::Value> = if session_file.is_file() {
        std::fs::read_to_string(&session_file)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    let has_local = !records.is_empty();
    let summary: Vec<serde_json::Value> = records
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "root_path": r.root_path,
                "color": r.color,
                "updated_at": r.updated_at,
            })
        })
        .collect();

    map.insert("has_local_vault".to_string(), serde_json::Value::Bool(has_local));
    map.insert("local_vault_mode_enabled".to_string(), serde_json::Value::Bool(has_local));
    map.insert("local_vaults_count".to_string(), serde_json::json!(records.len()));
    map.insert("local_vaults_summary".to_string(), serde_json::Value::Array(summary));

    if let Some(parent) = session_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = session_file.with_extension("json.tmp");
    let content = serde_json::Value::Object(map).to_string();
    if std::fs::write(&tmp, &content).is_ok() {
        if std::fs::rename(&tmp, &session_file).is_err() {
            let _ = std::fs::write(&session_file, &content);
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// Carga la lista de bóvedas locales registradas en `%APPDATA%/kyo/local_vaults.json`.
pub fn load_local_vault_records() -> Vec<LocalVaultRecord> {
    let reg_path = get_local_vaults_registry_path();
    if !reg_path.is_file() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&reg_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    if let Ok(list) = serde_json::from_str::<Vec<LocalVaultRecord>>(&content) {
        return list;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(arr) = val.get("vaults").or_else(|| val.get("data")).and_then(|v| v.as_array()) {
            let parsed: Vec<LocalVaultRecord> = arr
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect();
            return parsed;
        }
    }
    Vec::new()
}

/// Guarda la lista de bóvedas locales en `%APPDATA%/kyo/local_vaults.json` y sincroniza `session.json`.
pub fn save_local_vault_records(records: &[LocalVaultRecord]) -> Result<(), String> {
    let reg_path = get_local_vaults_registry_path();
    if let Some(parent) = reg_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json_str = serde_json::to_string_pretty(records)
        .map_err(|e| format!("Error serializando bóvedas locales: {e}"))?;
    let tmp_path = reg_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json_str)
        .map_err(|e| format!("Error escribiendo registro local: {e}"))?;
    if std::fs::rename(&tmp_path, &reg_path).is_err() {
        std::fs::write(&reg_path, &json_str)
            .map_err(|e| format!("Error guardando local_vaults.json: {e}"))?;
        let _ = std::fs::remove_file(&tmp_path);
    }
    sync_local_vault_status_to_session(records);
    Ok(())
}

/// Devuelve `true` únicamente si el usuario tiene creada al menos una bóveda en modo local.
/// Esta función gobierna si el "Modo Bóveda Local" es visible en modo Web y Desktop.
pub fn has_any_local_vault() -> bool {
    !load_local_vault_records().is_empty()
}

/// Obtiene el estado consolidado del Modo Bóveda Local (usado en Desktop y en modo Web).
pub fn get_local_vault_mode_status() -> LocalVaultModeStatus {
    let details = fetch_local_vaults_detailed();
    let count = details.len();
    let total_files: usize = details.iter().map(|v| v.file_count.unwrap_or(0)).sum();
    let total_bytes: u64 = details
        .iter()
        .map(|v| v.vault_size_bytes.or(v.size_bytes).unwrap_or(0))
        .sum();
    LocalVaultModeStatus {
        has_local_vault: count > 0,
        local_vault_mode_enabled: count > 0,
        local_vaults_count: count,
        total_files,
        total_bytes,
    }
}

/// Clasifica la categoría (`doc`, `image`, `sheet`, `code`, `other`) a partir de la extensión.
fn infer_file_kind_from_ext(ext: &str) -> String {
    let lower = ext.to_lowercase();
    if matches!(lower.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp") {
        "image".to_string()
    } else if matches!(lower.as_str(), "xlsx" | "xls" | "csv" | "tsv" | "ods") {
        "sheet".to_string()
    } else if matches!(lower.as_str(), "rs" | "go" | "ts" | "tsx" | "js" | "jsx" | "py" | "html" | "css" | "json" | "yaml" | "yml" | "toml" | "sql" | "sh" | "bat" | "ps1" | "vue") {
        "code".to_string()
    } else if matches!(lower.as_str(), "doc" | "docx" | "pdf" | "txt" | "md" | "markdown" | "rtf" | "odt") {
        "doc".to_string()
    } else {
        "other".to_string()
    }
}

/// Escanea el directorio físico de una bóveda local y devuelve `(VaultDetail, Vec<VaultFolder>, Vec<VaultFile>)`.
pub fn scan_local_vault(record: &LocalVaultRecord) -> (VaultDetail, Vec<VaultFolder>, Vec<VaultFile>) {
    let root = PathBuf::from(&record.root_path);
    let _ = std::fs::create_dir_all(&root);

    let mut folders = Vec::new();
    let mut files = Vec::new();
    let mut kind_map: std::collections::HashMap<String, (usize, u64)> = std::collections::HashMap::new();
    let mut total_size: u64 = 0;

    // 1. Escanear raíz y subcarpetas de primer nivel
    if let Ok(entries) = std::fs::read_dir(&root) {
        let mut pos_counter = 1i64;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if !n.starts_with('.') => n.to_string(),
                _ => continue,
            };

            if path.is_dir() {
                let folder_id = format!("lfld::{file_name}");
                let created_at = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().or_else(|_| m.created()).ok())
                    .map(format_system_time_iso);

                folders.push(VaultFolder {
                    id: folder_id.clone(),
                    vault_id: record.id.clone(),
                    parent_id: None,
                    name: file_name.clone(),
                    position: Some(pos_counter),
                    created_at,
                });
                pos_counter += 1;

                // Escanear archivos dentro de esta carpeta
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if !sub_path.is_file() {
                            continue;
                        }
                        let sub_name = match sub_path.file_name().and_then(|n| n.to_str()) {
                            Some(n) if !n.starts_with('.') => n.to_string(),
                            _ => continue,
                        };
                        let meta = sub_entry.metadata().ok();
                        let sz = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let mtime = meta
                            .as_ref()
                            .and_then(|m| m.modified().ok())
                            .map(format_system_time_iso)
                            .unwrap_or_else(|| record.updated_at.clone());
                        let ext = sub_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let kind = infer_file_kind_from_ext(&ext);

                        total_size += sz;
                        let stat = kind_map.entry(kind.clone()).or_insert((0, 0));
                        stat.0 += 1;
                        stat.1 += sz;

                        let full_str = sub_path.to_string_lossy().to_string();
                        let is_pinned = record.pinned_files.iter().any(|p| p == &sub_name || p == &full_str);
                        let tags = record.file_tags.get(&sub_name).cloned();

                        files.push(VaultFile {
                            id: format!("lfile::{full_str}"),
                            vault_id: record.id.clone(),
                            folder_id: Some(folder_id.clone()),
                            name: sub_name,
                            ext,
                            kind,
                            size: sz,
                            hash: Some(format!("LOCAL-FS · {}", path.display())),
                            tags,
                            pinned: Some(is_pinned),
                            created_at: Some(mtime.clone()),
                            updated_at: Some(mtime),
                        });
                    }
                }
            } else if path.is_file() {
                let meta = entry.metadata().ok();
                let sz = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(format_system_time_iso)
                    .unwrap_or_else(|| record.updated_at.clone());
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let kind = infer_file_kind_from_ext(&ext);

                total_size += sz;
                let stat = kind_map.entry(kind.clone()).or_insert((0, 0));
                stat.0 += 1;
                stat.1 += sz;

                let full_str = path.to_string_lossy().to_string();
                let is_pinned = record.pinned_files.iter().any(|p| p == &file_name || p == &full_str);
                let tags = record.file_tags.get(&file_name).cloned();

                files.push(VaultFile {
                    id: format!("lfile::{full_str}"),
                    vault_id: record.id.clone(),
                    folder_id: None,
                    name: file_name,
                    ext,
                    kind,
                    size: sz,
                    hash: Some(format!("LOCAL-FS · {}", root.display())),
                    tags,
                    pinned: Some(is_pinned),
                    created_at: Some(mtime.clone()),
                    updated_at: Some(mtime),
                });
            }
        }
    }

    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let mut kinds_vec: Vec<VaultKindStat> = kind_map
        .into_iter()
        .map(|(kind, (count, size))| VaultKindStat { kind, count, size })
        .collect();
    kinds_vec.sort_by(|a, b| b.count.cmp(&a.count));

    let detail = VaultDetail {
        id: record.id.clone(),
        name: record.name.clone(),
        description: record.description.clone().or_else(|| Some(format!("Carpeta local: {}", record.root_path))),
        color: Some(record.color.clone()),
        icon: Some("hard-drive".to_string()),
        provider: Some("Local Disk".to_string()),
        storage_account_name: Some(record.root_path.clone()),
        storage_account_email: Some("Almacenamiento Local en este PC".to_string()),
        status: Some("local_active".to_string()),
        file_count: Some(files.len()),
        vault_size_bytes: Some(total_size),
        size_bytes: Some(total_size),
        created_at: Some(record.created_at.clone()),
        updated_at: Some(record.updated_at.clone()),
        kinds: if kinds_vec.is_empty() { None } else { Some(kinds_vec) },
    };

    (detail, folders, files)
}

/// Obtiene los `VaultDetail` escaneados en tiempo real de todas las bóvedas locales del usuario.
pub fn fetch_local_vaults_detailed() -> Vec<VaultDetail> {
    let records = load_local_vault_records();
    records
        .iter()
        .map(|r| scan_local_vault(r).0)
        .collect()
}

/// Crea una nueva Bóveda Local en el equipo del usuario con carpetas iniciales opcionales para organización.
pub fn create_local_vault(
    name: &str,
    description: Option<String>,
    color: &str,
    custom_path: Option<PathBuf>,
    starter_folders: &[&str],
) -> Result<VaultDetail, String> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return Err("El nombre de la bóveda local no puede estar vacío.".to_string());
    }

    let mut records = load_local_vault_records();
    if records.iter().any(|r| r.name.eq_ignore_ascii_case(clean_name)) {
        return Err(format!("Ya existe una bóveda local llamada '{clean_name}'."));
    }

    let safe_folder_slug = sanitize_filename_for_os(clean_name);
    let target_dir = match custom_path {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => get_default_local_vaults_root_dir().join(&safe_folder_slug),
    };

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("No se pudo crear el directorio local '{}': {e}", target_dir.display()))?;

    // Crear subcarpetas organizativas seleccionadas por el usuario
    for folder_name in starter_folders {
        let clean_sub = sanitize_filename_for_os(folder_name);
        if !clean_sub.is_empty() {
            let _ = std::fs::create_dir_all(target_dir.join(clean_sub));
        }
    }

    let now_iso = format_system_time_iso(std::time::SystemTime::now());
    let ts_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let record = LocalVaultRecord {
        id: format!("local-{ts_millis}"),
        name: clean_name.to_string(),
        description: description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()),
        color: if color.trim().is_empty() { "emerald".to_string() } else { color.trim().to_string() },
        root_path: target_dir.to_string_lossy().to_string(),
        created_at: now_iso.clone(),
        updated_at: now_iso,
        pinned_files: Vec::new(),
        file_tags: std::collections::HashMap::new(),
    };

    let (detail, _, _) = scan_local_vault(&record);
    records.insert(0, record);
    save_local_vault_records(&records)?;

    Ok(detail)
}

/// Desvincula una bóveda local del registro de Kyō Desktop (conservando los archivos en el disco por seguridad).
pub fn remove_local_vault(vault_id: &str) -> Result<(), String> {
    let mut records = load_local_vault_records();
    let prev_len = records.len();
    records.retain(|r| r.id != vault_id);
    if records.len() == prev_len {
        return Err("No se encontró la bóveda local indicada.".to_string());
    }
    save_local_vault_records(&records)
}

/// Crea una nueva subcarpeta organizativa dentro de una bóveda local.
pub fn create_folder_in_local_vault(vault_id: &str, folder_name: &str) -> Result<VaultFolder, String> {
    let clean_folder = sanitize_filename_for_os(folder_name);
    if clean_folder.is_empty() || clean_folder == "archivo_descargado" {
        return Err("Ingrese un nombre de carpeta válido.".to_string());
    }

    let mut records = load_local_vault_records();
    let rec = records
        .iter_mut()
        .find(|r| r.id == vault_id)
        .ok_or_else(|| "Bóveda local no encontrada.".to_string())?;

    let root = PathBuf::from(&rec.root_path);
    let new_dir = root.join(&clean_folder);
    std::fs::create_dir_all(&new_dir)
        .map_err(|e| format!("Error creando carpeta '{}': {e}", clean_folder))?;

    let now_iso = format_system_time_iso(std::time::SystemTime::now());
    rec.updated_at = now_iso.clone();
    let _ = save_local_vault_records(&records);

    Ok(VaultFolder {
        id: format!("lfld::{clean_folder}"),
        vault_id: vault_id.to_string(),
        parent_id: None,
        name: clean_folder,
        position: None,
        created_at: Some(now_iso),
    })
}

/// Importa (copia) una lista de archivos del PC dentro de una bóveda local (en la raíz o carpeta activa).
pub fn import_files_to_local_vault(
    vault_id: &str,
    target_folder_id: Option<&str>,
    source_paths: &[PathBuf],
) -> Result<usize, String> {
    if source_paths.is_empty() {
        return Ok(0);
    }

    let mut records = load_local_vault_records();
    let rec = records
        .iter_mut()
        .find(|r| r.id == vault_id)
        .ok_or_else(|| "Bóveda local no encontrada.".to_string())?;

    let mut dest_dir = PathBuf::from(&rec.root_path);
    if let Some(fld_id) = target_folder_id {
        let folder_name = fld_id.strip_prefix("lfld::").unwrap_or(fld_id);
        let safe_sub = sanitize_filename_for_os(folder_name);
        if !safe_sub.is_empty() {
            dest_dir = dest_dir.join(safe_sub);
        }
    }
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Error preparando carpeta destino: {e}"))?;

    let mut imported_count = 0usize;
    for src in source_paths {
        if !src.is_file() {
            continue;
        }
        let raw_name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archivo");
        let safe_name = sanitize_filename_for_os(raw_name);
        let target_file = resolve_collision_name(&dest_dir, &safe_name);
        if std::fs::copy(src, &target_file).is_ok() {
            imported_count += 1;
        }
    }

    rec.updated_at = format_system_time_iso(std::time::SystemTime::now());
    let _ = save_local_vault_records(&records);

    Ok(imported_count)
}

/// Mueve un archivo dentro de una bóveda local hacia otra carpeta (o a la raíz si `target_folder_id` es `None`).
pub fn move_local_vault_file(
    vault_id: &str,
    file_id: &str,
    target_folder_id: Option<&str>,
) -> Result<PathBuf, String> {
    let raw_path = file_id.strip_prefix("lfile::").unwrap_or(file_id);
    let src_path = PathBuf::from(raw_path);
    if !src_path.is_file() {
        return Err("El archivo local ya no existe en la ubicación original.".to_string());
    }

    let mut records = load_local_vault_records();
    let rec = records
        .iter_mut()
        .find(|r| r.id == vault_id)
        .ok_or_else(|| "Bóveda local no encontrada.".to_string())?;

    let root = PathBuf::from(&rec.root_path);
    let dest_dir = match target_folder_id {
        Some(fld_id) if !fld_id.is_empty() => {
            let folder_name = fld_id.strip_prefix("lfld::").unwrap_or(fld_id);
            root.join(sanitize_filename_for_os(folder_name))
        }
        _ => root,
    };

    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("No se pudo acceder a la carpeta destino: {e}"))?;

    if src_path.parent() == Some(dest_dir.as_path()) {
        return Ok(src_path);
    }

    let file_name = src_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archivo");
    let final_dest = resolve_collision_name(&dest_dir, file_name);

    if std::fs::rename(&src_path, &final_dest).is_err() {
        std::fs::copy(&src_path, &final_dest)
            .map_err(|e| format!("Error moviendo archivo: {e}"))?;
        let _ = std::fs::remove_file(&src_path);
    }

    rec.updated_at = format_system_time_iso(std::time::SystemTime::now());
    let _ = save_local_vault_records(&records);

    Ok(final_dest)
}

/// Auto-organiza todos los archivos sueltos en la raíz de una bóveda local clasificándolos en carpetas
/// según su tipo (`Documentos`, `Imágenes`, `Hojas de cálculo`, `Código`, `Otros`).
pub fn auto_organize_local_vault(vault_id: &str) -> Result<usize, String> {
    let mut records = load_local_vault_records();
    let rec = records
        .iter_mut()
        .find(|r| r.id == vault_id)
        .ok_or_else(|| "Bóveda local no encontrada.".to_string())?;

    let root = PathBuf::from(&rec.root_path);
    let entries = std::fs::read_dir(&root)
        .map_err(|e| format!("No se pudo leer la carpeta de la bóveda: {e}"))?;

    let mut moved_count = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if !n.starts_with('.') => n.to_string(),
            _ => continue,
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let kind = infer_file_kind_from_ext(&ext);
        let folder_label = match kind.as_str() {
            "doc" => "Documentos",
            "image" => "Imágenes",
            "sheet" => "Hojas de cálculo",
            "code" => "Código",
            _ => "Otros",
        };

        let target_dir = root.join(folder_label);
        if std::fs::create_dir_all(&target_dir).is_ok() {
            let dest_path = resolve_collision_name(&target_dir, &file_name);
            if std::fs::rename(&path, &dest_path).is_ok() {
                moved_count += 1;
            } else if std::fs::copy(&path, &dest_path).is_ok() {
                let _ = std::fs::remove_file(&path);
                moved_count += 1;
            }
        }
    }

    if moved_count > 0 {
        rec.updated_at = format_system_time_iso(std::time::SystemTime::now());
        let _ = save_local_vault_records(&records);
    }

    Ok(moved_count)
}

/// Fija o desfija un archivo dentro de una bóveda local.
pub fn toggle_pin_local_file(vault_id: &str, file_name: &str) -> Result<bool, String> {
    let mut records = load_local_vault_records();
    let rec = records
        .iter_mut()
        .find(|r| r.id == vault_id)
        .ok_or_else(|| "Bóveda local no encontrada.".to_string())?;

    let is_now_pinned = if let Some(pos) = rec.pinned_files.iter().position(|p| p == file_name) {
        rec.pinned_files.remove(pos);
        false
    } else {
        rec.pinned_files.push(file_name.to_string());
        true
    };

    save_local_vault_records(&records)?;
    Ok(is_now_pinned)
}

/// Elimina un archivo físico dentro de una bóveda local.
pub fn delete_local_vault_file(vault_id: &str, file_id: &str) -> Result<(), String> {
    let raw_path = file_id.strip_prefix("lfile::").unwrap_or(file_id);
    let target = PathBuf::from(raw_path);
    if !target.is_file() {
        return Err("El archivo no se encontró en el disco.".to_string());
    }
    std::fs::remove_file(&target)
        .map_err(|e| format!("No se pudo eliminar el archivo: {e}"))?;

    let mut records = load_local_vault_records();
    if let Some(rec) = records.iter_mut().find(|r| r.id == vault_id) {
        rec.updated_at = format_system_time_iso(std::time::SystemTime::now());
        let _ = save_local_vault_records(&records);
    }
    Ok(())
}

// ── Acciones auxiliares desacopladas de closures complejos ───────────────────

fn trigger_overview_reload_action(
    mut vaults_overview: Signal<Vec<VaultDetail>>,
    mut available_vaults: Signal<Vec<CloudResource>>,
    mut is_overview_loading: Signal<bool>,
    mut overview_error: Signal<Option<String>>,
) {
    // 1. Cargar de inmediato las bóvedas locales desde el disco (0ms latencia, funciona 100% offline)
    let local_vaults = fetch_local_vaults_detailed();
    let local_resources: Vec<CloudResource> = local_vaults
        .iter()
        .map(|d| CloudResource {
            id: d.id.clone(),
            name: d.name.clone(),
            kind: CloudResourceKind::Vault,
        })
        .collect();

    vaults_overview.set(local_vaults.clone());
    available_vaults.set(local_resources.clone());
    overview_error.set(None);

    let b = resolve_cloud_api_base();
    let t = resolve_jwt_token().unwrap_or_default();

    // Si no hay token Cloud, mostramos las bóvedas locales sin bloquear con error si ya tiene bóvedas locales
    if t.is_empty() {
        is_overview_loading.set(false);
        return;
    }

    is_overview_loading.set(true);

    spawn(async move {
        match fetch_all_vaults_detailed(&b, &t).await {
            Ok(mut cloud_list) => {
                // Combinar: primero las bóvedas locales para acceso rápido, luego las bóvedas remotas
                let mut combined = fetch_local_vaults_detailed();
                combined.append(&mut cloud_list);

                let resources: Vec<CloudResource> = combined
                    .iter()
                    .map(|d| CloudResource {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        kind: CloudResourceKind::Vault,
                    })
                    .collect();
                available_vaults.set(resources);
                vaults_overview.set(combined);
            }
            Err(e) => {
                // Mantener visibles las bóvedas locales aunque falle el servidor remoto
                let current_local = fetch_local_vaults_detailed();
                vaults_overview.set(current_local);
                overview_error.set(Some(format!("Mostrando bóvedas locales (sin conexión a Kyō Cloud: {e})")));
            }
        }
        is_overview_loading.set(false);
    });
}

fn trigger_reload_action(
    target_vault_id_or_name: String,
    mut active_context: Signal<Option<CloudResource>>,
    mut available_vaults: Signal<Vec<CloudResource>>,
    mut is_loading: Signal<bool>,
    mut error_msg: Signal<Option<String>>,
    mut vault_detail: Signal<Option<VaultDetail>>,
    mut folders: Signal<Vec<VaultFolder>>,
    mut files: Signal<Vec<VaultFile>>,
) {
    let trimmed_target = target_vault_id_or_name.trim().to_string();
    let local_records = load_local_vault_records();

    // Verificar primero si la bóveda solicitada es una Bóveda Local
    let matched_local = if !trimmed_target.is_empty() {
        local_records
            .iter()
            .find(|r| r.id == trimmed_target || r.name.eq_ignore_ascii_case(&trimmed_target))
            .cloned()
    } else {
        None
    };

    if let Some(local_rec) = matched_local {
        is_loading.set(true);
        error_msg.set(None);

        let (detail, f_list, file_list) = scan_local_vault(&local_rec);
        let picked_res = CloudResource {
            id: detail.id.clone(),
            name: detail.name.clone(),
            kind: CloudResourceKind::Vault,
        };
        if active_context.peek().as_ref().map(|c| &c.id) != Some(&detail.id) {
            active_context.set(Some(picked_res));
        }

        // Asegurar que available_vaults incluya todas las bóvedas locales conocidas
        let mut cur_avail = available_vaults.peek().clone();
        for lr in &local_records {
            if !cur_avail.iter().any(|v| v.id == lr.id) {
                cur_avail.insert(
                    0,
                    CloudResource {
                        id: lr.id.clone(),
                        name: lr.name.clone(),
                        kind: CloudResourceKind::Vault,
                    },
                );
            }
        }
        available_vaults.set(cur_avail);

        vault_detail.set(Some(detail));
        folders.set(f_list);
        files.set(file_list);
        is_loading.set(false);
        return;
    }

    let b = resolve_cloud_api_base();
    let t = resolve_jwt_token().unwrap_or_default();

    if t.is_empty() {
        // Si no hay token Cloud pero existen bóvedas locales, abrir la primera bóveda local
        if let Some(first_local) = local_records.first() {
            let (detail, f_list, file_list) = scan_local_vault(first_local);
            active_context.set(Some(CloudResource {
                id: detail.id.clone(),
                name: detail.name.clone(),
                kind: CloudResourceKind::Vault,
            }));
            vault_detail.set(Some(detail));
            folders.set(f_list);
            files.set(file_list);
            is_loading.set(false);
            return;
        }
        error_msg.set(Some("No se encontró un token de autenticación Cloud ni bóvedas locales.".to_string()));
        return;
    }

    is_loading.set(true);
    error_msg.set(None);

    spawn(async move {
        // 1. Combinar bóvedas locales + bóvedas del servidor remoto
        let mut all_v: Vec<CloudResource> = load_local_vault_records()
            .into_iter()
            .map(|r| CloudResource {
                id: r.id,
                name: r.name,
                kind: CloudResourceKind::Vault,
            })
            .collect();

        match fetch_all_vaults(&b, &t).await {
            Ok(mut v_list) => {
                all_v.append(&mut v_list);
                available_vaults.set(all_v.clone());
            }
            Err(e) => {
                if all_v.is_empty() {
                    error_msg.set(Some(format!("Error cargando lista de bóvedas: {e}")));
                    is_loading.set(false);
                    return;
                }
            }
        };

        if all_v.is_empty() {
            error_msg.set(Some("No se encontraron bóvedas registradas.".to_string()));
            is_loading.set(false);
            return;
        }

        // 2. Resolver la bóveda objetivo: coincidir por ID o por Nombre (insensible a mayúsculas)
        let matched = if !trimmed_target.is_empty() {
            all_v.iter().find(|v| v.id == trimmed_target || v.name.eq_ignore_ascii_case(&trimmed_target)).cloned()
        } else {
            None
        }.or_else(|| {
            if let Some(prof_name) = load_active_profile_name() {
                let p = prof_name.trim();
                all_v.iter().find(|v| v.id == p || v.name.eq_ignore_ascii_case(p)).cloned()
            } else {
                None
            }
        }).or_else(|| {
            all_v.first().cloned()
        });

        let target_vault = match matched {
            Some(v) => v,
            None => {
                error_msg.set(Some("No se encontró la bóveda solicitada.".to_string()));
                is_loading.set(false);
                return;
            }
        };

        let resolved_id = target_vault.id.clone();

        if active_context.peek().as_ref().map(|c| &c.id) != Some(&resolved_id) {
            active_context.set(Some(target_vault.clone()));
        }

        // Si resultó ser una bóveda local tras el fallback, escanearla localmente
        if is_local_vault_id(&resolved_id) {
            if let Some(rec) = load_local_vault_records().into_iter().find(|r| r.id == resolved_id) {
                let (detail, f_list, file_list) = scan_local_vault(&rec);
                vault_detail.set(Some(detail));
                folders.set(f_list);
                files.set(file_list);
                is_loading.set(false);
                return;
            }
        }

        // 3 y 4. Obtener metadatos, carpetas y archivos concurrentemente usando el pool HTTP/2 compartido
        let (info_res, files_res) = tokio::join!(
            fetch_vault_info(&b, &t, &resolved_id),
            fetch_vault_files(&b, &t, &resolved_id)
        );

        match info_res {
            Ok((detail, f_list)) => {
                vault_detail.set(Some(detail));
                folders.set(f_list);
            }
            Err(e) => {
                error_msg.set(Some(format!("Error cargando metadatos: {e}")));
            }
        }

        match files_res {
            Ok(f_list) => {
                files.set(f_list);
            }
            Err(e) => {
                if error_msg.read().is_none() {
                    error_msg.set(Some(format!("Error cargando archivos: {e}")));
                }
            }
        }

        is_loading.set(false);
    });
}

fn trigger_download_action(
    file: VaultFile,
    target_destination: Option<PathBuf>,
    mut downloading_files: Signal<std::collections::HashSet<String>>,
    mut download_toast: Signal<Option<DownloadToast>>,
    mut transfer_progress_map: Signal<std::collections::HashMap<String, TransferProgress>>,
) {
    // Si el archivo pertenece a una Bóveda Local, copiar directamente en disco
    if file.id.starts_with("lfile::") || is_local_vault_id(&file.vault_id) {
        let raw_src = file.id.strip_prefix("lfile::").unwrap_or(&file.id);
        let src_path = PathBuf::from(raw_src);
        let safe_name = sanitize_filename_for_os(&file.name);
        let dest_path = match target_destination {
            Some(explicit) => {
                if explicit.is_dir() {
                    let _ = std::fs::create_dir_all(&explicit);
                    resolve_collision_name(&explicit, &safe_name)
                } else {
                    explicit
                }
            }
            None => {
                let d_dir = load_downloads_dir();
                let _ = std::fs::create_dir_all(&d_dir);
                resolve_collision_name(&d_dir, &safe_name)
            }
        };

        match std::fs::copy(&src_path, &dest_path) {
            Ok(_) => {
                let saved_name = dest_path.file_name().and_then(|n| n.to_str()).unwrap_or(&file.name);
                download_toast.set(Some(DownloadToast {
                    message: format!("Archivo copiado correctamente: {saved_name}"),
                    path: Some(dest_path),
                    is_error: false,
                }));
            }
            Err(e) => {
                download_toast.set(Some(DownloadToast {
                    message: format!("Error copiando archivo local: {e}"),
                    path: None,
                    is_error: true,
                }));
            }
        }
        return;
    }

    let b = resolve_cloud_api_base();
    let t = resolve_jwt_token().unwrap_or_default();
    let f_id = file.id.clone();
    let f_name = file.name.clone();
    let known_size = if file.size > 0 { Some(file.size) } else { None };

    downloading_files.write().insert(f_id.clone());
    transfer_progress_map.write().insert(
        f_id.clone(),
        TransferProgress {
            file_id: f_id.clone(),
            file_name: f_name.clone(),
            direction: TransferDirection::Download,
            status: TransferStatus::Active,
            transferred_bytes: 0,
            total_bytes: known_size,
            message: None,
        },
    );
    download_toast.set(Some(DownloadToast {
        message: format!("Descargando '{}'...", f_name),
        path: None,
        is_error: false,
    }));

    spawn(async move {
        let f_id_prog = f_id.clone();
        let mut progress_sig = transfer_progress_map;
        let res = download_file_to_desktop_with_progress(
            &b,
            &t,
            &file.vault_id,
            &file.id,
            &f_name,
            target_destination,
            known_size,
            move |transferred, total| {
                if let Some(entry) = progress_sig.write().get_mut(&f_id_prog) {
                    entry.transferred_bytes = transferred;
                    if total.is_some() {
                        entry.total_bytes = total;
                    }
                }
            },
        )
        .await;

        match res {
            Ok(path) => {
                let folder_name = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("carpeta");
                let saved_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(&f_name).to_string();
                if let Some(entry) = transfer_progress_map.write().get_mut(&f_id) {
                    entry.status = TransferStatus::Completed;
                    if let Some(total) = entry.total_bytes {
                        entry.transferred_bytes = entry.transferred_bytes.max(total);
                    }
                    entry.message = Some(format!("Guardado en {folder_name}: {saved_name}"));
                }
                download_toast.set(Some(DownloadToast {
                    message: format!("Archivo guardado en {folder_name}: {saved_name}"),
                    path: Some(path),
                    is_error: false,
                }));
            }
            Err(err) => {
                if let Some(entry) = transfer_progress_map.write().get_mut(&f_id) {
                    entry.status = TransferStatus::Error;
                    entry.message = Some(err.clone());
                }
                download_toast.set(Some(DownloadToast {
                    message: format!("Error al descargar: {err}"),
                    path: None,
                    is_error: true,
                }));
            }
        }
        downloading_files.write().remove(&f_id);
    });
}

fn trigger_save_as_dialog_action(
    file: VaultFile,
    downloading_files: Signal<std::collections::HashSet<String>>,
    download_toast: Signal<Option<DownloadToast>>,
    transfer_progress_map: Signal<std::collections::HashMap<String, TransferProgress>>,
) {
    let default_dir = load_downloads_dir();
    let safe_name = sanitize_filename_for_os(&file.name);
    let mut dialog = rfd::FileDialog::new()
        .set_directory(&default_dir)
        .set_file_name(&safe_name);

    if !file.ext.is_empty() {
        dialog = dialog.add_filter(&format!("Archivo .{}", file.ext.to_uppercase()), &[&file.ext]);
    }

    if let Some(target_path) = dialog.save_file() {
        trigger_download_action(
            file,
            Some(target_path),
            downloading_files,
            download_toast,
            transfer_progress_map,
        );
    }
}

fn trigger_inspect_action(
    file: VaultFile,
    mut inspected_file: Signal<Option<VaultFile>>,
    mut preview_text: Signal<Option<String>>,
    mut is_preview_loading: Signal<bool>,
    mut preview_error: Signal<Option<String>>,
) {
    inspected_file.set(Some(file.clone()));
    preview_text.set(None);
    preview_error.set(None);

    if is_previewable_text(&file.ext) && file.size < 250_000 {
        // Si es un archivo de bóveda local, leer directamente desde el sistema de archivos local en 0ms
        if let Some(raw_path) = file.id.strip_prefix("lfile::") {
            match std::fs::read_to_string(raw_path) {
                Ok(text) => {
                    if text.len() > 30000 {
                        let mut end = 30000;
                        while end > 0 && !text.is_char_boundary(end) {
                            end -= 1;
                        }
                        preview_text.set(Some(format!(
                            "{}\n\n--- [Vista previa local truncada a 30 KB] ---",
                            &text[..end]
                        )));
                    } else {
                        preview_text.set(Some(text));
                    }
                }
                Err(e) => {
                    preview_error.set(Some(format!("Error leyendo archivo local: {e}")));
                }
            }
            return;
        }

        is_preview_loading.set(true);
        let b = resolve_cloud_api_base();
        let t = resolve_jwt_token().unwrap_or_default();
        let v_id = file.vault_id.clone();
        let f_id = file.id.clone();

        spawn(async move {
            match fetch_text_preview(&b, &t, &v_id, &f_id).await {
                Ok(txt) => {
                    preview_text.set(Some(txt));
                }
                Err(err) => {
                    preview_error.set(Some(err));
                }
            }
            is_preview_loading.set(false);
        });
    }
}

// ============================================================================
// COMPONENTE PRINCIPAL: VaultView
// ============================================================================

#[component]
pub fn VaultView() -> Element {
    let mut active_view = use_context::<Signal<ActiveView>>();
    let cloud = use_kyo_cloud();
    let mut active_context = cloud.active_context;
    let account = cloud.account();

    // Modo de navegación interno: Vista general (cards) -> Detalle de bóveda
    let mut page_mode = use_signal(|| VaultPageMode::Overview);
    let vaults_overview = use_signal(fetch_local_vaults_detailed);
    let is_overview_loading = use_signal(|| false);
    let overview_error = use_signal(|| Option::<String>::None);
    let mut overview_search = use_signal(String::new);
    let mut overview_sort = use_signal(|| OverviewSortMode::Recent);
    let mut overview_source = use_signal(|| OverviewSourceFilter::All);

    // Estados del Modal "Crear Bóveda Local"
    let mut show_create_local_modal = use_signal(|| false);
    let mut new_vault_name = use_signal(String::new);
    let mut new_vault_desc = use_signal(String::new);
    let mut new_vault_color = use_signal(|| "emerald".to_string());
    let mut new_vault_disk = use_signal(|| {
        list_available_disks()
            .first()
            .map(|d| d.mount_point.clone())
            .unwrap_or_else(|| "appdata".to_string())
    });
    let mut new_vault_custom_dir = use_signal(|| Option::<PathBuf>::None);
    let mut new_vault_create_starters = use_signal(|| true);
    let mut create_vault_error = use_signal(|| Option::<String>::None);

    // Estado para crear nueva subcarpeta dentro de una bóveda local en la vista de detalle
    let mut show_new_folder_bar = use_signal(|| false);
    let mut new_folder_name_input = use_signal(String::new);
    let mut selected_target_local_vault = use_signal(String::new);

    // Lista de todas las bóvedas conocidas (locales + nube)
    let available_vaults = use_signal(|| {
        let mut list: Vec<CloudResource> = load_local_vault_records()
            .into_iter()
            .map(|r| CloudResource {
                id: r.id,
                name: r.name,
                kind: CloudResourceKind::Vault,
            })
            .collect();
        if let Some(acc) = account.as_ref() {
            for v in &acc.vaults {
                if !list.iter().any(|x| x.id == v.id) {
                    list.push(v.clone());
                }
            }
        }
        list
    });

    // Estados reactivos de la vista de detalle
    let vault_detail = use_signal(|| Option::<VaultDetail>::None);
    let folders = use_signal(|| Vec::<VaultFolder>::new());
    let files = use_signal(|| Vec::<VaultFile>::new());
    let is_loading = use_signal(|| false);
    let error_msg = use_signal(|| Option::<String>::None);

    // Filtros y ordenamiento
    let mut search_query = use_signal(String::new);
    let mut selected_folder = use_signal(|| Option::<Option<String>>::None); // None = Todas, Some(None) = Raíz, Some(Some(id)) = Carpeta
    let mut kind_filter = use_signal(|| "all".to_string());
    let mut sort_mode = use_signal(|| SortMode::Recent);
    let mut view_mode = use_signal(|| ViewMode::Grid);

    // Modal de inspección de archivo
    let mut inspected_file = use_signal(|| Option::<VaultFile>::None);
    let preview_text = use_signal(|| Option::<String>::None);
    let is_preview_loading = use_signal(|| false);
    let preview_error = use_signal(|| Option::<String>::None);

    // Toast de descarga, progreso de transferencias y configuración de descargas
    let mut download_toast = use_signal(|| Option::<DownloadToast>::None);
    let downloading_files = use_signal(|| std::collections::HashSet::<String>::new());
    let transfer_progress_map = use_signal(|| std::collections::HashMap::<String, TransferProgress>::new());
    let settings_open = try_use_context::<IsSettingsOpen>();
    let target_tab_ctx = try_use_context::<Signal<Option<SettingsTab>>>();
    let app_downloads_dir_ctx = try_use_context::<crate::models::AppDownloadsDir>();
    let mut downloads_dir = if let Some(ctx) = app_downloads_dir_ctx {
        ctx.0
    } else {
        use_signal(load_downloads_dir)
    };

    // Cargar automáticamente la vista general de bóvedas al montar el componente
    use_effect(move || {
        trigger_overview_reload_action(
            vaults_overview,
            available_vaults,
            is_overview_loading,
            overview_error,
        );
    });

    // Renderizador del Modal de Creación de Bóveda Local (compartido en Overview y Detail)
    let render_create_local_modal = {
        let is_open = *show_create_local_modal.read();
        if is_open {
            let cur_name = new_vault_name.read().clone();
            let cur_desc = new_vault_desc.read().clone();
            let cur_color = new_vault_color.read().clone();
            let cur_disk = new_vault_disk.read().clone();
            let cur_custom_dir = new_vault_custom_dir.read().clone();
            let create_starters = *new_vault_create_starters.read();
            let err_opt = create_vault_error.read().clone();
            let available_disks = list_available_disks();
            let slug = if cur_name.trim().is_empty() {
                "mi_boveda_local".to_string()
            } else {
                sanitize_filename_for_os(&cur_name)
            };
            let disk_base_dir = resolve_disk_vaults_root_dir(&cur_disk);
            let resolved_target_dir = cur_custom_dir
                .clone()
                .unwrap_or_else(|| disk_base_dir.join(&slug));
            let effective_dir_display = resolved_target_dir.display().to_string();
            let effective_dir_for_open = resolved_target_dir.clone();
            let disk_base_for_dialog = disk_base_dir.clone();
            let resolved_target_for_submit = resolved_target_dir.clone();

            rsx! {
                div {
                    class: "vk-modal-backdrop animate-fade-in",
                    onclick: move |_| show_create_local_modal.set(false),
                    div {
                        class: "vk-modal-card",
                        onclick: move |e| e.stop_propagation(),
                        header {
                            class: "vk-modal-header",
                            div {
                                class: "flex items-center gap-3",
                                div {
                                    class: "vk-file-icon-box vk-file-icon-sheet vk-modal-header-icon",
                                    Icon { d: ICON_HARD_DRIVE, class: "dk-ic" }
                                }
                                div {
                                    h3 { class: "font-semibold text-sm text-zinc-100", "Crear Nueva Bóveda Local" }
                                    span { class: "text-xs text-zinc-400", "Organiza tu almacenamiento local en carpetas listas para Desktop y Modo Bóveda Local Web" }
                                }
                            }
                            button {
                                class: "vk-modal-close-btn",
                                onclick: move |_| show_create_local_modal.set(false),
                                Icon { d: ICON_X, class: "dk-ic-sm" }
                            }
                        }

                        div {
                            class: "vk-modal-body",
                            if let Some(err) = err_opt {
                                div {
                                    class: "p-3 rounded-xl bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300 flex items-center gap-2",
                                    Icon { d: ICON_ALERT, class: "dk-ic-sm flex-none" }
                                    span { "{err}" }
                                }
                            }

                            // Nombre + Plantillas rápidas
                            div {
                                class: "flex flex-col gap-1.5",
                                label { class: "text-xs font-semibold text-zinc-300", "Nombre de la bóveda local *" }
                                input {
                                    class: "vk-search-input !pl-3",
                                    r#type: "text",
                                    placeholder: "Ej. Proyectos Personales, Documentos 2026, Recursos...",
                                    value: "{cur_name}",
                                    oninput: move |e| {
                                        new_vault_name.set(e.value());
                                        create_vault_error.set(None);
                                    },
                                }
                                div {
                                    class: "flex items-center gap-1.5 flex-wrap mt-1",
                                    span { class: "text-[11px] text-zinc-400", "Sugerencias:" }
                                    for preset in ["Documentos y Notas", "Proyectos de Código", "Multimedia y Diseño", "Archivo Personal"] {
                                        button {
                                            key: "{preset}",
                                            r#type: "button",
                                            class: "text-[11px] px-2 py-0.5 rounded-md bg-zinc-800/80 hover:bg-zinc-700 text-zinc-300 border border-zinc-700/60 cursor-pointer",
                                            onclick: move |_| {
                                                new_vault_name.set(preset.to_string());
                                                create_vault_error.set(None);
                                            },
                                            "+ {preset}"
                                        }
                                    }
                                }
                            }

                            // Descripción
                            div {
                                class: "flex flex-col gap-1.5",
                                label { class: "text-xs font-semibold text-zinc-300", "Descripción o propósito (opcional)" }
                                input {
                                    class: "vk-search-input !pl-3",
                                    r#type: "text",
                                    placeholder: "Ej. Almacenamiento organizado para trabajar sin conexión...",
                                    value: "{cur_desc}",
                                    oninput: move |e| new_vault_desc.set(e.value()),
                                }
                            }

                            // Selector de color identificador
                            div {
                                class: "flex flex-col gap-1.5",
                                label { class: "text-xs font-semibold text-zinc-300", "Color de etiqueta organizativa" }
                                div {
                                    class: "flex items-center gap-2 flex-wrap",
                                    for (col_id, col_label, col_cls) in [
                                        ("emerald", "Esmeralda", "bg-emerald-500/20 text-emerald-300 border-emerald-500/40"),
                                        ("indigo", "Índigo", "bg-indigo-500/20 text-indigo-300 border-indigo-500/40"),
                                        ("amber", "Ámbar", "bg-amber-500/20 text-amber-300 border-amber-500/40"),
                                        ("rose", "Rosa", "bg-rose-500/20 text-rose-300 border-rose-500/40"),
                                        ("purple", "Violeta", "bg-purple-500/20 text-purple-300 border-purple-500/40"),
                                    ] {
                                        button {
                                            key: "{col_id}",
                                            r#type: "button",
                                            class: if cur_color == col_id {
                                                format!("px-2.5 py-1 rounded-lg text-xs font-semibold border ring-2 ring-white/20 cursor-pointer {col_cls}")
                                            } else {
                                                "px-2.5 py-1 rounded-lg text-xs font-medium bg-zinc-900 text-zinc-400 border border-zinc-800 hover:text-zinc-200 cursor-pointer".to_string()
                                            },
                                            onclick: move |_| new_vault_color.set(col_id.to_string()),
                                            "{col_label}"
                                        }
                                    }
                                }
                            }

                            // Selector de Disco y Dirección Exacta en el PC
                            div {
                                class: "vk-modal-subcard flex-col !items-stretch gap-2.5",
                                div {
                                    class: "flex items-center justify-between gap-2 flex-wrap",
                                    div {
                                        class: "flex items-center gap-2",
                                        Icon { d: ICON_HARD_DRIVE, class: "dk-ic-sm text-emerald-400" }
                                        span { class: "text-xs font-semibold text-zinc-200", "Elegir disco donde se creará la bóveda *" }
                                    }
                                    div {
                                        class: "flex items-center gap-1.5",
                                        if cur_custom_dir.is_some() {
                                            button {
                                                r#type: "button",
                                                class: "dk-btn dk-btn-ghost dk-btn-xs text-zinc-400",
                                                onclick: move |_| new_vault_custom_dir.set(None),
                                                "Usar raíz del disco"
                                            }
                                        }
                                        button {
                                            r#type: "button",
                                            class: "dk-btn dk-btn-secondary dk-btn-xs",
                                            title: "Elegir una subcarpeta personalizada dentro del disco seleccionado",
                                            onclick: move |_| {
                                                let start_dir = if disk_base_for_dialog.exists() {
                                                    disk_base_for_dialog.clone()
                                                } else {
                                                    disk_base_for_dialog
                                                        .parent()
                                                        .map(|p| p.to_path_buf())
                                                        .unwrap_or_else(get_default_local_vaults_root_dir)
                                                };
                                                if let Some(picked) = rfd::FileDialog::new().set_directory(&start_dir).pick_folder() {
                                                    new_vault_custom_dir.set(Some(picked));
                                                }
                                            },
                                            Icon { d: ICON_FOLDER, class: "dk-ic-xs" }
                                            "Elegir subcarpeta..."
                                        }
                                    }
                                }

                                // Tarjetas de Discos Detectados en el Equipo
                                div {
                                    class: "grid grid-cols-2 gap-2",
                                    for disk in available_disks.iter() {
                                        {
                                            let mount_val = disk.mount_point.clone();
                                            let is_selected = cur_custom_dir.is_none() && cur_disk.eq_ignore_ascii_case(&mount_val);
                                            let free_str = format_bytes(disk.free_bytes);
                                            let total_str = format_bytes(disk.total_bytes);
                                            let used_pct = if disk.total_bytes > 0 {
                                                (((disk.total_bytes.saturating_sub(disk.free_bytes)) as f64 / disk.total_bytes as f64) * 100.0).clamp(4.0, 100.0)
                                            } else {
                                                0.0
                                            };
                                            rsx! {
                                                button {
                                                    key: "{disk.mount_point}",
                                                    r#type: "button",
                                                    class: if is_selected {
                                                        "p-2.5 rounded-xl border border-emerald-500/60 bg-emerald-500/10 text-left flex flex-col gap-1 cursor-pointer ring-1 ring-emerald-500/30"
                                                    } else {
                                                        "p-2.5 rounded-xl border border-zinc-800 bg-zinc-900/70 hover:bg-zinc-800/70 text-left flex flex-col gap-1 cursor-pointer"
                                                    },
                                                    onclick: move |_| {
                                                        new_vault_disk.set(mount_val.clone());
                                                        new_vault_custom_dir.set(None);
                                                        create_vault_error.set(None);
                                                    },
                                                    div {
                                                        class: "flex items-center justify-between gap-1.5",
                                                        div {
                                                            class: "flex items-center gap-1.5 min-w-0",
                                                            Icon { d: ICON_HARD_DRIVE, class: if is_selected { "dk-ic-xs text-emerald-400" } else { "dk-ic-xs text-zinc-400" } }
                                                            span { class: "text-xs font-semibold text-zinc-100 truncate", "{disk.label}" }
                                                        }
                                                        span {
                                                            class: if is_selected {
                                                                "text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 font-bold"
                                                            } else {
                                                                "text-[10px] font-mono px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400"
                                                            },
                                                            "{disk.mount_point}"
                                                        }
                                                    }
                                                    if disk.total_bytes > 0 {
                                                        div {
                                                            class: "text-[10px] text-zinc-400 flex items-center justify-between",
                                                            span { "{free_str} libres" }
                                                            span { "de {total_str}" }
                                                        }
                                                        div {
                                                            class: "w-full h-1 rounded-full bg-zinc-800 overflow-hidden",
                                                            div {
                                                                class: if is_selected { "h-full bg-emerald-400" } else { "h-full bg-zinc-500" },
                                                                style: format!("width: {used_pct:.1}%;"),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Opción adicional: Carpeta AppData de Kyō
                                    {
                                        let is_appdata = cur_custom_dir.is_none() && cur_disk.eq_ignore_ascii_case("appdata");
                                        rsx! {
                                            button {
                                                r#type: "button",
                                                class: if is_appdata {
                                                    "p-2.5 rounded-xl border border-emerald-500/60 bg-emerald-500/10 text-left flex flex-col justify-between gap-1 cursor-pointer ring-1 ring-emerald-500/30"
                                                } else {
                                                    "p-2.5 rounded-xl border border-zinc-800 bg-zinc-900/70 hover:bg-zinc-800/70 text-left flex flex-col justify-between gap-1 cursor-pointer"
                                                },
                                                onclick: move |_| {
                                                    new_vault_disk.set("appdata".to_string());
                                                    new_vault_custom_dir.set(None);
                                                    create_vault_error.set(None);
                                                },
                                                div {
                                                    class: "flex items-center justify-between gap-1.5",
                                                    div {
                                                        class: "flex items-center gap-1.5 min-w-0",
                                                        Icon { d: ICON_FOLDER, class: if is_appdata { "dk-ic-xs text-emerald-400" } else { "dk-ic-xs text-zinc-400" } }
                                                        span { class: "text-xs font-semibold text-zinc-100 truncate", "AppData de Kyō" }
                                                    }
                                                    span { class: "text-[10px] font-mono px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400", "%APPDATA%" }
                                                }
                                                span { class: "text-[10px] text-zinc-400 truncate", "Directorio interno de la aplicación" }
                                            }
                                        }
                                    }
                                }

                                // Vista previa de la dirección exacta donde se creará la bóveda (clicable para abrir en el explorador predeterminado)
                                div {
                                    class: "flex flex-col gap-1 mt-0.5",
                                    span { class: "text-[11px] text-zinc-400", "Dirección exacta donde se alojará la bóveda (clic para abrir en tu explorador):" }
                                    button {
                                        r#type: "button",
                                        class: "w-full text-left font-mono text-[11px] text-emerald-300 bg-zinc-950/90 hover:bg-zinc-900 px-2.5 py-2 rounded-lg border border-emerald-500/30 hover:border-emerald-400/60 flex items-center justify-between gap-2 cursor-pointer transition-colors",
                                        title: "Haz clic para abrir esta dirección en tu aplicación de explorador de archivos predeterminada",
                                        onclick: move |_| {
                                            open_path_in_explorer(&effective_dir_for_open);
                                        },
                                        span { class: "truncate", "{effective_dir_display}" }
                                        span {
                                            class: "flex items-center gap-1 text-[10px] font-sans font-semibold text-emerald-400 flex-none",
                                            Icon { d: ICON_EXTERNAL, class: "dk-ic-xs" }
                                            "Abrir explorador"
                                        }
                                    }
                                }
                            }

                            // Opción de subcarpetas iniciales de organización
                            div {
                                class: "vk-modal-subcard cursor-pointer",
                                onclick: move |_| new_vault_create_starters.set(!create_starters),
                                div {
                                    class: "flex items-center gap-2.5",
                                    input {
                                        r#type: "checkbox",
                                        checked: create_starters,
                                        onchange: move |e| new_vault_create_starters.set(e.checked()),
                                    }
                                    div {
                                        span { class: "text-xs font-semibold text-zinc-200 block", "Crear estructura de carpetas organizadas automáticamente" }
                                        span { class: "text-[11px] text-zinc-400", "Crea las carpetas: Documentos, Imágenes, Hojas de cálculo, Código y Notas" }
                                    }
                                }
                                Icon { d: ICON_SPARKLES, class: "dk-ic-sm text-amber-400 flex-none" }
                            }

                            // Nota sobre Modo Bóveda Local en Web
                            div {
                                class: "p-3 rounded-xl bg-emerald-950/25 border border-emerald-700/40 text-xs text-emerald-200/90 flex items-start gap-2.5",
                                Icon { d: ICON_CHECK, class: "dk-ic-sm text-emerald-400 flex-none mt-0.5" }
                                div {
                                    span { class: "font-semibold block", "Habilita el Modo Bóveda Local en Web" }
                                    span { class: "text-[11px] text-emerald-300/80", "Al tener al menos una bóveda creada en modo local, el Modo Bóveda Local queda habilitado automáticamente para que puedas consultarlo desde la vista Web." }
                                }
                            }
                        }

                        footer {
                            class: "vk-modal-footer",
                            button {
                                class: "dk-btn dk-btn-secondary dk-btn-sm",
                                onclick: move |_| show_create_local_modal.set(false),
                                "Cancelar"
                            }
                            button {
                                class: "dk-btn dk-btn-primary dk-btn-sm",
                                onclick: move |_| {
                                    let name_val = new_vault_name.peek().clone();
                                    let desc_val = new_vault_desc.peek().clone();
                                    let color_val = new_vault_color.peek().clone();
                                    let target_dir_val = Some(resolved_target_for_submit.clone());
                                    let starters: Vec<&str> = if *new_vault_create_starters.peek() {
                                        vec!["Documentos", "Imágenes", "Hojas de cálculo", "Código", "Notas"]
                                    } else {
                                        Vec::new()
                                    };

                                    match create_local_vault(
                                        &name_val,
                                        if desc_val.trim().is_empty() { None } else { Some(desc_val) },
                                        &color_val,
                                        target_dir_val,
                                        &starters,
                                    ) {
                                        Ok(created_detail) => {
                                            let created_id = created_detail.id.clone();
                                            let created_name = created_detail.name.clone();
                                            show_create_local_modal.set(false);
                                            new_vault_name.set(String::new());
                                            new_vault_desc.set(String::new());
                                            new_vault_custom_dir.set(None);
                                            create_vault_error.set(None);

                                            trigger_overview_reload_action(
                                                vaults_overview,
                                                available_vaults,
                                                is_overview_loading,
                                                overview_error,
                                            );

                                            active_context.set(Some(CloudResource {
                                                id: created_id.clone(),
                                                name: created_name.clone(),
                                                kind: CloudResourceKind::Vault,
                                            }));
                                            selected_folder.set(None);
                                            page_mode.set(VaultPageMode::Detail);
                                            trigger_reload_action(
                                                created_id,
                                                active_context,
                                                available_vaults,
                                                is_loading,
                                                error_msg,
                                                vault_detail,
                                                folders,
                                                files,
                                            );
                                        }
                                        Err(err) => {
                                            create_vault_error.set(Some(err));
                                        }
                                    }
                                },
                                Icon { d: ICON_PLUS, class: "dk-ic-sm" }
                                span { "Crear Bóveda Local" }
                            }
                        }
                    }
                }
            }
        } else {
            rsx! {}
        }
    };

    // Si el usuario está en la vista general, renderizar los cards de bóvedas accesibles (locales + nube)
    if *page_mode.read() == VaultPageMode::Overview {
        let raw_vaults = vaults_overview.read().clone();
        let ov_query = overview_search.read().to_lowercase().trim().to_string();
        let ov_sort = *overview_sort.read();
        let ov_source = *overview_source.read();
        let loading_ov = *is_overview_loading.read();
        let err_ov = overview_error.read().clone();

        let total_vaults_count = raw_vaults.len();
        let total_files_all: usize = raw_vaults.iter().map(|v| v.file_count.unwrap_or(0)).sum();
        let total_bytes_all: u64 = raw_vaults
            .iter()
            .map(|v| v.vault_size_bytes.or(v.size_bytes).unwrap_or(0))
            .sum();

        // Agrupar métricas por proveedor (Local Disk, Kyo Hosting, Google Drive)
        let mut local_count = 0usize;
        let mut local_bytes = 0u64;
        let mut kyo_count = 0usize;
        let mut kyo_bytes = 0u64;
        let mut drive_count = 0usize;
        let mut drive_bytes = 0u64;
        for v in &raw_vaults {
            let prov = v.provider.as_deref().unwrap_or("kyo").to_lowercase();
            let sz = v.vault_size_bytes.or(v.size_bytes).unwrap_or(0);
            if is_local_vault_id(&v.id) || prov.contains("local") {
                local_count += 1;
                local_bytes += sz;
            } else if prov.contains("drive") || prov.contains("google") {
                drive_count += 1;
                drive_bytes += sz;
            } else {
                kyo_count += 1;
                kyo_bytes += sz;
            }
        }
        let cloud_count = kyo_count + drive_count;

        let mut filtered_vaults: Vec<VaultDetail> = raw_vaults
            .into_iter()
            .filter(|v| {
                let is_local = is_local_vault_id(&v.id)
                    || v.provider.as_deref().unwrap_or("").to_lowercase().contains("local");
                match ov_source {
                    OverviewSourceFilter::All => {}
                    OverviewSourceFilter::LocalOnly => {
                        if !is_local {
                            return false;
                        }
                    }
                    OverviewSourceFilter::CloudOnly => {
                        if is_local {
                            return false;
                        }
                    }
                }

                if ov_query.is_empty() {
                    return true;
                }
                let in_name = v.name.to_lowercase().contains(&ov_query);
                let in_desc = v.description.as_deref().unwrap_or("").to_lowercase().contains(&ov_query);
                let in_prov = v.provider.as_deref().unwrap_or("").to_lowercase().contains(&ov_query);
                in_name || in_desc || in_prov
            })
            .collect();

        match ov_sort {
            OverviewSortMode::Recent => {
                filtered_vaults.sort_by(|a, b| {
                    let ta = a.updated_at.as_deref().or(a.created_at.as_deref()).unwrap_or("");
                    let tb = b.updated_at.as_deref().or(b.created_at.as_deref()).unwrap_or("");
                    tb.cmp(ta)
                });
            }
            OverviewSortMode::SizeDesc => {
                filtered_vaults.sort_by(|a, b| {
                    let sa = a.vault_size_bytes.or(a.size_bytes).unwrap_or(0);
                    let sb = b.vault_size_bytes.or(b.size_bytes).unwrap_or(0);
                    sb.cmp(&sa)
                });
            }
            OverviewSortMode::FilesDesc => {
                filtered_vaults.sort_by(|a, b| {
                    b.file_count.unwrap_or(0).cmp(&a.file_count.unwrap_or(0))
                });
            }
            OverviewSortMode::NameAsc => {
                filtered_vaults.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
        }

        return rsx! {
            div {
                class: "vk-root animate-fade-in",

                // ── Cabecera Superior de la Vista General ─────────────────────
                header {
                    class: "vk-header",
                    div {
                        class: "vk-header-left",
                        button {
                            class: "vk-back-btn",
                            title: "Regresar al Panel de control",
                            onclick: move |_| active_view.set(ActiveView::Dashboard),
                            Icon { d: ICON_ARROW_LEFT, class: "dk-ic-sm" }
                            span { "Panel" }
                        }

                        div {
                            class: "vk-vault-badge-icon",
                            Icon { d: ICON_VAULT, class: "dk-ic-lg" }
                        }

                        div {
                            class: "vk-title-area",
                            div {
                                class: "flex items-center gap-2 flex-wrap",
                                h2 { class: "vk-vault-title", "Bóvedas de Archivos" }
                                if local_count > 0 {
                                    span {
                                        class: "dk-chip dk-chip-mono dk-chip-ok",
                                        title: "El Modo Bóveda Local está activo y disponible para la vista Web",
                                        "MODO BÓVEDA LOCAL ACTIVO ({local_count})"
                                    }
                                }
                            }
                            p {
                                class: "text-xs text-zinc-400 mt-0.5",
                                if loading_ov && total_vaults_count == 0 {
                                    "Cargando bóvedas accesibles..."
                                } else {
                                    "{total_vaults_count} bóvedas ({local_count} locales · {cloud_count} en nube) · {total_files_all} archivos totales"
                                }
                            }
                        }
                    }

                    div {
                        class: "vk-header-actions",
                        div {
                            class: "vk-ov-search-wrap",
                            Icon { d: ICON_SEARCH, class: "vk-ov-search-icon" }
                            input {
                                class: "vk-ov-search-input",
                                r#type: "text",
                                placeholder: "Buscar bóvedas...",
                                value: "{overview_search}",
                                oninput: move |evt| overview_search.set(evt.value()),
                            }
                            if !overview_search.read().is_empty() {
                                button {
                                    class: "vk-ov-search-clear",
                                    title: "Limpiar búsqueda",
                                    onclick: move |_| overview_search.set(String::new()),
                                    Icon { d: ICON_X, class: "dk-ic-xs" }
                                }
                            }
                        }
                        button {
                            class: "dk-btn dk-btn-primary dk-btn-sm",
                            title: "Crear una nueva bóveda local en este equipo para organizar tu almacenamiento",
                            onclick: move |_| {
                                create_vault_error.set(None);
                                show_create_local_modal.set(true);
                            },
                            Icon { d: ICON_PLUS, class: "dk-ic-sm" }
                            span { "Nueva Bóveda Local" }
                        }
                        button {
                            class: "dk-btn dk-btn-ghost dk-btn-sm",
                            title: "Actualizar lista de bóvedas",
                            onclick: move |_| {
                                trigger_overview_reload_action(
                                    vaults_overview,
                                    available_vaults,
                                    is_overview_loading,
                                    overview_error,
                                );
                            },
                            Icon { d: ICON_REFRESH, class: if loading_ov { "dk-ic-sm animate-spin" } else { "dk-ic-sm" } }
                            span { "Actualizar" }
                        }
                    }
                }

                // ── Contenido desplazable de la Vista General ─────────────────
                div {
                    class: "vk-scroll",

                    if let Some(err) = err_ov {
                        div {
                            class: "vk-error-banner",
                            Icon { d: ICON_ALERT, class: "dk-ic-sm" }
                            span { "{err}" }
                        }
                    }

                    // Tarjetas resumen por proveedor (Almacenamiento Local + Kyo Hosting + Google Drive)
                    div {
                        class: "vk-ov-providers-grid",

                        // 1. Almacenamiento Local (En este PC)
                        div {
                            class: "vk-ov-provider-card",
                            div {
                                class: "vk-ov-provider-top",
                                div {
                                    class: "vk-ov-provider-title",
                                    span {
                                        class: "vk-ov-provider-badge-icon bg-emerald-500/15 text-emerald-400",
                                        Icon { d: ICON_HARD_DRIVE, class: "dk-ic-sm" }
                                    }
                                    span { class: "font-semibold text-sm", "Bóvedas Locales (Este PC)" }
                                    if local_count > 0 {
                                        span { class: "dk-chip dk-chip-mono dk-chip-ok", "VISIBLE EN MODO WEB" }
                                    } else {
                                        span { class: "dk-chip dk-chip-mono", "SIN CREAR" }
                                    }
                                }
                                span { class: "text-xs text-zinc-400", "{local_count} bóvedas" }
                            }
                            div {
                                class: "vk-ov-provider-meta",
                                span {
                                    class: "text-xs font-medium",
                                    if local_count > 0 {
                                        "{format_bytes(local_bytes)} organizados en disco"
                                    } else {
                                        "Crea una bóveda local para ordenar tu almacenamiento y habilitar el Modo Bóveda Local en Web"
                                    }
                                }
                                button {
                                    class: "text-xs font-semibold text-emerald-400 hover:underline cursor-pointer",
                                    onclick: move |_| {
                                        create_vault_error.set(None);
                                        show_create_local_modal.set(true);
                                    },
                                    "+ Crear local"
                                }
                            }
                            div {
                                class: "vk-ov-bar-track",
                                div {
                                    class: "vk-ov-bar-fill bg-emerald-500",
                                    style: format!("width: {}%;", if total_bytes_all > 0 && local_bytes > 0 { ((local_bytes as f64 / total_bytes_all as f64) * 100.0).clamp(6.0, 100.0) } else if local_count > 0 { 20.0 } else { 0.0 }),
                                }
                            }
                        }

                        // 2. Kyo Hosting (Nube)
                        if kyo_count > 0 || total_vaults_count > 0 {
                            div {
                                class: "vk-ov-provider-card",
                                div {
                                    class: "vk-ov-provider-top",
                                    div {
                                        class: "vk-ov-provider-title",
                                        span { class: "vk-ov-provider-badge-icon vk-ov-provider-kyo", Icon { d: ICON_VAULT, class: "dk-ic-sm" } }
                                        span { class: "font-semibold text-sm", "Kyo Hosting" }
                                        span { class: "dk-chip dk-chip-mono dk-chip-accent", "CLOUD SYNC" }
                                    }
                                    span { class: "text-xs text-zinc-400", "{kyo_count} bóvedas" }
                                }
                                div {
                                    class: "vk-ov-provider-meta",
                                    span { class: "text-xs font-medium", "{format_bytes(kyo_bytes)} usados" }
                                }
                                div {
                                    class: "vk-ov-bar-track",
                                    div {
                                        class: "vk-ov-bar-fill vk-ov-bar-kyo",
                                        style: format!("width: {}%;", if total_bytes_all > 0 && kyo_bytes > 0 { ((kyo_bytes as f64 / total_bytes_all as f64) * 100.0).clamp(4.0, 100.0) } else { 0.0 }),
                                    }
                                }
                            }
                        }

                        // 3. Google Drive (solo si existen bóvedas vinculadas a Drive)
                        if drive_count > 0 {
                            div {
                                class: "vk-ov-provider-card",
                                div {
                                    class: "vk-ov-provider-top",
                                    div {
                                        class: "vk-ov-provider-title",
                                        span { class: "vk-ov-provider-badge-icon vk-ov-provider-drive", Icon { d: ICON_FOLDER, class: "dk-ic-sm" } }
                                        span { class: "font-semibold text-sm", "Google Drive" }
                                        span { class: "dk-chip dk-chip-mono dk-chip-ok", "CONECTADO" }
                                    }
                                    span { class: "text-xs text-zinc-400", "{drive_count} bóvedas" }
                                }
                                div {
                                    class: "vk-ov-provider-meta",
                                    span { class: "text-xs font-medium", "{format_bytes(drive_bytes)} usados" }
                                }
                                div {
                                    class: "vk-ov-bar-track",
                                    div {
                                        class: "vk-ov-bar-fill vk-ov-bar-drive",
                                        style: format!("width: {}%;", if total_bytes_all > 0 && drive_bytes > 0 { ((drive_bytes as f64 / total_bytes_all as f64) * 100.0).clamp(4.0, 100.0) } else { 0.0 }),
                                    }
                                }
                            }
                        }
                    }

                    // Barra de sección: Tus repositorios y bóvedas + Filtro de Origen + Ordenamiento
                    div {
                        class: "vk-ov-section-bar",
                        div {
                            class: "flex items-center gap-3 flex-wrap",
                            div {
                                class: "flex items-center gap-2",
                                h3 { class: "text-sm font-semibold", "Tus repositorios y bóvedas" }
                                span { class: "vk-ov-count-pill", "{filtered_vaults.len()}" }
                            }

                            div {
                                class: "vk-ov-sort-group",
                                button {
                                    class: if ov_source == OverviewSourceFilter::All { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                    onclick: move |_| overview_source.set(OverviewSourceFilter::All),
                                    "Todas ({total_vaults_count})"
                                }
                                button {
                                    class: if ov_source == OverviewSourceFilter::LocalOnly { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                    onclick: move |_| overview_source.set(OverviewSourceFilter::LocalOnly),
                                    "Locales PC ({local_count})"
                                }
                                button {
                                    class: if ov_source == OverviewSourceFilter::CloudOnly { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                    onclick: move |_| overview_source.set(OverviewSourceFilter::CloudOnly),
                                    "Nube ({cloud_count})"
                                }
                            }
                        }

                        div {
                            class: "vk-ov-sort-group",
                            button {
                                class: if ov_sort == OverviewSortMode::Recent { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                onclick: move |_| overview_sort.set(OverviewSortMode::Recent),
                                "Recientes"
                            }
                            button {
                                class: if ov_sort == OverviewSortMode::SizeDesc { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                onclick: move |_| overview_sort.set(OverviewSortMode::SizeDesc),
                                "Más pesadas"
                            }
                            button {
                                class: if ov_sort == OverviewSortMode::FilesDesc { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                onclick: move |_| overview_sort.set(OverviewSortMode::FilesDesc),
                                "Más archivos"
                            }
                            button {
                                class: if ov_sort == OverviewSortMode::NameAsc { "vk-ov-sort-btn active" } else { "vk-ov-sort-btn" },
                                onclick: move |_| overview_sort.set(OverviewSortMode::NameAsc),
                                "A–Z"
                            }
                        }
                    }

                    // Estado de carga o vacío
                    if loading_ov && filtered_vaults.is_empty() {
                        div {
                            class: "vk-ov-grid",
                            for idx in 0..6 {
                                div {
                                    key: "skel-{idx}",
                                    class: "vk-vault-card vk-vault-card-skeleton",
                                    div { class: "vk-skeleton-bar vk-skel-w-50" }
                                    div { class: "vk-skeleton-bar vk-skel-w-70" }
                                    div { class: "vk-skeleton-bar vk-skel-w-100 vk-skel-thin" }
                                    div { class: "vk-skeleton-bar vk-skel-w-35" }
                                }
                            }
                        }
                    } else if filtered_vaults.is_empty() {
                        div {
                            class: "vk-empty-state",
                            Icon { d: ICON_HARD_DRIVE, class: "dk-ic-lg opacity-40" }
                            p { class: "text-sm font-medium mt-2", "Aún no hay bóvedas en esta vista" }
                            p {
                                class: "text-xs text-zinc-400 mt-1 max-w-md text-center",
                                "Crea una Bóveda Local en tu equipo para organizar tus carpetas y archivos, y activar automáticamente el Modo Bóveda Local en la versión Web."
                            }
                            div {
                                class: "flex items-center gap-2 mt-3",
                                button {
                                    class: "dk-btn dk-btn-primary dk-btn-sm",
                                    onclick: move |_| {
                                        create_vault_error.set(None);
                                        show_create_local_modal.set(true);
                                    },
                                    Icon { d: ICON_PLUS, class: "dk-ic-sm" }
                                    "Crear primera Bóveda Local"
                                }
                                if !overview_search.read().is_empty() || ov_source != OverviewSourceFilter::All {
                                    button {
                                        class: "dk-btn dk-btn-secondary dk-btn-sm",
                                        onclick: move |_| {
                                            overview_search.set(String::new());
                                            overview_source.set(OverviewSourceFilter::All);
                                        },
                                        "Ver todas"
                                    }
                                }
                            }
                        }
                    } else {
                        // Cuadrícula de Cards de Bóvedas (Locales + Remotas)
                        div {
                            class: "vk-ov-grid",
                            for v in filtered_vaults.iter() {
                                {
                                    let v_id = v.id.clone();
                                    let v_id_del = v.id.clone();
                                    let v_name = v.name.clone();
                                    let v_size = v.vault_size_bytes.or(v.size_bytes).unwrap_or(0);
                                    let v_files = v.file_count.unwrap_or(0);
                                    let pct_of_total = if total_bytes_all > 0 {
                                        ((v_size as f64 / total_bytes_all as f64) * 100.0).round() as u64
                                    } else {
                                        0
                                    };
                                    let bar_width = if total_bytes_all > 0 && v_size > 0 {
                                        ((v_size as f64 / total_bytes_all as f64) * 100.0).clamp(6.0, 100.0)
                                    } else if v_files > 0 {
                                        25.0
                                    } else {
                                        8.0
                                    };
                                    let prov_raw = v.provider.as_deref().unwrap_or("Kyo Hosting");
                                    let is_local = is_local_vault_id(&v.id) || prov_raw.to_lowercase().contains("local");
                                    let is_drive = !is_local && prov_raw.to_lowercase().contains("drive");
                                    let prov_badge = if is_local {
                                        "Local · PC"
                                    } else if is_drive {
                                        "Drive"
                                    } else {
                                        "Kyo Cloud"
                                    };
                                    let local_path_str = v.storage_account_name.clone().unwrap_or_default();
                                    let prov_subtitle = if is_local {
                                        "Almacenamiento Local en este PC"
                                    } else if is_drive {
                                        "Google Drive"
                                    } else {
                                        "Kyo Hosting"
                                    };
                                    let date_label = v.updated_at.as_deref()
                                        .or(v.created_at.as_deref())
                                        .map(format_short_date)
                                        .unwrap_or_else(|| "Activa".to_string());
                                    let top_kind_label = v.kinds.as_ref()
                                        .and_then(|ks| ks.first())
                                        .map(|k| format!("{} · {}", k.kind, k.count))
                                        .or_else(|| v.description.clone())
                                        .unwrap_or_else(|| format!("{v_files} archivos"));

                                    rsx! {
                                        div {
                                            key: "{v.id}",
                                            class: "vk-vault-card",
                                            onclick: move |_| {
                                                let picked = CloudResource {
                                                    id: v_id.clone(),
                                                    name: v_name.clone(),
                                                    kind: CloudResourceKind::Vault,
                                                };
                                                active_context.set(Some(picked));
                                                search_query.set(String::new());
                                                selected_folder.set(None);
                                                kind_filter.set("all".to_string());
                                                page_mode.set(VaultPageMode::Detail);
                                                trigger_reload_action(
                                                    v_id.clone(),
                                                    active_context,
                                                    available_vaults,
                                                    is_loading,
                                                    error_msg,
                                                    vault_detail,
                                                    folders,
                                                    files,
                                                );
                                            },

                                            div {
                                                class: "vk-vault-card-head",
                                                div {
                                                    class: "vk-vault-card-ident",
                                                    div {
                                                        class: if is_local { "vk-vault-card-folder-icon !bg-emerald-500/15 !text-emerald-400" } else { "vk-vault-card-folder-icon" },
                                                        Icon { d: if is_local { ICON_HARD_DRIVE } else { ICON_FOLDER }, class: "dk-ic-md" }
                                                    }
                                                    div {
                                                        class: "min-w-0 text-left",
                                                        h4 { class: "vk-vault-card-name", "{v.name}" }
                                                        p { class: "vk-vault-card-sub", "{prov_subtitle}" }
                                                    }
                                                }
                                                span {
                                                    class: if is_local {
                                                        "vk-vault-card-pill !bg-emerald-500/15 !text-emerald-300 !border-emerald-500/30"
                                                    } else if is_drive {
                                                        "vk-vault-card-pill vk-vault-card-pill-drive"
                                                    } else {
                                                        "vk-vault-card-pill"
                                                    },
                                                    "{prov_badge}"
                                                }
                                            }

                                            div {
                                                class: "vk-vault-card-metrics",
                                                span { class: "font-medium", "{format_bytes(v_size)} · {v_files} archivos" }
                                                span { class: "text-zinc-400", "{pct_of_total}% del total" }
                                            }

                                            div {
                                                class: "vk-ov-bar-track",
                                                div {
                                                    class: if is_local {
                                                        "vk-ov-bar-fill bg-emerald-500"
                                                    } else if is_drive {
                                                        "vk-ov-bar-fill vk-ov-bar-drive"
                                                    } else {
                                                        "vk-ov-bar-fill vk-ov-bar-kyo"
                                                    },
                                                    style: format!("width: {bar_width}%;"),
                                                }
                                            }

                                            div {
                                                class: "vk-vault-card-kind",
                                                span { class: if is_local { "vk-vault-card-dot !bg-emerald-400" } else { "vk-vault-card-dot" } }
                                                span { class: "truncate", "{top_kind_label}" }
                                            }

                                            // Dirección exacta en disco para Bóvedas Locales (al hacer clic abre el explorador de archivos predeterminado)
                                            if is_local && !local_path_str.is_empty() {
                                                {
                                                    let path_for_card_click = local_path_str.clone();
                                                    let path_for_card_label = local_path_str.clone();
                                                    rsx! {
                                                        button {
                                                            r#type: "button",
                                                            class: "w-full mt-1 px-2.5 py-1.5 rounded-lg bg-zinc-950/85 hover:bg-emerald-950/40 border border-zinc-800/90 hover:border-emerald-500/50 flex items-center justify-between gap-2 text-left cursor-pointer transition-colors group",
                                                            title: "Dirección exacta de la bóveda: {path_for_card_label} — Haz clic para abrirla en tu aplicación de explorador predeterminada",
                                                            onclick: move |e| {
                                                                e.stop_propagation();
                                                                open_path_in_explorer(std::path::Path::new(&path_for_card_click));
                                                            },
                                                            div {
                                                                class: "flex items-center gap-1.5 min-w-0",
                                                                Icon { d: ICON_FOLDER, class: "dk-ic-xs text-emerald-400 flex-none" }
                                                                span {
                                                                    class: "font-mono text-[11px] text-zinc-300 group-hover:text-emerald-200 truncate",
                                                                    "{path_for_card_label}"
                                                                }
                                                            }
                                                            span {
                                                                class: "text-[10px] font-semibold text-emerald-400 flex items-center gap-1 flex-none",
                                                                Icon { d: ICON_EXTERNAL, class: "dk-ic-xs" }
                                                                "Abrir"
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            div {
                                                class: "vk-vault-card-footer",
                                                span { "Actualizada: {date_label}" }
                                                div {
                                                    class: "flex items-center gap-2",
                                                    if is_local {
                                                        button {
                                                            class: "text-[11px] text-zinc-400 hover:text-emerald-400 px-1.5 py-0.5 rounded hover:bg-zinc-800/80",
                                                            title: "Abrir carpeta física en el Explorador de archivos",
                                                            onclick: move |e| {
                                                                e.stop_propagation();
                                                                if !local_path_str.is_empty() {
                                                                    open_path_in_explorer(std::path::Path::new(&local_path_str));
                                                                }
                                                            },
                                                            "Carpeta"
                                                        }
                                                        button {
                                                            class: "text-[11px] text-zinc-500 hover:text-rose-400 px-1.5 py-0.5 rounded hover:bg-zinc-800/80",
                                                            title: "Desvincular bóveda local de la lista (no borra tus archivos en disco)",
                                                            onclick: move |e| {
                                                                e.stop_propagation();
                                                                if remove_local_vault(&v_id_del).is_ok() {
                                                                    trigger_overview_reload_action(
                                                                        vaults_overview,
                                                                        available_vaults,
                                                                        is_overview_loading,
                                                                        overview_error,
                                                                    );
                                                                }
                                                            },
                                                            "Quitar"
                                                        }
                                                    }
                                                    span { class: "vk-vault-card-open-hint", "Abrir →" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                {render_create_local_modal}
            }
        };
    }

    // Determinar la lista de todas las bóvedas disponibles para el selector de detalle
    let all_vaults: Vec<CloudResource> = {
        let list = available_vaults.read().clone();
        if !list.is_empty() {
            list
        } else {
            account.as_ref().map(|a| a.vaults.clone()).unwrap_or_default()
        }
    };

    // Determinar la bóveda activa actual
    let current_vault_res: Option<CloudResource> = {
        let ctx = active_context.read().clone();
        ctx.filter(|c| c.kind == CloudResourceKind::Vault)
            .or_else(|| {
                vault_detail.read().as_ref().map(|d| CloudResource {
                    id: d.id.clone(),
                    name: d.name.clone(),
                    kind: CloudResourceKind::Vault,
                })
            })
            .or_else(|| available_vaults.read().first().cloned())
            .or_else(|| account.as_ref().and_then(|a| a.primary_vault_resource()))
    };
    let current_vault_id = current_vault_res.as_ref().map(|v| v.id.clone()).unwrap_or_default();
    let is_current_local = is_local_vault_id(&current_vault_id);
    let local_vault_records_now = load_local_vault_records();

    // Filtrar y ordenar archivos
    let raw_files = files.read().clone();
    let query = search_query.read().to_lowercase().trim().to_string();
    let selected_folder_val = selected_folder.read().clone();
    let current_kind = kind_filter.read().clone();
    let current_sort = *sort_mode.read();

    let mut filtered_files: Vec<VaultFile> = raw_files.into_iter().filter(|f| {
        // Filtro por carpeta
        if let Some(ref target_folder) = selected_folder_val {
            match target_folder {
                None => {
                    if f.folder_id.is_some() {
                        return false;
                    }
                }
                Some(folder_id) => {
                    if f.folder_id.as_deref() != Some(folder_id.as_str()) {
                        return false;
                    }
                }
            }
        }

        // Filtro por tipo/categoría
        if current_kind != "all" {
            let lower_kind = f.kind.to_lowercase();
            let lower_ext = f.ext.to_lowercase();
            let matches_kind = match current_kind.as_str() {
                "doc" => lower_kind == "doc" || matches!(lower_ext.as_str(), "docx" | "doc" | "pdf" | "txt" | "md" | "rtf"),
                "image" => lower_kind == "image" || matches!(lower_ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"),
                "sheet" => lower_kind == "sheet" || matches!(lower_ext.as_str(), "xlsx" | "xls" | "csv" | "tsv"),
                "code" => lower_kind == "code" || matches!(lower_ext.as_str(), "rs" | "go" | "ts" | "js" | "py" | "html" | "css" | "json"),
                "other" => lower_kind != "doc" && lower_kind != "image" && lower_kind != "sheet" && lower_kind != "code",
                _ => true,
            };
            if !matches_kind {
                return false;
            }
        }

        // Filtro por búsqueda
        if !query.is_empty() {
            let in_name = f.name.to_lowercase().contains(&query);
            let in_ext = f.ext.to_lowercase().contains(&query);
            let in_tags = f.tags.as_ref().map(|tags| tags.iter().any(|t| t.to_lowercase().contains(&query))).unwrap_or(false);
            if !in_name && !in_ext && !in_tags {
                return false;
            }
        }

        true
    }).collect();

    // Ordenamiento
    match current_sort {
        SortMode::Recent => {
            filtered_files.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
        SortMode::NameAsc => {
            filtered_files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        SortMode::NameDesc => {
            filtered_files.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()));
        }
        SortMode::SizeDesc => {
            filtered_files.sort_by(|a, b| b.size.cmp(&a.size));
        }
        SortMode::SizeAsc => {
            filtered_files.sort_by(|a, b| a.size.cmp(&b.size));
        }
    }

    // Datos calculados para los chips de métricas
    let total_files_count = files.read().len();
    let total_bytes: u64 = files.read().iter().map(|f| f.size).sum();
    let detail_opt = vault_detail.read().clone();
    let display_total_size = detail_opt.as_ref().and_then(|d| d.vault_size_bytes).unwrap_or(total_bytes);
    let folders_list = folders.read().clone();
    let folders_count = folders_list.len();

    // Nombre de la carpeta activa actual para breadcrumb
    let active_folder_name = match selected_folder_val {
        None => "Todos los archivos".to_string(),
        Some(None) => "Raíz (sin carpeta)".to_string(),
        Some(Some(ref f_id)) => {
            folders_list.iter()
                .find(|f| &f.id == f_id)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| "Carpeta".to_string())
        }
    };

    let vault_name = current_vault_res.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| "Bóveda".to_string());
    let provider_label = detail_opt.as_ref().and_then(|d| d.provider.as_deref()).unwrap_or(if is_current_local { "Local Disk" } else { "Servidor Kyō" });
    let account_label = detail_opt.as_ref().and_then(|d| d.storage_account_name.as_deref().or(d.storage_account_email.as_deref()));

    rsx! {
        div {
            class: "vk-root animate-fade-in",

            // ── Cabecera Superior de la Vista ──────────────────────────────
            header {
                class: "vk-header",
                div {
                    class: "vk-header-left",
                    button {
                        class: "vk-back-btn",
                        title: "Regresar a todas las bóvedas",
                        onclick: move |_| {
                            trigger_overview_reload_action(
                                vaults_overview,
                                available_vaults,
                                is_overview_loading,
                                overview_error,
                            );
                            page_mode.set(VaultPageMode::Overview);
                        },
                        Icon { d: ICON_ARROW_LEFT, class: "dk-ic-sm" }
                        span { "Bóvedas" }
                    }

                    div {
                        class: if is_current_local { "vk-vault-badge-icon !bg-emerald-600" } else { "vk-vault-badge-icon" },
                        Icon { d: if is_current_local { ICON_HARD_DRIVE } else { ICON_VAULT }, class: "dk-ic-lg" }
                    }

                    div {
                        class: "vk-title-area",
                        div {
                            class: "vk-vault-title-row",
                            h2 { class: "vk-vault-title", "{vault_name}" }

                            // Selector de Bóveda si el usuario tiene varias
                            if all_vaults.len() > 1 {
                                select {
                                    class: "vk-vault-select",
                                    title: "Cambiar de bóveda",
                                    onchange: move |evt| {
                                        let val = evt.value();
                                        let current_list = available_vaults.peek().clone();
                                        if let Some(target) = current_list.iter().find(|v| v.id == val).cloned()
                                            .or_else(|| all_vaults.iter().find(|v| v.id == val).cloned())
                                        {
                                            active_context.set(Some(target.clone()));
                                            selected_folder.set(None);
                                            trigger_reload_action(
                                                target.id.clone(),
                                                active_context,
                                                available_vaults,
                                                is_loading,
                                                error_msg,
                                                vault_detail,
                                                folders,
                                                files,
                                            );
                                        }
                                    },
                                    for v in all_vaults.iter() {
                                        option {
                                            key: "{v.id}",
                                            value: "{v.id}",
                                            selected: current_vault_res.as_ref().map(|cur| cur.id == v.id).unwrap_or(false),
                                            if is_local_vault_id(&v.id) {
                                                "💻 {v.name} (Local)"
                                            } else {
                                                "☁️ {v.name}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div {
                            class: "flex items-center gap-2 mt-0.5 flex-wrap",
                            span {
                                class: if is_current_local { "dk-chip dk-chip-mono dk-chip-ok" } else { "dk-chip dk-chip-mono dk-chip-accent" },
                                "{provider_label}"
                            }
                            if let Some(acc) = account_label {
                                if is_current_local {
                                    {
                                        let acc_path = acc.to_string();
                                        let acc_label = acc.to_string();
                                        rsx! {
                                            button {
                                                r#type: "button",
                                                class: "text-xs font-mono text-emerald-300 hover:text-emerald-200 bg-zinc-900/90 hover:bg-emerald-950/50 px-2 py-0.5 rounded-md border border-emerald-500/30 hover:border-emerald-400/60 flex items-center gap-1.5 max-w-md truncate cursor-pointer transition-colors",
                                                title: "Dirección exacta: {acc_label} (Haz clic para abrir en tu explorador de archivos predeterminado)",
                                                onclick: move |_| {
                                                    open_path_in_explorer(std::path::Path::new(&acc_path));
                                                },
                                                Icon { d: ICON_FOLDER, class: "dk-ic-xs text-emerald-400 flex-none" }
                                                span { class: "truncate", "{acc_label}" }
                                                Icon { d: ICON_EXTERNAL, class: "dk-ic-xs text-emerald-400 flex-none" }
                                            }
                                        }
                                    }
                                } else {
                                    span { class: "dk-muted text-xs truncate max-w-xs font-mono", title: "{acc}", "{acc}" }
                                }
                            }
                            span {
                                class: "sidebar-cloud-live ml-1",
                                span { class: "dk-dot dk-dot-ok" }
                                if is_current_local { "Bóveda Local Activa (Visible en Web)" } else { "Conectada" }
                            }
                        }
                    }
                }

                // Acciones de cabecera (incluye herramientas de organización si es Bóveda Local)
                div {
                    class: "vk-header-actions",
                    if is_current_local {
                        {
                            let v_id_import = current_vault_id.clone();
                            let active_fld_import = selected_folder.read().clone().flatten();
                            rsx! {
                                button {
                                    class: "dk-btn dk-btn-primary dk-btn-sm",
                                    title: "Añadir y organizar archivos de tu PC dentro de esta bóveda local",
                                    onclick: move |_| {
                                        if let Some(picked_files) = rfd::FileDialog::new().pick_files() {
                                            match import_files_to_local_vault(&v_id_import, active_fld_import.as_deref(), &picked_files) {
                                                Ok(n) => {
                                                    download_toast.set(Some(DownloadToast {
                                                        message: format!("Se añadieron {n} archivo(s) a la bóveda local."),
                                                        path: None,
                                                        is_error: false,
                                                    }));
                                                    trigger_reload_action(
                                                        v_id_import.clone(),
                                                        active_context,
                                                        available_vaults,
                                                        is_loading,
                                                        error_msg,
                                                        vault_detail,
                                                        folders,
                                                        files,
                                                    );
                                                }
                                                Err(e) => {
                                                    download_toast.set(Some(DownloadToast {
                                                        message: e,
                                                        path: None,
                                                        is_error: true,
                                                    }));
                                                }
                                            }
                                        }
                                    },
                                    Icon { d: ICON_UPLOAD, class: "dk-ic-sm" }
                                    span { "Añadir Archivos" }
                                }
                            }
                        }

                        button {
                            class: "dk-btn dk-btn-secondary dk-btn-sm",
                            title: "Crear una nueva carpeta para organizar archivos en esta bóveda local",
                            onclick: move |_| {
                                let cur = *show_new_folder_bar.read();
                                show_new_folder_bar.set(!cur);
                            },
                            Icon { d: ICON_PLUS, class: "dk-ic-sm" }
                            span { "Nueva Carpeta" }
                        }

                        {
                            let v_id_org = current_vault_id.clone();
                            rsx! {
                                button {
                                    class: "dk-btn dk-btn-secondary dk-btn-sm",
                                    title: "Ordenar automáticamente los archivos sueltos en carpetas por categoría (Documentos, Imágenes, Hojas, Código)",
                                    onclick: move |_| {
                                        match auto_organize_local_vault(&v_id_org) {
                                            Ok(moved) => {
                                                let msg = if moved > 0 {
                                                    format!("Auto-organización completada: {moved} archivo(s) clasificados en carpetas por tipo.")
                                                } else {
                                                    "Todos los archivos de la raíz ya están organizados en carpetas.".to_string()
                                                };
                                                download_toast.set(Some(DownloadToast {
                                                    message: msg,
                                                    path: None,
                                                    is_error: false,
                                                }));
                                                trigger_reload_action(
                                                    v_id_org.clone(),
                                                    active_context,
                                                    available_vaults,
                                                    is_loading,
                                                    error_msg,
                                                    vault_detail,
                                                    folders,
                                                    files,
                                                );
                                            }
                                            Err(e) => {
                                                download_toast.set(Some(DownloadToast {
                                                    message: e,
                                                    path: None,
                                                    is_error: true,
                                                }));
                                            }
                                        }
                                    },
                                    Icon { d: ICON_SPARKLES, class: "dk-ic-sm text-amber-400" }
                                    span { "Auto-Organizar" }
                                }
                            }
                        }

                        if let Some(ref root_p) = detail_opt.as_ref().and_then(|d| d.storage_account_name.clone()) {
                            {
                                let p_open = root_p.clone();
                                rsx! {
                                    button {
                                        class: "dk-btn dk-btn-ghost dk-btn-sm",
                                        title: "Abrir carpeta de la bóveda local en el Explorador de archivos",
                                        onclick: move |_| open_path_in_explorer(std::path::Path::new(&p_open)),
                                        Icon { d: ICON_FOLDER, class: "dk-ic-sm" }
                                        span { "Abrir Carpeta" }
                                    }
                                }
                            }
                        }
                    }

                    button {
                        class: "dk-btn dk-btn-ghost dk-btn-sm",
                        title: "Recargar contenidos de la bóveda",
                        onclick: move |_| {
                            let target = active_context.peek().as_ref().map(|c| c.id.clone()).unwrap_or_default();
                            trigger_reload_action(
                                target,
                                active_context,
                                available_vaults,
                                is_loading,
                                error_msg,
                                vault_detail,
                                folders,
                                files,
                            );
                        },
                        Icon { d: ICON_REFRESH, class: if *is_loading.read() { "dk-ic-sm animate-spin" } else { "dk-ic-sm" } }
                        span { "Actualizar" }
                    }

                    {
                        let current_pick = current_vault_res.clone();
                        rsx! {
                            button {
                                class: if is_current_local { "dk-btn dk-btn-secondary dk-btn-sm" } else { "dk-btn dk-btn-primary dk-btn-sm" },
                                title: "Iniciar chat con esta bóveda como contexto",
                                onclick: move |_| {
                                    if let Some(pick) = current_pick.clone() {
                                        active_context.set(Some(pick));
                                    }
                                    active_view.set(ActiveView::Chat);
                                },
                                Icon { d: ICON_CHAT, class: "dk-ic-sm" }
                                span { "Chatear con Bóveda" }
                            }
                        }
                    }

                    if !is_current_local {
                        button {
                            class: "dk-btn dk-btn-ghost dk-btn-sm",
                            title: "Configurar carpeta predeterminada de descargas",
                            onclick: move |_| {
                                if let Some(mut tab) = target_tab_ctx {
                                    tab.set(Some(SettingsTab::General));
                                }
                                if let Some(mut s) = settings_open {
                                    s.0.set(true);
                                }
                            },
                            Icon { d: ICON_FOLDER, class: "dk-ic-sm" }
                            span { "Descargas" }
                        }
                    }

                    button {
                        class: "dk-btn dk-btn-ghost dk-btn-sm",
                        title: "Abrir Kyō Web en el navegador",
                        onclick: move |_| open_in_browser("https://kyo.broco.dev/vaults"),
                        Icon { d: ICON_EXTERNAL, class: "dk-ic-sm" }
                        span { "Ver en Web" }
                    }
                }
            }

            // ── Barra rápida para crear carpeta en Bóveda Local ─────────────
            if is_current_local && *show_new_folder_bar.read() {
                {
                    let v_id_fld = current_vault_id.clone();
                    rsx! {
                        div {
                            class: "mx-6 mt-4 p-3 rounded-xl bg-zinc-900 border border-emerald-500/40 flex items-center justify-between gap-3 animate-fade-in",
                            div {
                                class: "flex items-center gap-2 flex-1",
                                Icon { d: ICON_FOLDER, class: "dk-ic-sm text-emerald-400 flex-none" }
                                input {
                                    class: "vk-search-input !pl-3 flex-1",
                                    r#type: "text",
                                    placeholder: "Nombre de la nueva carpeta para organizar tus archivos (ej. Facturas, Sprites, Informes)...",
                                    value: "{new_folder_name_input}",
                                    oninput: move |e| new_folder_name_input.set(e.value()),
                                }
                            }
                            div {
                                class: "flex items-center gap-2",
                                button {
                                    class: "dk-btn dk-btn-primary dk-btn-xs",
                                    onclick: move |_| {
                                        let fname = new_folder_name_input.peek().clone();
                                        match create_folder_in_local_vault(&v_id_fld, &fname) {
                                            Ok(created_fld) => {
                                                new_folder_name_input.set(String::new());
                                                show_new_folder_bar.set(false);
                                                selected_folder.set(Some(Some(created_fld.id)));
                                                trigger_reload_action(
                                                    v_id_fld.clone(),
                                                    active_context,
                                                    available_vaults,
                                                    is_loading,
                                                    error_msg,
                                                    vault_detail,
                                                    folders,
                                                    files,
                                                );
                                            }
                                            Err(e) => {
                                                download_toast.set(Some(DownloadToast {
                                                    message: e,
                                                    path: None,
                                                    is_error: true,
                                                }));
                                            }
                                        }
                                    },
                                    "Crear carpeta"
                                }
                                button {
                                    class: "dk-btn dk-btn-ghost dk-btn-xs",
                                    onclick: move |_| show_new_folder_bar.set(false),
                                    "Cancelar"
                                }
                            }
                        }
                    }
                }
            }

            // ── Toast Flotante de Descarga ──────────────────────────────────
            if let Some(toast) = download_toast.read().clone() {
                div {
                    class: if toast.is_error { "dk-flash dk-flash-err mx-6 mt-4 flex items-center justify-between" } else { "dk-flash dk-flash-ok mx-6 mt-4 flex items-center justify-between" },
                    div {
                        class: "flex items-center gap-2",
                        Icon { d: if toast.is_error { ICON_ALERT } else { ICON_CHECK }, class: "dk-ic-sm flex-none" }
                        span { "{toast.message}" }
                    }
                    div {
                        class: "flex items-center gap-2",
                        if let Some(p) = toast.path {
                            button {
                                class: "dk-btn dk-btn-ghost dk-btn-xs underline",
                                onclick: move |_| open_path_in_explorer(&p),
                                "Mostrar en carpeta"
                            }
                        }
                        button {
                            class: "p-1 hover:opacity-75",
                            onclick: move |_| download_toast.set(None),
                            Icon { d: ICON_X, class: "dk-ic-xs" }
                        }
                    }
                }
            }

            // ── Área Principal Scrollable ───────────────────────────────────
            div {
                class: "vk-scroll",

                // Alerta de Error
                if let Some(err) = error_msg.read().clone() {
                    div {
                        class: "dk-banner-err",
                        div { class: "dk-banner-icon", Icon { d: ICON_ALERT, class: "dk-ic-lg" } }
                        div {
                            class: "dk-banner-body",
                            div { class: "dk-banner-title", "No se pudo sincronizar el contenido de la bóveda" }
                            p { class: "dk-banner-desc", "{err}" }
                        }
                        button {
                            class: "dk-btn dk-btn-danger dk-btn-sm",
                            onclick: move |_| {
                                let target = active_context.peek().as_ref().map(|c| c.id.clone()).unwrap_or_default();
                                trigger_reload_action(
                                    target,
                                    active_context,
                                    available_vaults,
                                    is_loading,
                                    error_msg,
                                    vault_detail,
                                    folders,
                                    files,
                                );
                            },
                            "Reintentar"
                        }
                    }
                }

                // ── Métricas Bento de la Bóveda ─────────────────────────────
                section {
                    class: "vk-stats-grid",
                    div {
                        class: "vk-stat-card",
                        div { class: "vk-stat-label", Icon { d: ICON_FILE, class: "dk-ic-xs text-indigo-400" } "Archivos" }
                        div { class: "vk-stat-value", "{total_files_count}" }
                        div { class: "vk-stat-sub", "en esta bóveda" }
                    }

                    div {
                        class: "vk-stat-card",
                        div { class: "vk-stat-label", Icon { d: ICON_VAULT, class: "dk-ic-xs text-emerald-400" } "Almacenamiento" }
                        div { class: "vk-stat-value", "{format_bytes(display_total_size)}" }
                        div { class: "vk-stat-sub", if is_current_local { "en disco local" } else { "sincronizado" } }
                    }

                    div {
                        class: "vk-stat-card",
                        div { class: "vk-stat-label", Icon { d: ICON_FOLDER, class: "dk-ic-xs text-amber-400" } "Carpetas" }
                        div { class: "vk-stat-value", "{folders_count}" }
                        div { class: "vk-stat-sub", "organizadas" }
                    }

                    div {
                        class: "vk-stat-card",
                        div { class: "vk-stat-label", Icon { d: ICON_GRID, class: "dk-ic-xs text-purple-400" } "Categorías" }
                        div {
                            class: "flex flex-wrap gap-1 mt-1",
                            if let Some(ref kinds) = detail_opt.as_ref().and_then(|d| d.kinds.as_ref()) {
                                for k in kinds.iter() {
                                    span {
                                        key: "{k.kind}",
                                        class: "text-[11px] px-1.5 py-0.5 rounded bg-zinc-800/80 text-zinc-300 border border-zinc-700/50",
                                        "{k.count} {k.kind}"
                                    }
                                }
                            } else {
                                span { class: "text-xs text-zinc-400", "Diversos formatos" }
                            }
                        }
                    }
                }

                // ── Barra de Navegación de Carpetas (Folders Strip) ─────────
                if !folders_list.is_empty() || is_current_local {
                    section {
                        class: "vk-folders-section",
                        div {
                            class: "vk-section-heading justify-between",
                            div {
                                class: "flex items-center gap-1.5",
                                Icon { d: ICON_FOLDER, class: "dk-ic-xs" }
                                span { "Carpetas ({folders_count})" }
                            }
                            if is_current_local {
                                button {
                                    class: "text-xs text-emerald-400 hover:underline font-medium cursor-pointer normal-case tracking-normal",
                                    onclick: move |_| {
                                        let cur = *show_new_folder_bar.read();
                                        show_new_folder_bar.set(!cur);
                                    },
                                    "+ Nueva carpeta"
                                }
                            }
                        }
                        div {
                            class: "vk-folders-grid",
                            // Tarjeta para "Todas las carpetas"
                            div {
                                class: if selected_folder_val.is_none() { "vk-folder-card is-active" } else { "vk-folder-card" },
                                onclick: move |_| selected_folder.set(None),
                                div { class: "vk-folder-icon", Icon { d: ICON_FOLDER, class: "dk-ic" } }
                                span { class: "vk-folder-name", "Todas las carpetas" }
                                span { class: "vk-folder-count", "({total_files_count})" }
                            }

                            // Tarjeta para "Raíz"
                            {
                                let root_count = files.read().iter().filter(|f| f.folder_id.is_none()).count();
                                rsx! {
                                    div {
                                        class: if selected_folder_val == Some(None) { "vk-folder-card is-active" } else { "vk-folder-card" },
                                        onclick: move |_| selected_folder.set(Some(None)),
                                        div { class: "vk-folder-icon", Icon { d: ICON_FOLDER, class: "dk-ic" } }
                                        span { class: "vk-folder-name", "Raíz (sin carpeta)" }
                                        span { class: "vk-folder-count", "({root_count})" }
                                    }
                                }
                            }

                            // Cada carpeta específica
                            for fld in folders_list.iter() {
                                {
                                    let f_id = fld.id.clone();
                                    let f_name = fld.name.clone();
                                    let is_active = selected_folder_val == Some(Some(f_id.clone()));
                                    let count_in_folder = files.read().iter().filter(|f| f.folder_id.as_deref() == Some(&f_id)).count();
                                    rsx! {
                                        div {
                                            key: "{fld.id}",
                                            class: if is_active { "vk-folder-card is-active" } else { "vk-folder-card" },
                                            onclick: move |_| selected_folder.set(Some(Some(f_id.clone()))),
                                            div { class: "vk-folder-icon", Icon { d: ICON_FOLDER, class: "dk-ic" } }
                                            span { class: "vk-folder-name", title: "{f_name}", "{f_name}" }
                                            span { class: "vk-folder-count", "({count_in_folder})" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Barra de Herramientas: Búsqueda, Filtros y Vista ────────
                section {
                    class: "vk-toolbar",

                    // Buscador
                    div {
                        class: "vk-search-wrap",
                        div { class: "vk-search-icon", Icon { d: ICON_SEARCH, class: "dk-ic-sm" } }
                        input {
                            class: "vk-search-input",
                            placeholder: "Buscar por nombre, extensión o etiqueta...",
                            value: "{search_query}",
                            oninput: move |e| search_query.set(e.value()),
                        }
                        if !query.is_empty() {
                            button {
                                class: "absolute right-2.5 p-1 text-zinc-400 hover:text-zinc-200",
                                onclick: move |_| search_query.set(String::new()),
                                Icon { d: ICON_X, class: "dk-ic-xs" }
                            }
                        }
                    }

                    // Chips de Categoría
                    div {
                        class: "vk-filters-row",
                        button {
                            class: if current_kind == "all" { "vk-filter-chip is-active" } else { "vk-filter-chip" },
                            onclick: move |_| kind_filter.set("all".to_string()),
                            "Todos ({total_files_count})"
                        }
                        button {
                            class: if current_kind == "doc" { "vk-filter-chip is-active" } else { "vk-filter-chip" },
                            onclick: move |_| kind_filter.set("doc".to_string()),
                            Icon { d: ICON_FILE_TEXT, class: "dk-ic-xs mr-1 text-indigo-400" }
                            "Docs"
                        }
                        button {
                            class: if current_kind == "image" { "vk-filter-chip is-active" } else { "vk-filter-chip" },
                            onclick: move |_| kind_filter.set("image".to_string()),
                            Icon { d: ICON_IMAGE, class: "dk-ic-xs mr-1 text-purple-400" }
                            "Imágenes"
                        }
                        button {
                            class: if current_kind == "sheet" { "vk-filter-chip is-active" } else { "vk-filter-chip" },
                            onclick: move |_| kind_filter.set("sheet".to_string()),
                            Icon { d: ICON_SHEET, class: "dk-ic-xs mr-1 text-emerald-400" }
                            "Hojas"
                        }
                        button {
                            class: if current_kind == "code" { "vk-filter-chip is-active" } else { "vk-filter-chip" },
                            onclick: move |_| kind_filter.set("code".to_string()),
                            Icon { d: ICON_CODE, class: "dk-ic-xs mr-1 text-amber-400" }
                            "Código"
                        }
                    }

                    // Orden y Modo de Vista
                    div {
                        class: "flex items-center gap-2",
                        select {
                            class: "vk-vault-select",
                            value: match current_sort {
                                SortMode::Recent => "recent",
                                SortMode::NameAsc => "name_asc",
                                SortMode::NameDesc => "name_desc",
                                SortMode::SizeDesc => "size_desc",
                                SortMode::SizeAsc => "size_asc",
                            },
                            onchange: move |e| {
                                match e.value().as_str() {
                                    "recent" => sort_mode.set(SortMode::Recent),
                                    "name_asc" => sort_mode.set(SortMode::NameAsc),
                                    "name_desc" => sort_mode.set(SortMode::NameDesc),
                                    "size_desc" => sort_mode.set(SortMode::SizeDesc),
                                    "size_asc" => sort_mode.set(SortMode::SizeAsc),
                                    _ => {}
                                }
                            },
                            option { value: "recent", "Más recientes" }
                            option { value: "name_asc", "Nombre (A-Z)" }
                            option { value: "name_desc", "Nombre (Z-A)" }
                            option { value: "size_desc", "Mayor tamaño" }
                            option { value: "size_asc", "Menor tamaño" }
                        }

                        div {
                            class: "flex items-center bg-zinc-900 border border-zinc-700/60 rounded-lg p-0.5",
                            button {
                                class: if *view_mode.read() == ViewMode::Grid { "p-1.5 rounded bg-zinc-800 text-indigo-400 shadow-sm" } else { "p-1.5 rounded text-zinc-400 hover:text-zinc-200" },
                                title: "Vista en cuadrícula",
                                onclick: move |_| view_mode.set(ViewMode::Grid),
                                Icon { d: ICON_GRID, class: "dk-ic-sm" }
                            }
                            button {
                                class: if *view_mode.read() == ViewMode::List { "p-1.5 rounded bg-zinc-800 text-indigo-400 shadow-sm" } else { "p-1.5 rounded text-zinc-400 hover:text-zinc-200" },
                                title: "Vista en lista",
                                onclick: move |_| view_mode.set(ViewMode::List),
                                Icon { d: ICON_LIST, class: "dk-ic-sm" }
                            }
                        }
                    }
                }

                // ── Migas de Pan (Breadcrumbs) de Ubicación ─────────────────
                div {
                    class: "flex items-center justify-between text-xs text-zinc-400 px-1 py-0.5",
                    div {
                        class: "flex items-center gap-1.5 flex-wrap",
                        span { class: "text-zinc-500", "{vault_name}" }
                        span { class: "text-zinc-600", "/" }
                        span { class: "font-medium text-zinc-200", "{active_folder_name}" }
                        if selected_folder_val.is_some() {
                            button {
                                class: "ml-2 text-indigo-400 hover:underline text-[11px]",
                                onclick: move |_| selected_folder.set(None),
                                "Ver todos"
                            }
                        }
                    }
                    div {
                        class: "flex items-center gap-3",
                        div {
                            class: "vk-dest-chip",
                            title: format!("Descargas guardadas en: {}", downloads_dir.read().display()),
                            Icon { d: ICON_DOWNLOAD, class: "dk-ic-xs text-indigo-400 flex-none" }
                            span { class: "text-zinc-400", "Guardar en:" }
                            span {
                                class: "vk-dest-path text-zinc-200 font-mono",
                                "{downloads_dir.read().file_name().and_then(|n| n.to_str()).unwrap_or(\"Descargas\")}"
                            }
                            button {
                                class: "vk-dest-change-btn",
                                title: "Cambiar carpeta predeterminada de descargas",
                                onclick: move |_| {
                                    let current = downloads_dir.peek().clone();
                                    let picked = rfd::FileDialog::new()
                                        .set_directory(&current)
                                        .pick_folder();
                                    if let Some(new_p) = picked {
                                        if let Ok(()) = save_downloads_dir(&new_p) {
                                            downloads_dir.set(new_p);
                                        }
                                    }
                                },
                                "Cambiar"
                            }
                        }

                        span {
                            class: "text-[11px] text-zinc-400",
                            "{filtered_files.len()} archivos mostrados"
                        }
                    }
                }

                // ── Listado / Cuadrícula de Archivos ────────────────────────
                if *is_loading.read() && files.read().is_empty() {
                    // Estado de Carga (Skeletons)
                    div {
                        class: "vk-files-grid",
                        for i in 0..8 {
                            div {
                                key: "skel-{i}",
                                class: "vk-file-card animate-pulse",
                                div { class: "h-8 w-8 rounded-lg bg-zinc-800" }
                                div { class: "h-4 w-3/4 rounded bg-zinc-800" }
                                div { class: "h-3 w-1/2 rounded bg-zinc-800/60" }
                            }
                        }
                    }
                } else if filtered_files.is_empty() {
                    // Estado Vacío
                    div {
                        class: "dk-card p-12 flex flex-col items-center justify-center text-center gap-3",
                        div {
                            class: "w-12 h-12 rounded-2xl bg-zinc-800/80 border border-zinc-700/60 flex items-center justify-center text-zinc-400",
                            Icon { d: if is_current_local { ICON_UPLOAD } else { ICON_SEARCH }, class: "dk-ic-lg" }
                        }
                        h3 {
                            class: "text-sm font-semibold text-zinc-200",
                            if is_current_local && total_files_count == 0 {
                                "Tu bóveda local está lista para organizar tus archivos"
                            } else {
                                "No se encontraron archivos"
                            }
                        }
                        p {
                            class: "text-xs text-zinc-400 max-w-sm",
                            if is_current_local && total_files_count == 0 {
                                "Añade archivos desde tu equipo o crea carpetas para ordenar tu almacenamiento local."
                            } else {
                                "No hay elementos que coincidan con la búsqueda o carpeta seleccionada en esta bóveda."
                            }
                        }
                        div {
                            class: "flex items-center gap-2 mt-2",
                            if is_current_local {
                                {
                                    let v_id_empty = current_vault_id.clone();
                                    let active_fld_empty = selected_folder.read().clone().flatten();
                                    rsx! {
                                        button {
                                            class: "dk-btn dk-btn-primary dk-btn-sm",
                                            onclick: move |_| {
                                                if let Some(picked_files) = rfd::FileDialog::new().pick_files() {
                                                    if let Ok(n) = import_files_to_local_vault(&v_id_empty, active_fld_empty.as_deref(), &picked_files) {
                                                        download_toast.set(Some(DownloadToast {
                                                            message: format!("Se añadieron {n} archivo(s) a la bóveda local."),
                                                            path: None,
                                                            is_error: false,
                                                        }));
                                                        trigger_reload_action(
                                                            v_id_empty.clone(),
                                                            active_context,
                                                            available_vaults,
                                                            is_loading,
                                                            error_msg,
                                                            vault_detail,
                                                            folders,
                                                            files,
                                                        );
                                                    }
                                                }
                                            },
                                            Icon { d: ICON_UPLOAD, class: "dk-ic-sm" }
                                            "Añadir archivos del PC"
                                        }
                                    }
                                }
                            }
                            if !query.is_empty() || selected_folder_val.is_some() || current_kind != "all" {
                                button {
                                    class: "dk-btn dk-btn-secondary dk-btn-sm",
                                    onclick: move |_| {
                                        search_query.set(String::new());
                                        selected_folder.set(None);
                                        kind_filter.set("all".to_string());
                                    },
                                    "Restablecer filtros"
                                }
                            }
                        }
                    }
                } else {
                    match *view_mode.read() {
                        // 1. MODO CUADRÍCULA (Grid)
                        ViewMode::Grid => rsx! {
                            div {
                                class: "vk-files-grid",
                                for file in filtered_files.iter() {
                                    {
                                        let f_inspect = file.clone();
                                        let (icon_d, icon_cls, kind_name) = get_file_kind_meta(&file.kind, &file.ext);
                                        let folder_name_tag = file.folder_id.as_ref().and_then(|f_id| {
                                            folders_list.iter().find(|f| &f.id == f_id).map(|f| f.name.as_str())
                                        });

                                        rsx! {
                                            div {
                                                key: "{file.id}",
                                                class: "vk-file-card group",
                                                onclick: move |_| trigger_inspect_action(f_inspect.clone(), inspected_file, preview_text, is_preview_loading, preview_error),

                                                div {
                                                    class: "vk-file-card-top",
                                                    div {
                                                        class: "vk-file-icon-box {icon_cls}",
                                                        Icon { d: icon_d, class: "dk-ic" }
                                                    }
                                                    div {
                                                        class: "flex items-center gap-1.5",
                                                        if file.pinned.unwrap_or(false) {
                                                            span { class: "text-amber-400", title: "Fijado", Icon { d: ICON_PIN, class: "dk-ic-xs" } }
                                                        }
                                                        span { class: "vk-file-ext-chip", if file.ext.is_empty() { "{kind_name}" } else { ".{file.ext.to_uppercase()}" } }
                                                    }
                                                }

                                                div {
                                                    class: "vk-file-title group-hover:text-indigo-400 transition-colors",
                                                    title: "{file.name}",
                                                    "{file.name}"
                                                }

                                                div {
                                                    class: "vk-file-meta",
                                                    span { "{format_bytes(file.size)}" }
                                                    if let Some(f_tag) = folder_name_tag {
                                                        span { class: "vk-file-folder-tag", title: "Carpeta: {f_tag}", Icon { d: ICON_FOLDER, class: "dk-ic-xs" } "{f_tag}" }
                                                    } else if let Some(ref dt) = file.updated_at.as_ref().or(file.created_at.as_ref()) {
                                                        span { "{format_short_date(dt)}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },

                        // 2. MODO LISTA DETALLADA (List)
                        ViewMode::List => rsx! {
                            div {
                                class: "dk-card overflow-hidden p-0",
                                table {
                                    class: "vk-files-table",
                                    thead {
                                        tr {
                                            class: "border-b border-zinc-800 text-[11px] uppercase tracking-wider text-zinc-400 text-left bg-zinc-900/50",
                                            th { class: "p-3 font-semibold", "Nombre" }
                                            th { class: "p-3 font-semibold", "Carpeta" }
                                            th { class: "p-3 font-semibold", "Tipo" }
                                            th { class: "p-3 font-semibold", "Tamaño" }
                                            th { class: "p-3 font-semibold", "Modificado" }
                                            th { class: "p-3 font-semibold text-right", "Acción" }
                                        }
                                    }
                                    tbody {
                                        for file in filtered_files.iter() {
                                            {
                                                let f_inspect = file.clone();
                                                let f_down = file.clone();
                                                let f_down_saveas = file.clone();
                                                let (icon_d, icon_cls, kind_name) = get_file_kind_meta(&file.kind, &file.ext);
                                                let folder_name_tag = file.folder_id.as_ref().and_then(|f_id| {
                                                    folders_list.iter().find(|f| &f.id == f_id).map(|f| f.name.as_str())
                                                }).unwrap_or("Raíz");

                                                rsx! {
                                                    tr {
                                                        key: "{file.id}",
                                                        class: "vk-table-row cursor-pointer",
                                                        onclick: move |_| trigger_inspect_action(f_inspect.clone(), inspected_file, preview_text, is_preview_loading, preview_error),
                                                        td {
                                                            class: "p-3",
                                                            div {
                                                                class: "flex items-center gap-2.5",
                                                                div { class: "vk-file-icon-box {icon_cls} w-7 h-7", Icon { d: icon_d, class: "dk-ic-sm" } }
                                                                div {
                                                                    class: "flex flex-col min-w-0",
                                                                    span { class: "font-medium text-zinc-100 hover:text-indigo-400 truncate max-w-md", "{file.name}" }
                                                                    if file.pinned.unwrap_or(false) {
                                                                        span { class: "text-[10px] text-amber-400 font-medium", "📌 Fijado" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        td {
                                                            class: "p-3 text-zinc-400 text-xs",
                                                            span { class: "inline-flex items-center gap-1", Icon { d: ICON_FOLDER, class: "dk-ic-xs text-amber-500" } "{folder_name_tag}" }
                                                        }
                                                        td {
                                                            class: "p-3 text-zinc-400 text-xs",
                                                            span { class: "vk-file-ext-chip", "{kind_name} (.{file.ext})" }
                                                        }
                                                        td {
                                                            class: "p-3 text-zinc-300 font-mono text-xs",
                                                            "{format_bytes(file.size)}"
                                                        }
                                                        td {
                                                            class: "p-3 text-zinc-400 text-xs",
                                                            "{format_short_date(file.updated_at.as_deref().unwrap_or(file.created_at.as_deref().unwrap_or(\"\")))}"
                                                        }
                                                        td {
                                                            class: "p-3 text-right",
                                                            div {
                                                                class: "flex items-center justify-end gap-1.5",
                                                                {
                                                                    let is_this_downloading = downloading_files.read().contains(&file.id);
                                                                    rsx! {
                                                                        button {
                                                                            class: "dk-btn dk-btn-ghost dk-btn-xs text-zinc-300 hover:text-indigo-400",
                                                                            disabled: is_this_downloading,
                                                                            title: format!("Descarga rápida en: {}", downloads_dir.read().display()),
                                                                            onclick: move |e| {
                                                                                e.stop_propagation();
                                                                                trigger_download_action(f_down.clone(), None, downloading_files, download_toast, transfer_progress_map);
                                                                            },
                                                                            Icon { d: ICON_DOWNLOAD, class: if is_this_downloading { "dk-ic-xs animate-spin" } else { "dk-ic-xs" } }
                                                                            span { class: "text-[11px] hidden xl:inline", if is_this_downloading { "Descargando..." } else { "Descargar" } }
                                                                        }
                                                                        button {
                                                                            class: "dk-btn dk-btn-ghost dk-btn-xs text-zinc-400 hover:text-zinc-200",
                                                                            disabled: is_this_downloading,
                                                                            title: "Guardar como... (elegir carpeta)",
                                                                            onclick: move |e| {
                                                                                e.stop_propagation();
                                                                                trigger_save_as_dialog_action(f_down_saveas.clone(), downloading_files, download_toast, transfer_progress_map);
                                                                            },
                                                                            Icon { d: ICON_FOLDER, class: "dk-ic-xs" }
                                                                            span { class: "text-[11px] hidden xl:inline", "Guardar como..." }
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
                                }
                            }
                        }
                    }
                }
            }

            // ── Modal de Inspección y Previsualización de Archivo ───────────
            if let Some(file) = inspected_file.read().clone() {
                {
                    let (icon_d, icon_cls, kind_name) = get_file_kind_meta(&file.kind, &file.ext);
                    let f_down = file.clone();
                    let f_down_saveas = file.clone();
                    let f_local_org = file.clone();
                    let is_local_file = file.id.starts_with("lfile::") || is_local_vault_id(&file.vault_id);
                    let can_preview = is_previewable_text(&file.ext);
                    let folder_tag = file.folder_id.as_ref().and_then(|f_id| {
                        folders_list.iter().find(|f| &f.id == f_id).map(|f| f.name.clone())
                    }).unwrap_or_else(|| "Raíz (sin carpeta)".to_string());

                    rsx! {
                        div {
                            class: "vk-modal-backdrop animate-fade-in",
                            tabindex: "0",
                            onkeydown: move |e| {
                                if e.key() == Key::Escape {
                                    inspected_file.set(None);
                                }
                            },
                            onclick: move |_| inspected_file.set(None),

                            div {
                                class: "vk-modal-card",
                                onclick: move |e| e.stop_propagation(),

                                // Cabecera del Modal
                                header {
                                    class: "vk-modal-header",
                                    div {
                                        class: "flex items-center gap-3 min-w-0",
                                        div { class: "vk-file-icon-box {icon_cls} vk-modal-header-icon", Icon { d: icon_d, class: "dk-ic" } }
                                        div {
                                            class: "flex flex-col min-w-0",
                                            h3 { class: "font-semibold text-sm text-zinc-100 truncate", "{file.name}" }
                                            span { class: "text-xs text-zinc-400 font-mono", "{kind_name} · {format_bytes(file.size)}" }
                                        }
                                    }
                                    button {
                                        class: "vk-modal-close-btn",
                                        title: "Cerrar",
                                        onclick: move |_| inspected_file.set(None),
                                        Icon { d: ICON_X, class: "dk-ic-sm" }
                                    }
                                }

                                // Contenido del Modal
                                div {
                                    class: "vk-modal-body",

                                    // Fila de Metadatos
                                    div {
                                        class: "vk-modal-meta-grid",
                                        div {
                                            span { class: "vk-modal-meta-label", "Extensión" }
                                            p { class: "font-mono font-medium text-zinc-200 mt-0.5", ".{file.ext}" }
                                        }
                                        div {
                                            span { class: "vk-modal-meta-label", "Tamaño" }
                                            p { class: "font-mono font-medium text-zinc-200 mt-0.5", "{format_bytes(file.size)}" }
                                        }
                                        div {
                                            span { class: "vk-modal-meta-label", "Carpeta" }
                                            p { class: "font-medium text-zinc-200 mt-0.5 truncate", "{folder_tag}" }
                                        }
                                        div {
                                            span { class: "vk-modal-meta-label", "Modificado" }
                                            p { class: "font-mono text-zinc-200 mt-0.5", "{format_short_date(file.updated_at.as_deref().unwrap_or(file.created_at.as_deref().unwrap_or(\"-\")))}" }
                                        }
                                    }

                                    // Herramientas de Organización Local (mover de carpeta / fijar / eliminar) si es Bóveda Local
                                    if is_local_file {
                                        {
                                            let v_id_m = file.vault_id.clone();
                                            let f_id_m = file.id.clone();
                                            let f_name_pin = file.name.clone();
                                            let v_id_pin = file.vault_id.clone();
                                            let f_id_del = file.id.clone();
                                            let v_id_del = file.vault_id.clone();
                                            let is_pinned_now = file.pinned.unwrap_or(false);
                                            let cur_fld = file.folder_id.clone().unwrap_or_default();
                                            rsx! {
                                                div {
                                                    class: "vk-modal-subcard flex-wrap",
                                                    div {
                                                        class: "flex items-center gap-2",
                                                        Icon { d: ICON_FOLDER, class: "dk-ic-xs text-emerald-400" }
                                                        span { class: "text-xs font-semibold text-zinc-200", "Organizar en carpeta:" }
                                                        select {
                                                            class: "vk-vault-select",
                                                            value: "{cur_fld}",
                                                            onchange: move |e| {
                                                                let target_fld = e.value();
                                                                let fld_opt = if target_fld.is_empty() { None } else { Some(target_fld.as_str()) };
                                                                if let Ok(new_path) = move_local_vault_file(&v_id_m, &f_id_m, fld_opt) {
                                                                    download_toast.set(Some(DownloadToast {
                                                                        message: format!("Archivo movido a {}", new_path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("carpeta")),
                                                                        path: Some(new_path),
                                                                        is_error: false,
                                                                    }));
                                                                    inspected_file.set(None);
                                                                    trigger_reload_action(
                                                                        v_id_m.clone(),
                                                                        active_context,
                                                                        available_vaults,
                                                                        is_loading,
                                                                        error_msg,
                                                                        vault_detail,
                                                                        folders,
                                                                        files,
                                                                    );
                                                                }
                                                            },
                                                            option { value: "", "Raíz (sin carpeta)" }
                                                            for fld in folders_list.iter() {
                                                                option {
                                                                    key: "{fld.id}",
                                                                    value: "{fld.id}",
                                                                    selected: cur_fld == fld.id,
                                                                    "{fld.name}"
                                                                }
                                                            }
                                                        }
                                                    }

                                                    div {
                                                        class: "flex items-center gap-1.5",
                                                        button {
                                                            class: "dk-btn dk-btn-ghost dk-btn-xs text-amber-400",
                                                            onclick: move |_| {
                                                                if let Ok(pinned_state) = toggle_pin_local_file(&v_id_pin, &f_name_pin) {
                                                                    if let Some(cur_f) = inspected_file.write().as_mut() {
                                                                        cur_f.pinned = Some(pinned_state);
                                                                    }
                                                                    trigger_reload_action(
                                                                        v_id_pin.clone(),
                                                                        active_context,
                                                                        available_vaults,
                                                                        is_loading,
                                                                        error_msg,
                                                                        vault_detail,
                                                                        folders,
                                                                        files,
                                                                    );
                                                                }
                                                            },
                                                            Icon { d: ICON_PIN, class: "dk-ic-xs" }
                                                            if is_pinned_now { "Desfijar" } else { "Fijar" }
                                                        }
                                                        button {
                                                            class: "dk-btn dk-btn-ghost dk-btn-xs text-rose-400",
                                                            onclick: move |_| {
                                                                if delete_local_vault_file(&v_id_del, &f_id_del).is_ok() {
                                                                    inspected_file.set(None);
                                                                    trigger_reload_action(
                                                                        v_id_del.clone(),
                                                                        active_context,
                                                                        available_vaults,
                                                                        is_loading,
                                                                        error_msg,
                                                                        vault_detail,
                                                                        folders,
                                                                        files,
                                                                    );
                                                                }
                                                            },
                                                            Icon { d: ICON_TRASH, class: "dk-ic-xs" }
                                                            "Eliminar"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Si es un archivo de la Nube y el usuario tiene Bóvedas Locales, permitir enviarlo/organizarlo en una Bóveda Local
                                    if !is_local_file && !local_vault_records_now.is_empty() {
                                        {
                                            let default_local_target = {
                                                let cur_sel = selected_target_local_vault.read().clone();
                                                if cur_sel.is_empty() {
                                                    local_vault_records_now.first().map(|r| r.root_path.clone()).unwrap_or_default()
                                                } else {
                                                    cur_sel
                                                }
                                            };
                                            let f_to_local = f_local_org.clone();
                                            rsx! {
                                                div {
                                                    class: "vk-modal-subcard flex-wrap",
                                                    div {
                                                        class: "flex items-center gap-2",
                                                        Icon { d: ICON_HARD_DRIVE, class: "dk-ic-xs text-emerald-400" }
                                                        span { class: "text-xs font-semibold text-zinc-200", "Organizar en Bóveda Local:" }
                                                        select {
                                                            class: "vk-vault-select",
                                                            onchange: move |e| selected_target_local_vault.set(e.value()),
                                                            for lr in local_vault_records_now.iter() {
                                                                option {
                                                                    key: "{lr.id}",
                                                                    value: "{lr.root_path}",
                                                                    "{lr.name}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                    button {
                                                        class: "dk-btn dk-btn-secondary dk-btn-xs text-emerald-300",
                                                        onclick: move |_| {
                                                            let target_dir = PathBuf::from(&default_local_target);
                                                            trigger_download_action(
                                                                f_to_local.clone(),
                                                                Some(target_dir),
                                                                downloading_files,
                                                                download_toast,
                                                                transfer_progress_map,
                                                            );
                                                        },
                                                        Icon { d: ICON_DOWNLOAD, class: "dk-ic-xs" }
                                                        "Guardar en Bóveda Local"
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Hash SHA-256 e ID técnico
                                    if let Some(ref h) = file.hash {
                                        div {
                                            class: "vk-modal-subcard",
                                            div {
                                                span { class: "vk-modal-meta-label block", "Hash / Ubicación Física" }
                                                span { class: "font-mono text-zinc-300 text-xs select-all break-all", "{h}" }
                                            }
                                        }
                                    }

                                    // Carpeta de descarga predeterminada con botón de cambiar y abrir
                                    div {
                                        class: "vk-modal-subcard",
                                        div {
                                            class: "flex items-center gap-2 min-w-0",
                                            Icon { d: ICON_FOLDER, class: "dk-ic-xs text-indigo-400 flex-none" }
                                            div {
                                                class: "flex flex-col min-w-0",
                                                span { class: "vk-modal-meta-label", "Carpeta de descarga predeterminada" }
                                                span {
                                                    class: "font-mono text-zinc-300 text-[11px] truncate max-w-sm",
                                                    title: format!("{}", downloads_dir.read().display()),
                                                    "{downloads_dir.read().display()}"
                                                }
                                            }
                                        }
                                        div {
                                            class: "flex items-center gap-1.5 flex-none",
                                            button {
                                                class: "dk-btn dk-btn-ghost dk-btn-xs text-indigo-400 hover:text-indigo-300",
                                                title: "Elegir otra carpeta predeterminada",
                                                onclick: move |_| {
                                                    let current = downloads_dir.peek().clone();
                                                    if let Some(new_p) = rfd::FileDialog::new().set_directory(&current).pick_folder() {
                                                        if let Ok(()) = save_downloads_dir(&new_p) {
                                                            downloads_dir.set(new_p);
                                                        }
                                                    }
                                                },
                                                "Cambiar"
                                            }
                                            button {
                                                class: "dk-btn dk-btn-ghost dk-btn-xs text-zinc-400 hover:text-zinc-200",
                                                title: "Abrir carpeta en el Explorador",
                                                onclick: move |_| open_path_in_explorer(&downloads_dir.peek()),
                                                Icon { d: ICON_EXTERNAL, class: "dk-ic-xs" }
                                            }
                                        }
                                    }

                                    // Área de Vista Previa de Texto
                                    if can_preview {
                                        div {
                                            class: "flex flex-col gap-2",
                                            span { class: "text-xs font-semibold uppercase tracking-wider text-zinc-400", "Vista previa de contenido" }
                                            if *is_preview_loading.read() {
                                                div {
                                                    class: "h-48 rounded-xl bg-zinc-950/80 border border-zinc-800 flex items-center justify-center gap-2 text-zinc-400 text-xs",
                                                    Icon { d: ICON_REFRESH, class: "dk-ic-sm animate-spin" }
                                                    span { "Cargando vista previa de texto..." }
                                                }
                                            } else if let Some(ref txt) = preview_text.read().clone() {
                                                pre {
                                                    class: "max-h-64 overflow-y-auto p-4 rounded-xl bg-zinc-950/90 border border-zinc-800 text-xs font-mono text-zinc-200 whitespace-pre-wrap select-text leading-relaxed",
                                                    "{txt}"
                                                }
                                            } else if let Some(ref err) = preview_error.read().clone() {
                                                div {
                                                    class: "p-4 rounded-xl bg-red-950/20 border border-red-800/50 text-xs text-red-300",
                                                    "No se pudo cargar la vista previa: {err}"
                                                }
                                            }
                                        }
                                    } else {
                                        div {
                                            class: "vk-modal-empty-preview",
                                            Icon { d: icon_d, class: "vk-modal-preview-icon" }
                                            p { class: "text-xs text-zinc-300 font-medium", "Vista previa directa no disponible para formato .{file.ext}" }
                                            p { class: "text-xs text-zinc-400", "Puedes descargar o abrir el archivo localmente en tu equipo con su aplicación predeterminada." }
                                        }
                                    }

                                    // Barra de progreso reutilizable de descarga/subida
                                    if let Some(tp) = transfer_progress_map.read().get(&file.id).cloned() {
                                        TransferProgressBar { progress: tp }
                                    }
                                }

                                // Barra de Acciones del Modal
                                footer {
                                    class: "vk-modal-footer",
                                    button {
                                        class: "dk-btn dk-btn-secondary dk-btn-sm",
                                        onclick: move |_| inspected_file.set(None),
                                        "Cerrar"
                                    }

                                    div {
                                        class: "flex items-center gap-2 flex-wrap",
                                        if is_local_file {
                                            {
                                                let raw_p = file.id.strip_prefix("lfile::").unwrap_or(&file.id).to_string();
                                                rsx! {
                                                    button {
                                                        class: "dk-btn dk-btn-primary dk-btn-sm",
                                                        onclick: move |_| open_path_in_explorer(std::path::Path::new(&raw_p)),
                                                        Icon { d: ICON_EXTERNAL, class: "dk-ic-sm" }
                                                        span { "Mostrar en Explorador" }
                                                    }
                                                }
                                            }
                                        } else {
                                            {
                                                let is_this_downloading = downloading_files.read().contains(&file.id);
                                                rsx! {
                                                    // Botón 1: Descarga rápida en la carpeta predeterminada
                                                    button {
                                                        class: "dk-btn dk-btn-primary dk-btn-sm",
                                                        disabled: is_this_downloading,
                                                        title: format!("Descargar directamente en {}", downloads_dir.read().display()),
                                                        onclick: move |_| trigger_download_action(f_down.clone(), None, downloading_files, download_toast, transfer_progress_map),
                                                        Icon { d: ICON_DOWNLOAD, class: if is_this_downloading { "dk-ic-sm animate-spin" } else { "dk-ic-sm" } }
                                                        span { if is_this_downloading { "Descargando..." } else { "Descargar" } }
                                                    }

                                                    // Botón 2: Guardar como... (elegir carpeta personalizada)
                                                    button {
                                                        class: "dk-btn dk-btn-secondary dk-btn-sm",
                                                        disabled: is_this_downloading,
                                                        title: "Seleccionar carpeta y guardar archivo",
                                                        onclick: move |_| trigger_save_as_dialog_action(f_down_saveas.clone(), downloading_files, download_toast, transfer_progress_map),
                                                        Icon { d: ICON_FOLDER, class: "dk-ic-sm" }
                                                        span { "Guardar como..." }
                                                    }
                                                }
                                            }
                                        }

                                        button {
                                            class: "dk-btn dk-btn-secondary dk-btn-sm",
                                            title: "Preguntar sobre este archivo en el chat",
                                            onclick: move |_| {
                                                if let Some(pick) = current_vault_res.clone() {
                                                    active_context.set(Some(pick));
                                                }
                                                inspected_file.set(None);
                                                active_view.set(ActiveView::Chat);
                                            },
                                            Icon { d: ICON_CHAT, class: "dk-ic-sm" }
                                            span { "Consultar en Chat" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            {render_create_local_modal}
        }
    }
}
