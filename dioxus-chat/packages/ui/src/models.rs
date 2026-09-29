use serde::{Deserialize, Serialize};

/// Vista activa en el área principal de la aplicación.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveView {
    Dashboard,
    Chat,
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
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            id: format!("ws-{ts}"),
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

/// Estado global del tema visual (true = Dark, false = Light)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppTheme(pub dioxus::prelude::Signal<bool>);

/// Control de visibilidad del modal de ajustes de Kyo
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsSettingsOpen(pub dioxus::prelude::Signal<bool>);

