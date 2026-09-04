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

const CHAT_CSS: Asset = asset!("/assets/styling/chat.css");

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

/// A workspace grouping conversations and contexts.
#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon_color: String,
    pub is_expanded: bool,
    pub chats: Vec<ChatSummary>,
}

/// Metadata summary of an individual conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub time_label: String,
    pub is_active: bool,
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

    // Workspaces and Nested Chats
    let mut workspaces = use_signal(|| vec![
        Workspace {
            id: "ws-ideas".to_string(),
            name: "Grivyzom Ideas".to_string(),
            icon_color: "text-indigo-400".to_string(),
            is_expanded: true,
            chats: vec![
                ChatSummary {
                    id: "chat-1".to_string(),
                    title: "Dioxus Chat Architecture".to_string(),
                    time_label: "Active".to_string(),
                    is_active: true,
                },
                ChatSummary {
                    id: "chat-2".to_string(),
                    title: "Custom Window Controls & Tao".to_string(),
                    time_label: "2h ago".to_string(),
                    is_active: false,
                },
                ChatSummary {
                    id: "chat-3".to_string(),
                    title: "Tailwind Native Bundling".to_string(),
                    time_label: "Yesterday".to_string(),
                    is_active: false,
                },
            ],
        },
        Workspace {
            id: "ws-tauri".to_string(),
            name: "Tauri v2 Migration".to_string(),
            icon_color: "text-emerald-400".to_string(),
            is_expanded: false,
            chats: vec![
                ChatSummary {
                    id: "chat-4".to_string(),
                    title: "IPC Protocols & Permissions".to_string(),
                    time_label: "3d ago".to_string(),
                    is_active: false,
                },
                ChatSummary {
                    id: "chat-5".to_string(),
                    title: "Mobile iOS/Android Build".to_string(),
                    time_label: "1w ago".to_string(),
                    is_active: false,
                },
            ],
        },
        Workspace {
            id: "ws-research".to_string(),
            name: "LLM Agent Workflows".to_string(),
            icon_color: "text-violet-400".to_string(),
            is_expanded: false,
            chats: vec![
                ChatSummary {
                    id: "chat-6".to_string(),
                    title: "Tool Calling & Evaluation".to_string(),
                    time_label: "2w ago".to_string(),
                    is_active: false,
                },
            ],
        },
    ]);

    // Theme state (Dark/Light)
    let mut is_dark_theme = use_signal(|| true);

    // Calculated style values
    let current_width = if *is_collapsed.read() { 0.0 } else { *width.read() };
    let resizing_now = *is_resizing.read();

    rsx! {
        // High-performance bundled local stylesheet
        document::Link { rel: "stylesheet", href: CHAT_CSS }

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
                            class: "sidebar-header-badge",
                            div {
                                class: "status-dot"
                            }
                            span {
                                class: "sidebar-header-title",
                                "AI Workspace"
                            }
                        }
                        button {
                            class: "sidebar-icon-btn",
                            title: "Collapse sidebar",
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

                    // Primary Button: "New conversation" (Visually highlighted)
                    button {
                        class: "sidebar-btn-primary group",
                        onclick: move |_| {
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
                        span { "New conversation" }
                    }

                    // Navigation Button: "Conversation history"
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
                        span { "Conversation history" }
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
                                {
                                    let ag_idx = *selected_agent_idx.read();
                                    let agent_list = agents.read();
                                    let current_agent = &agent_list[ag_idx.min(agent_list.len() - 1)];
                                    rsx! {
                                        div {
                                            class: "w-2.5 h-2.5 rounded-full bg-gradient-to-tr {current_agent.avatar_gradient} flex-none shadow-sm"
                                        }
                                        span {
                                            class: "truncate font-medium text-xs text-zinc-200",
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

                    // Quick Search Bar
                    div {
                        class: "sidebar-search-box",
                        div {
                            class: "absolute inset-y-0 left-3 flex items-center pointer-events-none text-zinc-500",
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
                            placeholder: "Search chats...",
                            value: "{search_query}",
                            oninput: move |evt| {
                                search_query.set(evt.value());
                            },
                        }
                    }

                    // Section Title: Workspaces
                    div {
                        class: "sidebar-section-title",
                        span { "Workspaces" }
                        span { class: "workspace-badge", "{workspaces.read().len()}" }
                    }

                    // Workspaces List (Accordion)
                    div {
                        class: "flex flex-col gap-1.5",
                        for (ws_idx, ws) in workspaces.read().iter().enumerate() {
                            {
                                let is_expanded = ws.is_expanded;
                                let query = search_query.read().to_lowercase();
                                let filtered_chats: Vec<ChatSummary> = ws.chats.iter()
                                    .filter(|c| query.is_empty() || c.title.to_lowercase().contains(&query))
                                    .cloned()
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

                // ------------------------------------------------------------
                // 3. BOTTOM DOCK: Settings, Theme, Docs, GitHub
                // ------------------------------------------------------------
                div {
                    class: "sidebar-dock",

                    // Settings Button
                    button {
                        class: "sidebar-dock-btn",
                        title: "Settings",
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
                        title: if *is_dark_theme.read() { "Switch to Light theme" } else { "Switch to Dark theme" },
                        onclick: move |_| {
                            let curr = *is_dark_theme.read();
                            is_dark_theme.set(!curr);
                        },
                        if *is_dark_theme.read() {
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
                            // Sun icon
                            svg {
                                class: "w-4 h-4 text-amber-400",
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
                        title: "Documentation",
                        href: "https://dioxuslabs.com",
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
                        title: "GitHub Repository",
                        href: "https://github.com/DioxusLabs/dioxus",
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
