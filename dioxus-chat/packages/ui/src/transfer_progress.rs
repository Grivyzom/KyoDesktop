//! Componente reutilizable de barra de progreso para transferencias (descargas y subidas).

use dioxus::prelude::*;
use crate::dashboard::{Icon, ICON_ALERT, ICON_CHECK, ICON_REFRESH};
use crate::vault::{format_bytes, ICON_DOWNLOAD};

const ICON_UPLOAD: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M17 8l-5-5-5 5 M12 3v12";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Download,
    Upload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Active,
    Completed,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransferProgress {
    pub file_id: String,
    pub file_name: String,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub transferred_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: Option<String>,
}

impl TransferProgress {
    pub fn percent(&self) -> Option<f64> {
        match self.status {
            TransferStatus::Completed => Some(100.0),
            _ => self.total_bytes.and_then(|total| {
                if total > 0 {
                    Some(((self.transferred_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0))
                } else {
                    None
                }
            }),
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == TransferStatus::Active
    }
}

/// Barra de progreso reutilizable para mostrar el estado actual de una descarga o subida.
#[component]
pub fn TransferProgressBar(
    progress: TransferProgress,
    #[props(default = false)] compact: bool,
) -> Element {
    let pct_opt = progress.percent();
    let is_indeterminate = progress.status == TransferStatus::Active && pct_opt.is_none();

    let status_label = match (progress.direction, progress.status) {
        (TransferDirection::Download, TransferStatus::Active) => "Descargando archivo...",
        (TransferDirection::Download, TransferStatus::Completed) => "Descarga completada",
        (TransferDirection::Download, TransferStatus::Error) => "Error en la descarga",
        (TransferDirection::Upload, TransferStatus::Active) => "Subiendo archivo...",
        (TransferDirection::Upload, TransferStatus::Completed) => "Subida completada",
        (TransferDirection::Upload, TransferStatus::Error) => "Error en la subida",
    };

    let bytes_label = match progress.total_bytes {
        Some(total) if total > 0 => {
            format!("{} / {}", format_bytes(progress.transferred_bytes), format_bytes(total))
        }
        _ => format_bytes(progress.transferred_bytes),
    };

    let pct_label = pct_opt.map(|p| format!("{p:.0}%"));

    let card_mod = match progress.status {
        TransferStatus::Active => "is-active",
        TransferStatus::Completed => "is-completed",
        TransferStatus::Error => "is-error",
    };

    let fill_mod = match progress.status {
        TransferStatus::Active if is_indeterminate => "is-indeterminate",
        TransferStatus::Active => "",
        TransferStatus::Completed => "is-completed",
        TransferStatus::Error => "is-error",
    };

    let fill_width = match (progress.status, pct_opt) {
        (TransferStatus::Completed, _) => "100%".to_string(),
        (_, Some(p)) => format!("{p:.1}%"),
        (_, None) => "40%".to_string(),
    };

    let dir_icon = match progress.direction {
        TransferDirection::Download => ICON_DOWNLOAD,
        TransferDirection::Upload => ICON_UPLOAD,
    };

    rsx! {
        div {
            class: if compact { "kyo-progress-card is-compact {card_mod}" } else { "kyo-progress-card {card_mod}" },
            div {
                class: "kyo-progress-head",
                div {
                    class: "kyo-progress-title-row",
                    span {
                        class: "kyo-progress-icon {card_mod}",
                        match progress.status {
                            TransferStatus::Active => rsx! { Icon { d: ICON_REFRESH, class: "dk-ic-xs animate-spin" } },
                            TransferStatus::Completed => rsx! { Icon { d: ICON_CHECK, class: "dk-ic-xs" } },
                            TransferStatus::Error => rsx! { Icon { d: ICON_ALERT, class: "dk-ic-xs" } },
                        }
                    }
                    span { class: "kyo-progress-status", "{status_label}" }
                }

                div {
                    class: "kyo-progress-metrics",
                    span { class: "font-mono", "{bytes_label}" }
                    if let Some(ref pct) = pct_label {
                        span { class: "kyo-progress-pct {card_mod}", "{pct}" }
                    } else {
                        Icon { d: dir_icon, class: "dk-ic-xs opacity-70" }
                    }
                }
            }

            div {
                class: "kyo-progress-track",
                div {
                    class: "kyo-progress-fill {fill_mod}",
                    style: if is_indeterminate { String::new() } else { format!("width: {fill_width};") },
                }
            }

            if let Some(ref msg) = progress.message {
                if !msg.trim().is_empty() {
                    p { class: "kyo-progress-msg {card_mod}", "{msg}" }
                }
            }
        }
    }
}
