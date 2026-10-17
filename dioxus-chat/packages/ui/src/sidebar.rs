//! Extensible & Collapsible Sidebar Component for AI Agent Chat Launcher
//!
//! Features:
//! - 3-tier vertical hierarchy (Top actions, Scrollable Workspaces & Chats, Bottom fixed dock)
//! - Dynamic drag resize handle (+30px max expansion, 200px min threshold, collapse to 0px)
//! - Floating expand trigger when collapsed
//! - Search filter for recent conversations
//! - Nested workspace accordion with chat sublists
//! - Full offline Lucide-style SVG icons and dark-mode styling

use dioxus::prelude::*;
use crate::models::{ActiveView, Workspace, ChatSummary, AppTheme, IsSettingsOpen};

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");

// ============================================================================
// 1. DATA MODELS
// ============================================================================

/// Profile for an AI Agent available in the launcher.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub model: String,
    pub description: String,
    pub avatar_gradient: String,
}

// ============================================================================
// 2. MAIN SIDEBAR COMPONENT
// ============================================================================

/// Props for the extensible sidebar.
#[derive(Props, Clone, PartialEq)]
pub struct SidebarProps {
    /// Callback triggered when the user requests a new conversation
    #[props(default)]
    pub on_new_chat: Option<EventHandler<()>>,
    /// Callback triggered when the user selects a chat
    #[props(default)]
    pub on_select_chat: Option<EventHandler<String>>,
}

/// Production-ready AI Agent Sidebar with drag resize and collapsible hierarchy.
#[component]
pub fn Sidebar(props: SidebarProps) -> Element {
    // Width state: Base 260px, Min 200px, Max 290px (+30px max)
    let mut width = use_signal(|| 260.0f64);
    let mut is_collapsed = use_signal(|| false);
    let mut is_resizing = use_signal(|| false);

    // Filter & Search
    let mut search_query = use_signal(String::new);

    // Active Agent State
    let agents = use_signal(|| vec![
        AgentProfile {
            id: "agent-antigravity".to_string(),
            name: "Antigravity Agent".to_string(),
            model: "Rust 2024 Engine".to_string(),
            description: "Default full-stack pairing assistant".to_string(),
            avatar_gradient: "from-indigo-600 to-violet-500".to_string(),
        },
        AgentProfile {
            id: "agent-architect".to_string(),
            name: "Systems Architect".to_string(),
            model: "Deep Reasoning 3.8".to_string(),
            description: "High-level architecture & DB design".to_string(),
            avatar_gradient: "from-emerald-600 to-teal-500".to_string(),
        },
        AgentProfile {
            id: "agent-researcher".to_string(),
            name: "Codebase Explorer".to_string(),
            model: "Semantic Search".to_string(),
            description: "Fast multi-file scanning & indexing".to_string(),
            avatar_gradient: "from-amber-500 to-orange-600".to_string(),
        },
    ]);
    let mut selected_agent_idx = use_signal(|| 0usize);
    let mut is_agent_menu_open = use_signal(|| false);

    // Workspaces and Views from Shared Context (no hardcoded chats or workspaces)
    let mut workspaces = try_use_context::<Signal<Vec<Workspace>>>().unwrap_or_else(|| use_signal(Vec::new));
    let active_view = try_use_context::<Signal<ActiveView>>();
    let is_settings_open = try_use_context::<IsSettingsOpen>();
    let sidecar_state = try_use_context::<Signal<crate::sidecar::SidecarState>>();

    // Theme state (Dark/Light) — shared via context from App
    let mut app_theme = use_context::<AppTheme>();

    // Calculated style values
    let current_width = if *is_collapsed.read() { 0.0 } else { *width.read() };
    let resizing_now = *is_resizing.read();

    rsx! {
        // High-performance bundled local stylesheet
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        // Global mouse capture overlay active only while dragging
        if resizing_now {
            div {
                class: "fixed inset-0 z-50 cursor-col-resize select-none bg-transparent",
                onmousemove: move |evt| {
                    let mouse_x = evt.client_coordinates().x;
                    // Clamp between 200px and 290px (+30px over 260px base)
                    if mouse_x < 180.0 {
                        // Dragged past collapse threshold -> collapse
                        is_collapsed.set(true);
                        width.set(260.0);
                        is_resizing.set(false);
                    } else {
                        is_collapsed.set(false);
                        let clamped = mouse_x.clamp(200.0, 290.0);
                        width.set(clamped);
                    }
                },
                onmouseup: move |_| {
                    is_resizing.set(false);
                },
            }
        }

        // Collapsed trigger button (appears when sidebar is collapsed)
        if *is_collapsed.read() {
            button {
                class: "sidebar-floating-expand-btn",
                title: "Expand Sidebar",
                onclick: move |_| {
                    is_collapsed.set(false);
                },
                svg {
                    class: "w-4 h-4",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    rect { x: "3", y: "3", width: "18", height: "18", rx: "2" }
                    path { d: "M9 3v18" }
                    path { d: "M14 9l3 3-3 3" }
                }
            }
        }

        // The Sidebar Container
        aside {
            class: if resizing_now {
                "relative flex-none h-full sidebar-container flex flex-col select-none overflow-hidden"
            } else {
                "relative flex-none h-full sidebar-container flex flex-col select-none overflow-hidden sidebar-transition"
            },
            style: "width: {current_width}px;",

            // Content container (invisible if width is 0)
            div {
                class: "flex flex-col h-full w-full min-w-[200px] overflow-hidden",

                // ------------------------------------------------------------
                // 1. TOP DOCK: Primary Actions & Agent Selector
                // ------------------------------------------------------------
                div {
                    class: "sidebar-top-section flex-none",

                    // Header row with collapse button
                    div {
                        class: "sidebar-header-row",
                        div {
                            class: "sidebar-header-badge flex items-center gap-2",
                            // Isotipo Kyo
                            svg {
                                class: "w-4 h-4 text-indigo-400 flex-none",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2.2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" }
                            }
                            span {
                                class: "font-semibold text-xs tracking-tight",
                                style: "color: var(--kyo-fg-primary);",
                                "Kyo Workspace"
                            }
                            {
                                let dot_style = if let Some(sc) = sidecar_state {
                                    let (dot, _, _) = sc.read().status.color_classes();
                                    dot
                                } else {
                                    "bg-emerald-400"
                                };
                                rsx! {
                                    div {
                                        class: "flex items-center gap-1.5 text-[10px] font-mono px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20",
                                        div { class: "w-1.5 h-1.5 rounded-full {dot_style} animate-pulse" }
                                        span { "Local Ready" }
                                    }
                                }
                            }
                        }
                        button {
                            class: "sidebar-icon-btn",
                            title: "Colapsar barra lateral",
                            onclick: move |_| {
                                is_collapsed.set(true);
                            },
                            svg {
                                class: "w-3.5 h-3.5",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                rect { x: "3", y: "3", width: "18", height: "18", rx: "2" }
                                path { d: "M9 3v18" }
                                path { d: "M16 15l-3-3 3-3" }
                            }
                        }
                    }

                    // Navigation Button: "Panel de control"
                    {
                        let is_dash = active_view.map(|av| *av.read() == ActiveView::Dashboard).unwrap_or(false);
                        rsx! {
                            button {
                                class: if is_dash { "sidebar-btn-nav active" } else { "sidebar-btn-nav" },
                                onclick: move |_| {
                                    if let Some(mut s) = is_settings_open {
                                        s.0.set(false);
                                    }
                                    if let Some(mut av) = active_view {
                                        av.set(ActiveView::Dashboard);
                                    }
                                },
                                svg {
                                    class: "w-4 h-4 text-indigo-400 flex-none",
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
                                span { "Panel de control" }
                            }
                        }
                    }

                    // Button: "Nueva conversación" (Secondary when 0 workspaces to prevent CTA conflict)
                    {
                        let has_no_ws = workspaces.read().is_empty();
                        let btn_class = if has_no_ws { "sidebar-btn-nav group" } else { "sidebar-btn-primary group" };
                        rsx! {
                            button {
                                class: "{btn_class}",
                                onclick: move |_| {
                                    if let Some(mut av) = active_view {
                                        av.set(ActiveView::Chat);
                                    }
                                    if let Some(ref handler) = props.on_new_chat {
                                        handler.call(());
                                    }
                                },
                                svg {
                                    class: "w-3.5 h-3.5 transition-transform group-hover:rotate-90",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2.5",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M12 5v14M5 12h14" }
                                }
                                span { "Nueva conversación" }
                            }
                        }
                    }

                    // Navigation Button: "Historial de chats"
                    button {
                        class: "sidebar-btn-nav",
                        onclick: move |_| {
                            // History trigger / filter reset
                            search_query.set(String::new());
                        },
                        svg {
                            class: "w-4 h-4 text-zinc-400 flex-none",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle { cx: "12", cy: "12", r: "10" }
                            polyline { points: "12 6 12 12 16 14" }
                        }
                        span { "Historial de chats" }
                    }

                    // Quick Search Bar placed contextually right after Historial de chats
                    div {
                        class: "sidebar-search-box",
                        div {
                            class: "sidebar-search-icon",
                            svg {
                                class: "w-3.5 h-3.5",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                circle { cx: "11", cy: "11", r: "8" }
                                line { x1: "21", y1: "21", x2: "16.65", y2: "16.65" }
                            }
                        }
                        input {
                            class: "sidebar-search-input",
                            placeholder: "Buscar conversaciones...",
                            value: "{search_query}",
                            oninput: move |evt| {
                                search_query.set(evt.value());
                            },
                        }
                    }

                    // Selector/Menu: "Agent"
                    div {
                        class: "relative",
                        button {
                            class: "sidebar-agent-button",
                            onclick: move |_| {
                                let curr = *is_agent_menu_open.read();
                                is_agent_menu_open.set(!curr);
                            },
                            div {
                                class: "flex items-center gap-2 overflow-hidden",
                                div {
                                    class: "relative flex items-center justify-center w-5 h-5 rounded-md bg-indigo-500/15 text-indigo-400 border border-indigo-500/30 flex-none",
                                    svg {
                                        class: "w-3 h-3 fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        rect { x: "4", y: "4", width: "16", height: "16", rx: "2" }
                                        rect { x: "9", y: "9", width: "6", height: "6" }
                                        line { x1: "9", y1: "1", x2: "9", y2: "4" }
                                        line { x1: "15", y1: "1", x2: "15", y2: "4" }
                                        line { x1: "9", y1: "20", x2: "9", y2: "23" }
                                        line { x1: "15", y1: "20", x2: "15", y2: "23" }
                                        line { x1: "20", y1: "9", x2: "23", y2: "9" }
                                        line { x1: "20", y1: "14", x2: "23", y2: "14" }
                                        line { x1: "1", y1: "9", x2: "4", y2: "9" }
                                        line { x1: "1", y1: "14", x2: "4", y2: "14" }
                                    }
                                    div {
                                        class: "absolute -top-0.5 -right-0.5 w-1.5 h-1.5 rounded-full bg-emerald-400 ring-1 ring-zinc-950 animate-pulse"
                                    }
                                }
                                {
                                    let ag_idx = *selected_agent_idx.read();
                                    let agent_list = agents.read();
                                    let current_agent = &agent_list[ag_idx.min(agent_list.len() - 1)];
                                    rsx! {
                                        span {
                                            class: "truncate font-medium text-xs",
                                            style: "color: var(--kyo-fg-primary);",
                                            "{current_agent.name}"
                                        }
                                    }
                                }
                            }
                            svg {
                                class: if *is_agent_menu_open.read() {
                                    "w-3.5 h-3.5 text-zinc-400 transition-transform rotate-180 flex-none"
                                } else {
                                    "w-3.5 h-3.5 text-zinc-400 transition-transform flex-none"
                                },
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M6 9l6 6 6-6" }
                            }
                        }

                        // Agent dropdown menu
                        if *is_agent_menu_open.read() {
                            div {
                                class: "sidebar-agent-dropdown",
                                for (idx, agent) in agents.read().iter().enumerate() {
                                    {
                                        let is_selected = idx == *selected_agent_idx.read();
                                        rsx! {
                                            button {
                                                key: "{agent.id}",
                                                class: if is_selected {
                                                    "sidebar-agent-item selected"
                                                } else {
                                                    "sidebar-agent-item"
                                                },
                                                onclick: move |_| {
                                                    selected_agent_idx.set(idx);
                                                    is_agent_menu_open.set(false);
                                                },
                                                div {
                                                    class: "w-3 h-3 mt-0.5 rounded-full bg-gradient-to-tr {agent.avatar_gradient} flex-none"
                                                }
                                                div {
                                                    class: "flex flex-col min-w-0 flex-1",
                                                    div {
                                                        class: "flex items-center justify-between gap-2",
                                                        span { class: "text-xs font-semibold text-zinc-200 truncate", "{agent.name}" }
                                                        span { class: "text-[10px] font-mono text-zinc-500 flex-none", "{agent.model}" }
                                                    }
                                                    span { class: "text-[11px] text-zinc-400 truncate", "{agent.description}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ------------------------------------------------------------
                // 2. SCROLLABLE CENTER: Quick Search + Workspaces & Chats
                // ------------------------------------------------------------
                div {
                    class: "flex-1 min-h-0 overflow-y-auto px-2.5 py-3 flex flex-col gap-3",


                    // Section Title: Espacios de Trabajo con botón rápido de añadir
                    div {
                        class: "sidebar-section-title",
                        span { "Espacios de Trabajo" }
                        div {
                            class: "flex items-center gap-1.5",
                            span { class: "workspace-badge", "{workspaces.read().len()}" }
                            button {
                                class: "p-0.5 rounded hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer",
                                title: "Crear espacio de trabajo",
                                onclick: move |_| {
                                    if let Some(mut av) = active_view {
                                        av.set(ActiveView::Dashboard);
                                    }
                                },
                                svg {
                                    class: "w-3.5 h-3.5",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2.5",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M12 5v14M5 12h14" }
                                }
                            }
                        }
                    }

                    // Workspaces List (Accordion)
                    div {
                        class: "flex flex-col gap-1.5",
                        if workspaces.read().is_empty() {
                            div {
                                class: "py-2 pl-3 text-left text-xs italic",
                                style: "color: var(--kyo-fg-muted);",
                                p { "Sin espacios creados" }
                            }
                        } else {
                            for (ws_idx, ws) in workspaces.read().iter().enumerate() {
                                {
                                    let is_expanded = ws.is_expanded;
                                    let query = search_query.read().to_lowercase();
                                    let filtered_chats: Vec<&ChatSummary> = ws.chats.iter()
                                        .filter(|c| query.is_empty() || c.title.to_lowercase().contains(&query))
                                        .collect();

                                    rsx! {
                                        div {
                                            key: "{ws.id}",
                                            class: "workspace-card",

                                            // Level 1: Workspace Header Button
                                            button {
                                                class: "workspace-header-btn",
                                                onclick: move |_| {
                                                    let mut ws_list = workspaces.write();
                                                    ws_list[ws_idx].is_expanded = !ws_list[ws_idx].is_expanded;
                                                },
                                                div {
                                                    class: "flex items-center gap-2 min-w-0 flex-1",
                                                    svg {
                                                        class: "w-3.5 h-3.5 {ws.icon_color} flex-none",
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        stroke_linecap: "round",
                                                        stroke_linejoin: "round",
                                                        path { d: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" }
                                                    }
                                                    span {
                                                        class: "truncate",
                                                        "{ws.name}"
                                                    }
                                                }
                                                div {
                                                    class: "flex items-center gap-1.5 flex-none",
                                                    span {
                                                        class: "workspace-badge",
                                                        "{filtered_chats.len()}"
                                                    }
                                                    svg {
                                                        class: if is_expanded {
                                                            "w-3 h-3 text-zinc-400 transition-transform rotate-90"
                                                        } else {
                                                            "w-3 h-3 text-zinc-400 transition-transform"
                                                        },
                                                        view_box: "0 0 24 24",
                                                        fill: "none",
                                                        stroke: "currentColor",
                                                        stroke_width: "2",
                                                        path { d: "M9 18l6-6-6-6" }
                                                    }
                                                }
                                            }

                                            // Level 2: Nested Chats List
                                            if is_expanded {
                                                div {
                                                    class: "chat-sublist",
                                                    if filtered_chats.is_empty() {
                                                        div {
                                                            class: "px-3 py-2 text-[11px] text-zinc-500 italic",
                                                            "No matching chats"
                                                        }
                                                    } else {
                                                        for chat in filtered_chats.iter() {
                                                            {
                                                                let is_active = chat.is_active;
                                                                let chat_id = chat.id.clone();
                                                                rsx! {
                                                                    button {
                                                                        key: "{chat.id}",
                                                                        class: if is_active {
                                                                            "chat-item-btn active"
                                                                        } else {
                                                                            "chat-item-btn"
                                                                        },
                                                                        onclick: move |_| {
                                                                            if let Some(mut av) = active_view {
                                                                                av.set(ActiveView::Chat);
                                                                            }
                                                                            // Set active chat
                                                                            let mut ws_list = workspaces.write();
                                                                            for w in ws_list.iter_mut() {
                                                                                for c in w.chats.iter_mut() {
                                                                                    c.is_active = c.id == chat_id;
                                                                                }
                                                                            }
                                                                            if let Some(ref handler) = props.on_select_chat {
                                                                                handler.call(chat_id.clone());
                                                                            }
                                                                        },
                                                                    div {
                                                                        class: "flex items-center gap-2 min-w-0 flex-1",
                                                                        svg {
                                                                            class: if is_active { "w-3 h-3 text-indigo-400 flex-none" } else { "w-3 h-3 text-zinc-500 flex-none" },
                                                                            view_box: "0 0 24 24",
                                                                            fill: "none",
                                                                            stroke: "currentColor",
                                                                            stroke_width: "2",
                                                                            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                                                                        }
                                                                        span {
                                                                            class: "truncate",
                                                                            "{chat.title}"
                                                                        }
                                                                    }
                                                                    span {
                                                                        class: "text-[10px] font-mono text-zinc-500 flex-none pl-1",
                                                                        "{chat.time_label}"
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

                // ------------------------------------------------------------
                // 3. BOTTOM DOCK: Settings, Theme, Docs, GitHub
                // ------------------------------------------------------------
                div {
                    class: "sidebar-dock",

                    // Settings Button
                    button {
                        class: "sidebar-dock-btn",
                        title: "Ajustes de Kyo y Conexión",
                        onclick: move |_| {
                            if let Some(mut s) = is_settings_open {
                                s.0.set(true);
                            }
                        },
                        svg {
                            class: "w-4 h-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle { cx: "12", cy: "12", r: "3" }
                            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" }
                        }
                    }

                    // Theme Toggle (Light / Dark)
                    button {
                        class: "sidebar-dock-btn",
                        title: if *app_theme.0.read() { "Cambiar a modo claro" } else { "Cambiar a modo oscuro" },
                        onclick: move |_| {
                            let curr = *app_theme.0.read();
                            app_theme.0.set(!curr);
                        },
                        if *app_theme.0.read() {
                            // Moon icon
                            svg {
                                class: "w-4 h-4 text-amber-300/90",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                            }
                        } else {
                            // Sun icon (en modo claro: amber-600 oscuro de alto contraste WCAG AA)
                            svg {
                                class: "w-4 h-4 text-amber-600 hover:text-amber-500 transition-colors",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                circle { cx: "12", cy: "12", r: "5" }
                                line { x1: "12", y1: "1", x2: "12", y2: "3" }
                                line { x1: "12", y1: "21", x2: "12", y2: "23" }
                                line { x1: "4.22", y1: "4.22", x2: "5.64", y2: "5.64" }
                                line { x1: "18.36", y1: "18.36", x2: "19.78", y2: "19.78" }
                                line { x1: "1", y1: "12", x2: "3", y2: "12" }
                                line { x1: "21", y1: "12", x2: "23", y2: "12" }
                                line { x1: "4.22", y1: "19.78", x2: "5.64", y2: "18.36" }
                                line { x1: "18.36", y1: "5.64", x2: "19.78", y2: "4.22" }
                            }
                        }
                    }

                    // Docs Button
                    a {
                        class: "sidebar-dock-btn",
                        title: "Documentación de Kyo",
                        href: "https://github.com/Grivyzom/KyoDesktop#readme",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        svg {
                            class: "w-4 h-4",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" }
                            path { d: "M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" }
                        }
                    }

                    // GitHub External Link Button
                    a {
                        class: "sidebar-dock-btn",
                        title: "Repositorio en GitHub",
                        href: "https://github.com/Grivyzom/KyoDesktop",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        svg {
                            class: "w-4 h-4",
                            view_box: "0 0 24 24",
                            fill: "currentColor",
                            path {
                                fill_rule: "evenodd",
                                clip_rule: "evenodd",
                                d: "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"
                            }
                        }
                    }
                }
            }

            // ----------------------------------------------------------------
            // 4. DRAG RESIZE HANDLE: Right edge controller (+30px max, collapse)
            // ----------------------------------------------------------------
            if !*is_collapsed.read() {
                div {
                    class: "sidebar-drag-handle",
                    onmousedown: move |evt| {
                        evt.stop_propagation();
                        is_resizing.set(true);
                    },
                }
            }
        }
    }
}
