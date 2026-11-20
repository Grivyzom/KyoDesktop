use dioxus::prelude::*;
use ui::{
    fetch_local_vaults_detailed, format_bytes, get_local_vault_mode_status, open_path_in_explorer,
    Echo, Hero,
};

#[component]
pub fn Home() -> Element {
    let local_status = get_local_vault_mode_status();
    let mut show_local_vault_mode = use_signal(|| false);

    rsx! {
        Hero {}

        // Modo Bóveda Local en Web: SOLO se muestra si el usuario tiene creada al menos una bóveda en modo local
        if local_status.has_local_vault {
            div {
                style: "max-width: 720px; margin: 18px auto; padding: 16px 20px; border-radius: 14px; background: rgba(16, 185, 129, 0.08); border: 1px solid rgba(16, 185, 129, 0.35); color: #e4e4e7;",
                div {
                    style: "display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap;",
                    div {
                        span {
                            style: "font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; color: #34d399; display: block;",
                            "Modo Bóveda Local Detectado"
                        }
                        h3 {
                            style: "margin: 4px 0 2px; font-size: 15px; font-weight: 600;",
                            "{local_status.local_vaults_count} bóveda(s) local(es) activa(s) · {local_status.total_files} archivos ({format_bytes(local_status.total_bytes)})"
                        }
                    }
                    button {
                        style: "padding: 8px 14px; border-radius: 8px; background: #10b981; color: #09090b; font-weight: 600; font-size: 12px; border: none; cursor: pointer;",
                        onclick: move |_| {
                            let cur = *show_local_vault_mode.read();
                            show_local_vault_mode.set(!cur);
                        },
                        if *show_local_vault_mode.read() {
                            "Ocultar Modo Bóveda Local"
                        } else {
                            "Abrir Modo Bóveda Local"
                        }
                    }
                }

                if *show_local_vault_mode.read() {
                    div {
                        style: "margin-top: 14px; display: grid; gap: 10px;",
                        for v in fetch_local_vaults_detailed() {
                            {
                                let exact_path = v.storage_account_name.clone().unwrap_or_default();
                                let exact_path_click = exact_path.clone();
                                rsx! {
                                    div {
                                        key: "{v.id}",
                                        style: "padding: 12px 14px; border-radius: 10px; background: rgba(9, 9, 11, 0.65); border: 1px solid rgba(255, 255, 255, 0.1); display: flex; justify-content: space-between; align-items: center; gap: 12px;",
                                        div {
                                            style: "min-width: 0;",
                                            strong { style: "font-size: 13px; display: block;", "{v.name}" }
                                            if !exact_path.is_empty() {
                                                button {
                                                    r#type: "button",
                                                    title: "Clic para abrir en tu explorador de archivos predeterminado",
                                                    style: "margin-top: 4px; padding: 3px 8px; border-radius: 6px; background: rgba(16, 185, 129, 0.12); border: 1px solid rgba(16, 185, 129, 0.3); font-family: monospace; font-size: 11px; color: #6ee7b7; cursor: pointer;",
                                                    onclick: move |_| {
                                                        open_path_in_explorer(std::path::Path::new(&exact_path_click));
                                                    },
                                                    "📁 {exact_path}"
                                                }
                                            }
                                        }
                                        span {
                                            style: "font-size: 12px; color: #34d399; font-weight: 600; flex-shrink: 0;",
                                            "{v.file_count.unwrap_or(0)} archivos · {format_bytes(v.vault_size_bytes.unwrap_or(0))}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Echo {}
    }
}
