use dioxus::prelude::*;
use crate::models::{ActiveView, Workspace};

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");

/// Vista principal de Dashboard para gestionar y visualizar los espacios de trabajo.
#[component]
pub fn Dashboard() -> Element {
    let mut active_view = use_context::<Signal<ActiveView>>();
    let mut workspaces = use_context::<Signal<Vec<Workspace>>>();

    let mut is_modal_open = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut new_desc = use_signal(String::new);
    let mut selected_color = use_signal(|| "text-indigo-400".to_string());

    let ws_count = workspaces.read().len();

    let color_options = vec![
        ("Indigo", "text-indigo-400", "bg-indigo-500"),
        ("Esmeralda", "text-emerald-400", "bg-emerald-500"),
        ("Violeta", "text-violet-400", "bg-violet-500"),
        ("Ámbar", "text-amber-400", "bg-amber-500"),
        ("Rosa", "text-rose-400", "bg-rose-500"),
        ("Cian", "text-cyan-400", "bg-cyan-500"),
    ];

    let mut create_workspace = move || {
        let name = new_name.read().trim().to_string();
        if name.is_empty() {
            return;
        }
        let desc = new_desc.read().trim().to_string();
        let color = selected_color.read().clone();

        let new_ws = Workspace::new(name, desc, color);
        workspaces.write().push(new_ws);

        // Limpiar inputs y cerrar modal
        new_name.set(String::new());
        new_desc.set(String::new());
        is_modal_open.set(false);

        // Navegar directamente al chat para comenzar
        active_view.set(ActiveView::Chat);
    };

    rsx! {
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        div {
            class: "flex flex-col h-full w-full antialiased font-sans select-none overflow-hidden relative",
            style: "background-color: var(--kyo-surface-0); color: var(--kyo-fg-primary);",

            // ── Cabecera del Dashboard ───────────────────────────────────────
            header {
                class: "flex-none h-14 border-b backdrop-blur-md px-6 flex items-center justify-between z-10",
                style: "background-color: var(--kyo-surface-1); border-color: var(--kyo-border-medium);",
                div {
                    class: "flex items-center gap-3",
                    div {
                        class: "flex items-center justify-center w-8 h-8 rounded-lg bg-gradient-to-tr from-indigo-600 to-violet-500 text-white shadow-md shadow-indigo-500/20",
                        svg {
                            class: "w-4 h-4 fill-none stroke-current stroke-2",
                            view_box: "0 0 24 24",
                            path { d: "M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" }
                        }
                    }
                    div {
                        div { class: "flex items-center gap-2",
                            span { class: "font-semibold text-sm tracking-tight", style: "color: var(--kyo-fg-primary);", "Espacios de Trabajo" }
                            span { class: "px-2 py-0.5 text-[10px] font-medium tracking-wide rounded-full", style: "background-color: var(--kyo-surface-2); color: var(--kyo-fg-tertiary); border: 1px solid var(--kyo-border-hover);", "{ws_count} espacios" }
                        }
                        span { class: "text-[11px] font-mono", style: "color: var(--kyo-fg-tertiary);", "Kyo Desktop Dashboard" }
                    }
                }

                if ws_count > 0 {
                    button {
                        class: "flex items-center gap-2 px-3.5 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold shadow-md shadow-indigo-600/30 transition-all cursor-pointer",
                        onclick: move |_| is_modal_open.set(true),
                        svg {
                            class: "w-3.5 h-3.5",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2.5",
                            path { d: "M12 5v14M5 12h14" }
                        }
                        span { "Nuevo Espacio" }
                    }
                }
            }

            // ── Contenido Principal ───────────────────────────────────────────
            main {
                class: "flex-1 overflow-y-auto p-6 md:p-10 flex flex-col justify-center",

                if ws_count == 0 {
                    // ── ESTADO VACÍO CENTRADO (Empty State) ──────────────────
                    div {
                        class: "flex flex-col items-center justify-center text-center max-w-md mx-auto my-auto space-y-6 animate-fade-in",

                        div {
                            class: "relative flex items-center justify-center w-20 h-20 rounded-3xl shadow-2xl shadow-indigo-500/10",
                            style: "background: linear-gradient(to bottom, var(--kyo-surface-2), var(--kyo-surface-1)); border: 1px solid var(--kyo-border-hover);",
                            div {
                                class: "absolute -inset-1 rounded-3xl bg-indigo-500/20 blur-xl pointer-events-none"
                            }
                            svg {
                                class: "w-10 h-10 text-indigo-400 relative z-10 fill-none stroke-current stroke-1.5",
                                view_box: "0 0 24 24",
                                path { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" }
                                path { d: "M12 11v6M9 14h6" }
                            }
                        }

                        div {
                            class: "space-y-2",
                            h2 {
                                class: "text-xl font-bold tracking-tight",
                                style: "color: var(--kyo-fg-primary);",
                                "No tienes ningún espacio de trabajo"
                            }
                            p {
                                class: "text-xs md:text-sm leading-relaxed max-w-sm",
                                style: "color: var(--kyo-fg-tertiary);",
                                "Crea tu primer espacio para organizar tus documentos, fuentes de conocimiento y comenzar a chatear con el agente de Kyo."
                            }
                        }

                        button {
                            class: "flex items-center gap-2.5 px-6 py-3 rounded-xl bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white text-sm font-semibold shadow-lg shadow-indigo-500/25 transition-all transform hover:-translate-y-0.5 cursor-pointer",
                            onclick: move |_| is_modal_open.set(true),
                            svg {
                                class: "w-4 h-4",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2.5",
                                stroke_linecap: "round",
                                path { d: "M12 5v14M5 12h14" }
                            }
                            span { "Crear mi primer espacio" }
                        }
                    }
                } else {
                    // ── LISTADO DE WORKSPACES (Grid State) ───────────────────
                    div {
                        class: "max-w-5xl mx-auto w-full space-y-6",
                        div {
                            class: "flex items-center justify-between pb-2 border-b",
                            style: "border-color: var(--kyo-border-subtle);",
                            div {
                                h2 { class: "text-base font-semibold", style: "color: var(--kyo-fg-secondary);", "Tus Espacios" }
                                p { class: "text-xs", style: "color: var(--kyo-fg-tertiary);", "Selecciona un espacio para abrir sus conversaciones o iniciar una nueva." }
                            }
                        }

                        div {
                            class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
                            for (idx, ws) in workspaces.read().iter().enumerate() {
                                {
                                    let ws_id = ws.id.clone();
                                    let ws_name = ws.name.clone();
                                    let ws_desc = if ws.description.is_empty() {
                                        "Espacio de trabajo para interactuar con Kyo.".to_string()
                                    } else {
                                        ws.description.clone()
                                    };
                                    let icon_color = ws.icon_color.clone();
                                    let chats_len = ws.chats.len();

                                    rsx! {
                                        div {
                                            key: "{ws_id}",
                                            class: "group p-5 rounded-2xl transition-all duration-150 flex flex-col justify-between shadow-sm hover:shadow-lg hover:shadow-indigo-500/5",
                                            style: "background-color: var(--kyo-surface-1); border: 1px solid var(--kyo-border-medium);",

                                            div {
                                                class: "space-y-3",
                                                div {
                                                    class: "flex items-start justify-between",
                                                    div {
                                                        class: "p-2.5 rounded-xl {icon_color}",
                                                        style: "background-color: var(--kyo-surface-2); border: 1px solid var(--kyo-border-hover);",
                                                        svg {
                                                            class: "w-5 h-5 fill-none stroke-current stroke-2",
                                                            view_box: "0 0 24 24",
                                                            path { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" }
                                                        }
                                                    }
                                                    button {
                                                        class: "p-1 rounded-lg hover:bg-rose-500/10 transition-colors",
                                                        style: "color: var(--kyo-fg-muted);",
                                                        title: "Eliminar espacio",
                                                        onclick: move |_| {
                                                            workspaces.write().remove(idx);
                                                        },
                                                        svg {
                                                            class: "w-4 h-4 fill-none stroke-current stroke-1.5",
                                                            view_box: "0 0 24 24",
                                                            path { d: "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" }
                                                        }
                                                    }
                                                }

                                                div {
                                                    h3 { class: "font-semibold text-sm group-hover:text-indigo-300 transition-colors", style: "color: var(--kyo-fg-primary);", "{ws_name}" }
                                                    p { class: "text-xs line-clamp-2 leading-relaxed", style: "color: var(--kyo-fg-tertiary);", "{ws_desc}" }
                                                }
                                            }

                                            div {
                                                class: "pt-4 mt-4 border-t flex items-center justify-between",
                                                style: "border-color: var(--kyo-border-subtle);",
                                                span {
                                                    class: "text-[11px] font-mono",
                                                    style: "color: var(--kyo-fg-muted);",
                                                    "{chats_len} conversaciones"
                                                }
                                                button {
                                                    class: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600/20 hover:bg-indigo-600 text-indigo-300 hover:text-white border border-indigo-500/30 text-xs font-medium transition-all cursor-pointer",
                                                    onclick: move |_| {
                                                        active_view.set(ActiveView::Chat);
                                                    },
                                                    span { "Abrir chat" }
                                                    svg {
                                                        class: "w-3 h-3",
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
            }

            // ── MODAL PARA CREAR NUEVO ESPACIO ────────────────────────────────
            if *is_modal_open.read() {
                div {
                    class: "fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-fade-in",
                    onclick: move |_| is_modal_open.set(false),

                    div {
                        class: "w-full max-w-md rounded-2xl shadow-2xl p-6 space-y-5 select-none",
                        style: "background-color: var(--kyo-surface-1); border: 1px solid var(--kyo-border-hover);",
                        onclick: move |evt| evt.stop_propagation(),

                        // Título del Modal
                        div {
                            class: "flex items-center justify-between pb-2 border-b",
                            style: "border-color: var(--kyo-border-default);",
                            div {
                                class: "flex items-center gap-2",
                                svg {
                                    class: "w-5 h-5 text-indigo-400 fill-none stroke-current stroke-2",
                                    view_box: "0 0 24 24",
                                    path { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" }
                                }
                                h3 { class: "text-base font-bold", style: "color: var(--kyo-fg-primary);", "Crear nuevo espacio" }
                            }
                            button {
                                class: "p-1 rounded-lg",
                                style: "color: var(--kyo-fg-muted);",
                                onclick: move |_| is_modal_open.set(false),
                                svg {
                                    class: "w-4 h-4 fill-none stroke-current stroke-2",
                                    view_box: "0 0 24 24",
                                    path { d: "M6 18L18 6M6 6l12 12" }
                                }
                            }
                        }

                        // Formulario
                        div {
                            class: "space-y-4 text-xs",

                            div {
                                class: "space-y-1.5",
                                label { class: "font-medium", style: "color: var(--kyo-fg-secondary);", "Nombre del espacio" }
                                input {
                                    class: "w-full px-3 py-2 rounded-xl focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/50 text-xs",
                                    style: "background-color: var(--kyo-surface-0); border: 1px solid var(--kyo-border-hover); color: var(--kyo-fg-primary);",
                                    placeholder: "ej. Proyectos de Investigación, Estudio...",
                                    value: "{new_name}",
                                    autofocus: true,
                                    oninput: move |evt| new_name.set(evt.value()),
                                    onkeydown: move |evt: KeyboardEvent| {
                                        if evt.key() == Key::Enter {
                                            create_workspace();
                                        }
                                    }
                                }
                            }

                            div {
                                class: "space-y-1.5",
                                label { class: "font-medium", style: "color: var(--kyo-fg-secondary);", "Descripción corta (opcional)" }
                                input {
                                    class: "w-full px-3 py-2 rounded-xl focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/50 text-xs",
                                    style: "background-color: var(--kyo-surface-0); border: 1px solid var(--kyo-border-hover); color: var(--kyo-fg-primary);",
                                    placeholder: "ej. Apuntes, código y sesiones de IA dedicadas",
                                    value: "{new_desc}",
                                    oninput: move |evt| new_desc.set(evt.value()),
                                    onkeydown: move |evt: KeyboardEvent| {
                                        if evt.key() == Key::Enter {
                                            create_workspace();
                                        }
                                    }
                                }
                            }

                            div {
                                class: "space-y-1.5",
                                label { class: "font-medium", style: "color: var(--kyo-fg-secondary);", "Color del ícono" }
                                div {
                                    class: "flex items-center gap-2 pt-1",
                                    for (_label, text_class, bg_class) in color_options.iter() {
                                        {
                                            let is_sel = *selected_color.read() == *text_class;
                                            let tc = text_class.to_string();
                                            rsx! {
                                                button {
                                                    key: "{text_class}",
                                                    type: "button",
                                                    class: if is_sel {
                                                        "w-6 h-6 rounded-full {bg_class} ring-2 ring-white ring-offset-2 ring-offset-zinc-900 transition-all cursor-pointer"
                                                    } else {
                                                        "w-6 h-6 rounded-full {bg_class} opacity-60 hover:opacity-100 transition-all cursor-pointer"
                                                    },
                                                    onclick: move |_| selected_color.set(tc.clone()),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Acciones
                        div {
                            class: "flex items-center justify-end gap-2 pt-2 border-t",
                            style: "border-color: var(--kyo-border-default);",
                            button {
                                class: "px-4 py-2 rounded-xl text-xs font-medium transition-colors",
                                style: "color: var(--kyo-fg-tertiary);",
                                onclick: move |_| is_modal_open.set(false),
                                "Cancelar"
                            }
                            button {
                                class: "px-4 py-2 rounded-xl text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/30 transition-all cursor-pointer",
                                onclick: move |_| create_workspace(),
                                "Crear espacio"
                            }
                        }
                    }
                }
            }
        }
    }
}
