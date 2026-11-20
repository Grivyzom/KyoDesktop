use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Contador atómico monotónico para garantizar IDs únicos de workspace.
static WS_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Vista activa en el área principal de la aplicación.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveView {
    Dashboard,
    Chat,
    Vault,
}

/// Representa un espacio de trabajo que agrupa conversaciones y fuentes de contexto.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon_color: String,
    pub is_expanded: bool,
    pub chats: Vec<ChatSummary>,
}

impl Workspace {
    pub fn new(name: impl Into<String>, description: impl Into<String>, icon_color: impl Into<String>) -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let seq = WS_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!("ws-{ts}-{seq}"),
            name: name.into(),
            description: description.into(),
            icon_color: icon_color.into(),
            is_expanded: true,
            chats: Vec::new(),
        }
    }
}

/// Metadatos resumidos de una conversación individual dentro de un espacio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub time_label: String,
    pub is_active: bool,
}

/// Preferencia de tema configurada por el usuario (System = sigue a Windows 11)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemePreference {
    #[default]
    System,
    Dark,
    Light,
}

/// Preferencia de tema activa en la sesión
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppThemeMode(pub dioxus::prelude::Signal<ThemePreference>);

/// Estado global del tema visual resuelto (true = Dark, false = Light)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppTheme(pub dioxus::prelude::Signal<bool>);

/// Control de visibilidad del modal de ajustes de Kyo
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsSettingsOpen(pub dioxus::prelude::Signal<bool>);

/// Carpeta global de descargas compartida entre vistas y modales
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppDownloadsDir(pub dioxus::prelude::Signal<std::path::PathBuf>);

