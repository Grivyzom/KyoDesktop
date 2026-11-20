//! Vista nativa "Proyectos & CLI": implementación gráfica completa de `kyo-cli` en Kyo Desktop.
//!
//! Equivalente nativo a `kyo-cli project link/status/sync/renew/unlink/auto-log` y `kyo-cli ui`:
//! - Lista los proyectos vinculados en esta máquina (interoperable con `kyo-cli/projects.json`).
//! - Permite vincular nuevas carpetas usando el selector nativo del SO (`rfd`) o ruta directa,
//!   creando un Proyecto nuevo (Espacio + Lienzo + Bóveda) o adoptando uno existente.
//! - Muestra estado de expiración del token `kyo_link_...` (con aviso desde 5 días antes),
//!   auditoría de archivos locales (`kyo-project.json`, `.kyo/credentials`, `CLAUDE.md`, `AGENTS.md`,
//!   `GEMINI.md`, `.mcp.json`, `.claude/settings.local.json`) y controles de 1 clic.
//! - Explorador de Tools MCP activas agrupadas por dominio con buscador.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dioxus::prelude::*;

use crate::dashboard::{
    use_kyo_cloud, CloudResource, CloudResourceKind, Icon, ICON_ALERT, ICON_CHAT, ICON_CHECK,
    ICON_CLOCK, ICON_EXTERNAL, ICON_FOLDER, ICON_KEY, ICON_LOCK, ICON_REFRESH, ICON_SHIELD,
    ICON_VAULT, ICON_X, KYO_WEB_URL,
};
use crate::kyo_cli::{
    find_project_root, forget_project_root, link_project, list_registered_roots,
    list_remote_projects, list_remote_spaces, load_project_row, project_name, project_ref,
    read_origin_remote, read_project_file, register_project_root, renew_project, set_auto_log,
    strip_credentials, sync_project, unlink_project, fetch_mcp_tools, LinkOptions, McpBridgeMode,
    McpToolInfo, ProjectRow, ProjectState, ProjectSummary, SpaceSummary, RENEW_WARNING_DAYS,
};
use crate::models::ActiveView;
use crate::settings::{load_auth_token, load_server_url, open_path_in_explorer};

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");
const ICON_PLUS: &str = "M12 5v14M5 12h14";
const ICON_TERMINAL: &str = "M4 17l6-6-6-6M12 19h8";
const ICON_SEARCH: &str = "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.35-4.35";
const ICON_TRASH: &str = "M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectsTab {
    LinkedProjects,
    McpTools,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkMode {
    NewProject,
    ExistingProject,
}

fn state_badge(state: ProjectState) -> (&'static str, &'static str) {
    match state {
        ProjectState::Ok => ("Vinculado", "dk-status-pill dk-status-ok"),
        ProjectState::NoSession => ("Sin sesión", "dk-status-pill dk-status-warn"),
        ProjectState::Unlinked => ("Sin vínculo", "dk-status-pill dk-status-off"),
        ProjectState::Absent => ("Carpeta ausente", "dk-status-pill dk-status-bad"),
        ProjectState::Error => ("Error", "dk-status-pill dk-status-warn"),
    }
}

fn expiry_text(row: &ProjectRow) -> (String, &'static str) {
    match row.days {
        None => (
            if row.has_token {
                "Token presente".to_string()
            } else {
                "Sin .kyo/credentials".to_string()
            },
            "dk-chip dk-chip-mono",
        ),
        Some(d) if d < 0 => (
            "TOKEN VENCIDO — Renovar".to_string(),
            "dk-chip dk-chip-bad",
        ),
        Some(d) if d <= RENEW_WARNING_DAYS => (
            format!("Vence en {d} día{}", if d == 1 { "" } else { "s" }),
            "dk-chip dk-chip-warn",
        ),
        Some(d) => (
            format!("Vigente ({d} días)"),
            "dk-chip dk-chip-ok",
        ),
    }
}

#[component]
pub fn ProjectsView() -> Element {
    let mut active_view = use_context::<Signal<ActiveView>>();
    let cloud = use_kyo_cloud();
    let mut active_context = cloud.active_context;

    let mut active_tab = use_signal(|| ProjectsTab::LinkedProjects);
    let projects = use_signal(Vec::<ProjectRow>::new);
    let mut selected_root = use_signal(|| Option::<String>::None);
    let is_loading = use_signal(|| false);
    let mut busy_root = use_signal(|| Option::<String>::None);
    let mut flash_msg = use_signal(|| Option::<(bool, String)>::None);
    let mut confirm_unlink_root = use_signal(|| Option::<String>::None);
    let mut mcp_mode = use_signal(|| McpBridgeMode::NativeDesktop);

    // Estado del explorador de Tools MCP
    let mcp_tools = use_signal(Vec::<McpToolInfo>::new);
    let mcp_loading = use_signal(|| false);
    let mcp_error = use_signal(|| Option::<String>::None);
    let mut mcp_filter = use_signal(String::new);
    let mut selected_tool = use_signal(|| Option::<McpToolInfo>::None);

    // Estado del modal "Vincular carpeta"
    let mut is_link_modal_open = use_signal(|| false);
    let mut link_folder_input = use_signal(String::new);
    let mut link_mode = use_signal(|| LinkMode::NewProject);
    let mut link_name_input = use_signal(String::new);
    let mut link_create_tools = use_signal(|| true);
    let mut link_template_id = use_signal(String::new);
    let mut link_existing_id = use_signal(String::new);
    let mut link_force = use_signal(|| false);
    let mut link_agent_files = use_signal(|| true);
    let mut link_status_line = use_signal(|| true);
    let mut link_auto_log = use_signal(|| false);
    let mut link_submitting = use_signal(|| false);
    let mut link_error = use_signal(|| Option::<String>::None);
    let remote_projects = use_signal(Vec::<ProjectSummary>::new);
    let remote_spaces = use_signal(Vec::<SpaceSummary>::new);
    let loading_remote_lists = use_signal(|| false);

    let has_session = !load_auth_token().trim().is_empty();
    let server_url_now = load_server_url();

    let refresh_projects = move || {
        let mut is_loading = is_loading;
        let mut selected_root = selected_root;
        let mut projects = projects;
        is_loading.set(true);
        spawn(async move {
            let roots = list_registered_roots();
            let sess_ok = !load_auth_token().trim().is_empty();
            let mut rows = Vec::with_capacity(roots.len());
            for r in roots {
                rows.push(load_project_row(&r, sess_ok).await);
            }
            let cur_sel = selected_root.peek().clone();
            if cur_sel
                .as_ref()
                .map(|s| !rows.iter().any(|r| &r.root == s))
                .unwrap_or(true)
            {
                selected_root.set(rows.first().map(|r| r.root.clone()));
            }
            projects.set(rows);
            is_loading.set(false);
        });
    };

    let load_tools_now = move || {
        let mut mcp_loading = mcp_loading;
        let mut mcp_error = mcp_error;
        let mut selected_tool = selected_tool;
        let mut mcp_tools = mcp_tools;
        mcp_loading.set(true);
        mcp_error.set(None);
        spawn(async move {
            match fetch_mcp_tools().await {
                Ok(list) => {
                    if selected_tool.peek().is_none() {
                        selected_tool.set(list.first().cloned());
                    }
                    mcp_tools.set(list);
                }
                Err(e) => {
                    mcp_error.set(Some(e));
                }
            }
            mcp_loading.set(false);
        });
    };

    // Carga inicial al abrir la vista
    use_effect(move || {
        refresh_projects();
    });

    let open_link_modal = move || {
        let mut link_error = link_error;
        let mut link_force = link_force;
        let mut link_existing_id = link_existing_id;
        let mut link_template_id = link_template_id;
        let mut is_link_modal_open = is_link_modal_open;
        let mut loading_remote_lists = loading_remote_lists;
        let mut remote_projects = remote_projects;
        let mut remote_spaces = remote_spaces;
        link_error.set(None);
        link_force.set(false);
        link_existing_id.set(String::new());
        link_template_id.set(String::new());
        is_link_modal_open.set(true);
        if !load_auth_token().trim().is_empty() {
            loading_remote_lists.set(true);
            spawn(async move {
                let (projs, spaces) = tokio::join!(list_remote_projects(), list_remote_spaces());
                if let Ok(p) = projs {
                    if let Some(first) = p.first() {
                        if link_existing_id.peek().is_empty() {
                            link_existing_id.set(first.id.clone());
                        }
                    }
                    remote_projects.set(p);
                }
                if let Ok(s) = spaces {
                    remote_spaces.set(s);
                }
                loading_remote_lists.set(false);
            });
        }
    };

    let pick_folder_for_link = move || {
        let mut link_folder_input = link_folder_input;
        let mut link_name_input = link_name_input;
        let mut link_error = link_error;
        spawn(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Selecciona la carpeta del proyecto a vincular con Kyō")
                .pick_folder()
                .await
            {
                let path = handle.path().to_path_buf();
                let root = find_project_root(&path);
                let r = project_ref(&root);
                let suggested = project_name(&root, &r);
                link_folder_input.set(root.to_string_lossy().to_string());
                if link_name_input.peek().trim().is_empty() {
                    link_name_input.set(suggested);
                }
                link_error.set(None);
            }
        });
    };

    let pick_existing_linked_folder = move || {
        let mut selected_root = selected_root;
        let mut flash_msg = flash_msg;
        let mut link_folder_input = link_folder_input;
        let mut link_name_input = link_name_input;
        spawn(async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Abrir carpeta ya vinculada (con kyo-project.json)")
                .pick_folder()
                .await
            {
                let root = find_project_root(handle.path());
                if read_project_file(&root).is_some() {
                    register_project_root(&root);
                    let r_str = root.to_string_lossy().to_string();
                    selected_root.set(Some(r_str));
                    flash_msg.set(Some((true, "Carpeta vinculada agregada al panel.".to_string())));
                    refresh_projects();
                } else {
                    link_folder_input.set(root.to_string_lossy().to_string());
                    let r = project_ref(&root);
                    link_name_input.set(project_name(&root, &r));
                    open_link_modal();
                }
            }
        });
    };

    let rows_snapshot = projects.read().clone();
    let selected_row = selected_root
        .read()
        .as_ref()
        .and_then(|s| rows_snapshot.iter().find(|r| &r.root == s).cloned())
        .or_else(|| rows_snapshot.first().cloned());

    let tools_snapshot = mcp_tools.read().clone();
    let filter_q = mcp_filter.read().trim().to_lowercase();
    let mut grouped_tools: BTreeMap<String, Vec<McpToolInfo>> = BTreeMap::new();
    for t in tools_snapshot.iter() {
        if !filter_q.is_empty()
            && !t.name.to_lowercase().contains(&filter_q)
            && !t.description.to_lowercase().contains(&filter_q)
            && !t.domain.to_lowercase().contains(&filter_q)
        {
            continue;
        }
        grouped_tools
            .entry(t.domain.clone())
            .or_default()
            .push(t.clone());
    }

    rsx! {
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        div {
            class: "dk-root",

            // ── Topbar ──────────────────────────────────────────────────────
            header {
                class: "dk-topbar",
                div {
                    class: "flex items-center gap-3",
                    div { class: "dk-topbar-icon", Icon { d: ICON_TERMINAL } }
                    h1 { class: "dk-topbar-title", "Proyectos & CLI" }
                    span { class: "dk-chip dk-chip-mono", "{rows_snapshot.len()} en esta máquina" }
                }
                div {
                    class: "flex items-center gap-2",
                    div {
                        class: "flex items-center gap-1 p-1 rounded-xl border border-zinc-800 bg-zinc-900/60",
                        button {
                            class: if *active_tab.read() == ProjectsTab::LinkedProjects {
                                "px-3 py-1 rounded-lg text-xs font-semibold bg-indigo-600/20 text-indigo-300 border border-indigo-500/30"
                            } else {
                                "px-3 py-1 rounded-lg text-xs font-medium text-zinc-400 hover:text-zinc-200 cursor-pointer"
                            },
                            onclick: move |_| active_tab.set(ProjectsTab::LinkedProjects),
                            "Proyectos ({rows_snapshot.len()})"
                        }
                        button {
                            class: if *active_tab.read() == ProjectsTab::McpTools {
                                "px-3 py-1 rounded-lg text-xs font-semibold bg-indigo-600/20 text-indigo-300 border border-indigo-500/30"
                            } else {
                                "px-3 py-1 rounded-lg text-xs font-medium text-zinc-400 hover:text-zinc-200 cursor-pointer"
                            },
                            onclick: move |_| {
                                active_tab.set(ProjectsTab::McpTools);
                                if mcp_tools.peek().is_empty() && !*mcp_loading.peek() {
                                    load_tools_now();
                                }
                            },
                            "Tools MCP ({tools_snapshot.len()})"
                        }
                    }

                    button {
                        class: "dk-btn dk-btn-secondary",
                        title: "Importar carpeta que ya tenga kyo-project.json",
                        onclick: move |_| pick_existing_linked_folder(),
                        Icon { d: ICON_FOLDER }
                        "Abrir carpeta"
                    }

                    button {
                        class: "dk-btn dk-btn-secondary",
                        disabled: *is_loading.read(),
                        onclick: move |_| {
                            if *active_tab.read() == ProjectsTab::McpTools {
                                load_tools_now();
                            } else {
                                refresh_projects();
                            }
                        },
                        Icon { d: ICON_REFRESH, class: if *is_loading.read() || *mcp_loading.read() { "dk-ic animate-spin" } else { "dk-ic" } }
                        "Refrescar"
                    }

                    button {
                        class: "dk-btn dk-btn-primary",
                        onclick: move |_| open_link_modal(),
                        Icon { d: ICON_PLUS }
                        "Vincular proyecto"
                    }
                }
            }

            main {
                class: "dk-scroll",
                div {
                    class: "dashboard-canvas",

                    if !has_session {
                        div {
                            class: "dk-card flex items-center justify-between gap-4 border-amber-500/30 bg-amber-950/15",
                            div {
                                class: "flex items-center gap-3",
                                div { class: "dk-card-head-icon dk-tone-warn", Icon { d: ICON_ALERT } }
                                div {
                                    h3 { class: "font-semibold text-sm", "Sin sesión de cuenta activa" }
                                    p { class: "dk-muted", "Inicia sesión con Kyō en el Panel de control para vincular proyectos, renovar tokens o consultar las tools MCP." }
                                }
                            }
                            button {
                                class: "dk-btn dk-btn-primary",
                                onclick: move |_| active_view.set(ActiveView::Dashboard),
                                "Ir al Panel de control"
                            }
                        }
                    }

                    if let Some((ok, msg)) = flash_msg.read().as_ref() {
                        div {
                            class: if *ok { "dk-flash animate-fade-in" } else { "dk-card border-rose-500/40 bg-rose-950/20 text-rose-200 flex items-center justify-between gap-3" },
                            div {
                                class: "flex items-center gap-2",
                                Icon { d: if *ok { ICON_CHECK } else { ICON_ALERT } }
                                span { "{msg}" }
                            }
                            button {
                                class: "dk-icon-btn",
                                onclick: move |_| flash_msg.set(None),
                                Icon { d: ICON_X }
                            }
                        }
                    }

                    // ── TAB 1: PROYECTOS VINCULADOS ────────────────────────
                    if *active_tab.read() == ProjectsTab::LinkedProjects {
                        if rows_snapshot.is_empty() {
                            section {
                                class: "dk-card text-center space-y-4 py-10",
                                div { class: "dk-hero-icon dk-tone-accent mx-auto", Icon { d: ICON_TERMINAL, class: "dk-ic-lg" } }
                                div {
                                    class: "space-y-1 max-w-md mx-auto",
                                    h3 { class: "text-base font-bold", "No hay proyectos vinculados en esta máquina" }
                                    p { class: "dk-muted", "Vincula una carpeta o repositorio Git para crear su Espacio de documentación, Bóveda de archivos y Lienzo de diagramas, y dejar listos CLAUDE.md, AGENTS.md, GEMINI.md y .mcp.json." }
                                }
                                div {
                                    class: "flex items-center justify-center gap-2.5",
                                    button {
                                        class: "dk-btn dk-btn-primary",
                                        onclick: move |_| open_link_modal(),
                                        Icon { d: ICON_PLUS }
                                        "Vincular una carpeta"
                                    }
                                    button {
                                        class: "dk-btn dk-btn-secondary",
                                        onclick: move |_| pick_existing_linked_folder(),
                                        Icon { d: ICON_FOLDER }
                                        "Importar proyecto ya vinculado"
                                    }
                                }
                            }
                        } else {
                            div {
                                class: "dk-bento",

                                // Columna Izquierda: Lista de Proyectos
                                section {
                                    class: "dk-card dk-col-main space-y-3",
                                    div {
                                        class: "dk-card-head",
                                        div { class: "dk-card-head-icon dk-tone-accent", Icon { d: ICON_FOLDER } }
                                        div {
                                            class: "dk-card-head-text",
                                            h3 { "Repositorios y Carpetas Vinculadas" }
                                            p { class: "dk-muted", "Registro compartido entre Kyo Desktop y `kyo-cli`." }
                                        }
                                    }

                                    div {
                                        class: "space-y-2",
                                        for row in rows_snapshot.iter() {
                                            {
                                                let is_sel = selected_row.as_ref().map(|s| s.root == row.root).unwrap_or(false);
                                                let r_path = row.root.clone();
                                                let (st_label, st_class) = state_badge(row.state);
                                                let (exp_lbl, exp_cls) = expiry_text(row);
                                                let is_busy = busy_root.read().as_deref() == Some(row.root.as_str());
                                                rsx! {
                                                    button {
                                                        key: "{row.root}",
                                                        class: if is_sel {
                                                            "w-full text-left p-3.5 rounded-xl border border-indigo-500/60 bg-indigo-950/25 transition-all cursor-pointer space-y-1.5"
                                                        } else {
                                                            "w-full text-left p-3.5 rounded-xl border border-zinc-800/80 bg-zinc-900/35 hover:border-zinc-700 hover:bg-zinc-900/70 transition-all cursor-pointer space-y-1.5"
                                                        },
                                                        onclick: move |_| {
                                                            selected_root.set(Some(r_path.clone()));
                                                            confirm_unlink_root.set(None);
                                                        },
                                                        div {
                                                            class: "flex items-center justify-between gap-2",
                                                            div {
                                                                class: "flex items-center gap-2 min-w-0",
                                                                span { class: "font-semibold text-sm truncate", "{row.name}" }
                                                                if row.auto_log {
                                                                    span { class: "dk-chip dk-chip-accent", "auto-log" }
                                                                }
                                                                if is_busy {
                                                                    span { class: "dk-chip dk-chip-mono", "procesando..." }
                                                                }
                                                            }
                                                            span { class: "{st_class}", "{st_label}" }
                                                        }
                                                        div {
                                                            class: "flex items-center justify-between gap-2 text-[11px]",
                                                            span { class: "dk-mono dk-muted truncate", "{row.root}" }
                                                            span { class: "{exp_cls}", "{exp_lbl}" }
                                                        }
                                                        if !row.branch.is_empty() {
                                                            div {
                                                                class: "text-[11px] dk-muted dk-mono",
                                                                "rama: {row.branch}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Columna Derecha: Detalle & Acciones del Proyecto seleccionado
                                if let Some(det) = selected_row.clone() {
                                    {
                                        let (_, st_cls) = state_badge(det.state);
                                        let (st_txt, _) = state_badge(det.state);
                                        let (exp_txt, exp_cls) = expiry_text(&det);
                                        let is_busy = busy_root.read().as_deref() == Some(det.root.as_str());
                                        let det_root_open = det.root.clone();
                                        let det_root_sync = det.root.clone();
                                        let det_root_renew = det.root.clone();
                                        let det_root_autolog = det.root.clone();
                                        let det_root_unlink = det.root.clone();
                                        let det_root_confirm = det.root.clone();
                                        let det_root_forget = det.root.clone();
                                        let det_auto = det.auto_log;
                                        let confirming_unlink = confirm_unlink_root.read().as_deref() == Some(det.root.as_str());

                                        rsx! {
                                            div {
                                                class: "dk-col-side",

                                                // Tarjeta principal del detalle
                                                section {
                                                    class: "dk-card space-y-4",
                                                    div {
                                                        class: "flex items-start justify-between gap-3",
                                                        div {
                                                            class: "min-w-0",
                                                            div {
                                                                class: "flex items-center gap-2 flex-wrap",
                                                                h3 { class: "text-base font-bold truncate", "{det.name}" }
                                                                span { class: "{st_cls}", "{st_txt}" }
                                                            }
                                                            p { class: "dk-muted dk-mono text-[11px] break-all mt-1", "{det.root}" }
                                                        }
                                                        button {
                                                            class: "dk-btn dk-btn-ghost dk-btn-xs shrink-0",
                                                            onclick: move |_| open_path_in_explorer(&PathBuf::from(&det_root_open)),
                                                            Icon { d: ICON_EXTERNAL, class: "dk-ic-sm" }
                                                            "Abrir"
                                                        }
                                                    }

                                                    dl {
                                                        class: "dk-diag",
                                                        div { class: "dk-diag-row",
                                                            dt { "Token vínculo" }
                                                            dd { span { class: "{exp_cls}", "{exp_txt}" } }
                                                        }
                                                        if !det.branch.is_empty() {
                                                            div { class: "dk-diag-row",
                                                                dt { "Rama Git" }
                                                                dd { class: "dk-mono", "{det.branch} · {det.hostname}" }
                                                            }
                                                        }
                                                        if let Some(ref pid) = det.project_id {
                                                            div { class: "dk-diag-row",
                                                                dt { "Project ID" }
                                                                dd { class: "dk-mono", "{pid}" }
                                                            }
                                                        }
                                                        if let Some(ref sid) = det.space_id {
                                                            div { class: "dk-diag-row",
                                                                dt { "Space ID" }
                                                                dd { class: "dk-mono", "{sid}" }
                                                            }
                                                        }
                                                    }

                                                    // Selector de modo MCP para sync/auto-log
                                                    div {
                                                        class: "flex items-center justify-between gap-2 pt-1 text-[11px]",
                                                        span { class: "dk-muted", "Puente `.mcp.json`:" }
                                                        select {
                                                            class: "dashboard-input dk-input-plain text-xs py-1 px-2 max-w-[220px]",
                                                            value: match *mcp_mode.read() {
                                                                McpBridgeMode::NativeDesktop => "native",
                                                                McpBridgeMode::KyoCli => "cli",
                                                                McpBridgeMode::HttpDirect => "http",
                                                            },
                                                            onchange: move |evt| {
                                                                match evt.value().as_str() {
                                                                    "cli" => mcp_mode.set(McpBridgeMode::KyoCli),
                                                                    "http" => mcp_mode.set(McpBridgeMode::HttpDirect),
                                                                    _ => mcp_mode.set(McpBridgeMode::NativeDesktop),
                                                                }
                                                            },
                                                            option { value: "native", "Nativo KyoDesktop (go-server)" }
                                                            option { value: "cli", "CLI npm (kyo-cli mcp)" }
                                                            option { value: "http", "HTTP directo (/api/v1/mcp)" }
                                                        }
                                                    }

                                                    // Acciones principales
                                                    if det.state == ProjectState::Absent {
                                                        button {
                                                            class: "dk-btn dk-btn-danger w-full",
                                                            onclick: move |_| {
                                                                forget_project_root(&PathBuf::from(&det_root_forget));
                                                                flash_msg.set(Some((true, "Carpeta olvidada del registro local.".to_string())));
                                                                refresh_projects();
                                                            },
                                                            Icon { d: ICON_TRASH }
                                                            "Olvidar carpeta ausente"
                                                        }
                                                    } else {
                                                        div {
                                                            class: "flex flex-wrap gap-2",
                                                            button {
                                                                class: "dk-btn dk-btn-primary dk-btn-xs",
                                                                disabled: is_busy || !has_session,
                                                                onclick: move |_| {
                                                                    let r = det_root_sync.clone();
                                                                    let mode = *mcp_mode.read();
                                                                    busy_root.set(Some(r.clone()));
                                                                    spawn(async move {
                                                                        match sync_project(&PathBuf::from(&r), true, true, mode).await {
                                                                            Ok((_, touched)) => {
                                                                                let msg = if touched.is_empty() {
                                                                                    "Proyecto sincronizado (archivos al día).".to_string()
                                                                                } else {
                                                                                    format!("Sincronizado · actualizado: {}", touched.join(", "))
                                                                                };
                                                                                flash_msg.set(Some((true, msg)));
                                                                                refresh_projects();
                                                                            }
                                                                            Err(e) => flash_msg.set(Some((false, e))),
                                                                        }
                                                                        busy_root.set(None);
                                                                    });
                                                                },
                                                                Icon { d: ICON_REFRESH, class: "dk-ic-sm" }
                                                                "Sincronizar"
                                                            }

                                                            button {
                                                                class: "dk-btn dk-btn-secondary dk-btn-xs",
                                                                disabled: is_busy || !has_session,
                                                                onclick: move |_| {
                                                                    let r = det_root_renew.clone();
                                                                    busy_root.set(Some(r.clone()));
                                                                    spawn(async move {
                                                                        match renew_project(&PathBuf::from(&r)).await {
                                                                            Ok((link, tracked)) => {
                                                                                let mut msg = format!("Token renovado (+30 días) para Espacio {}.", link.space_id);
                                                                                if tracked {
                                                                                    msg.push_str(" ATENCIÓN: .kyo/credentials está trackeado en git.");
                                                                                }
                                                                                flash_msg.set(Some((true, msg)));
                                                                                refresh_projects();
                                                                            }
                                                                            Err(e) => flash_msg.set(Some((false, e))),
                                                                        }
                                                                        busy_root.set(None);
                                                                    });
                                                                },
                                                                Icon { d: ICON_KEY, class: "dk-ic-sm" }
                                                                "Renovar token"
                                                            }

                                                            button {
                                                                class: if det_auto { "dk-btn dk-btn-secondary dk-btn-xs is-active" } else { "dk-btn dk-btn-ghost dk-btn-xs" },
                                                                disabled: is_busy || !has_session,
                                                                onclick: move |_| {
                                                                    let r = det_root_autolog.clone();
                                                                    let next = !det_auto;
                                                                    let mode = *mcp_mode.read();
                                                                    busy_root.set(Some(r.clone()));
                                                                    spawn(async move {
                                                                        match set_auto_log(&PathBuf::from(&r), next, mode).await {
                                                                            Ok(()) => {
                                                                                flash_msg.set(Some((true, format!("Auto-log {}.", if next { "activado" } else { "desactivado" }))));
                                                                                refresh_projects();
                                                                            }
                                                                            Err(e) => flash_msg.set(Some((false, e))),
                                                                        }
                                                                        busy_root.set(None);
                                                                    });
                                                                },
                                                                Icon { d: ICON_CLOCK, class: "dk-ic-sm" }
                                                                if det_auto { "Auto-log: ON" } else { "Auto-log: OFF" }
                                                            }

                                                            if !confirming_unlink {
                                                                button {
                                                                    class: "dk-btn dk-btn-danger dk-btn-xs ml-auto",
                                                                    disabled: is_busy || !has_session,
                                                                    onclick: move |_| confirm_unlink_root.set(Some(det_root_confirm.clone())),
                                                                    Icon { d: ICON_TRASH, class: "dk-ic-sm" }
                                                                    "Desvincular"
                                                                }
                                                            }
                                                        }
                                                    }

                                                    if confirming_unlink {
                                                        div {
                                                            class: "p-3 rounded-xl border border-rose-500/40 bg-rose-950/20 space-y-2 text-xs",
                                                            p { class: "text-rose-200", "¿Desvincular esta carpeta? Se borrarán `kyo-project.json`, `.kyo/credentials` y los bloques administrados. El Proyecto y su Espacio en Kyō se conservan." }
                                                            div {
                                                                class: "flex items-center justify-end gap-2",
                                                                button {
                                                                    class: "dk-btn dk-btn-ghost dk-btn-xs",
                                                                    onclick: move |_| confirm_unlink_root.set(None),
                                                                    "Cancelar"
                                                                }
                                                                button {
                                                                    class: "dk-btn dk-btn-danger dk-btn-xs",
                                                                    onclick: move |_| {
                                                                        let r = det_root_unlink.clone();
                                                                        confirm_unlink_root.set(None);
                                                                        busy_root.set(Some(r.clone()));
                                                                        spawn(async move {
                                                                            match unlink_project(&PathBuf::from(&r)).await {
                                                                                Ok(_) => {
                                                                                    flash_msg.set(Some((true, "Proyecto desvinculado. Su Espacio en Kyō sigue intacto.".to_string())));
                                                                                    refresh_projects();
                                                                                }
                                                                                Err(e) => flash_msg.set(Some((false, e))),
                                                                            }
                                                                            busy_root.set(None);
                                                                        });
                                                                    },
                                                                    "Sí, desvincular"
                                                                }
                                                            }
                                                        }
                                                    }

                                                    // Avisos y errores
                                                    for w in det.warnings.iter() {
                                                        div {
                                                            class: "p-2.5 rounded-xl border border-amber-500/30 bg-amber-950/20 text-amber-200 text-xs flex items-start gap-2",
                                                            Icon { d: ICON_ALERT, class: "dk-ic-sm shrink-0 mt-0.5" }
                                                            span { "{w}" }
                                                        }
                                                    }
                                                    if let Some(ref err) = det.error {
                                                        div {
                                                            class: "p-2.5 rounded-xl border border-rose-500/30 bg-rose-950/20 text-rose-200 text-xs",
                                                            "{err}"
                                                        }
                                                    }
                                                }

                                                // Herramientas del Workspace (Espacio / Bóveda / Lienzo)
                                                if let Some(ref ws) = det.workspace {
                                                    section {
                                                        class: "dk-card space-y-3",
                                                        div {
                                                            class: "dk-card-head",
                                                            div { class: "dk-card-head-icon dk-tone-ok", Icon { d: ICON_VAULT } }
                                                            div {
                                                                class: "dk-card-head-text",
                                                                h3 { "Herramientas del Workspace" }
                                                                p { class: "dk-muted", "{ws.tools.len()} herramientas vinculadas" }
                                                            }
                                                        }
                                                        div {
                                                            class: "space-y-2",
                                                            for tool in ws.tools.iter() {
                                                                {
                                                                    let t_kind = tool.tool_type.clone();
                                                                    let t_id = tool.tool_id.clone();
                                                                    let t_title = tool.title.clone();
                                                                    let web_link = if !tool.url.is_empty() {
                                                                        if tool.url.starts_with("http") {
                                                                            tool.url.clone()
                                                                        } else {
                                                                            format!("{}{}", server_url_now.trim_end_matches('/'), tool.url)
                                                                        }
                                                                    } else {
                                                                        KYO_WEB_URL.to_string()
                                                                    };
                                                                    rsx! {
                                                                        div {
                                                                            key: "{tool.tool_id}",
                                                                            class: "dk-explorer-row",
                                                                            div {
                                                                                class: "min-w-0",
                                                                                div {
                                                                                    class: "flex items-center gap-2",
                                                                                    span { class: "dk-chip dk-chip-accent uppercase", "{t_kind}" }
                                                                                    span { class: "font-semibold text-xs truncate", "{t_title}" }
                                                                                }
                                                                                span { class: "dk-mono dk-muted text-[10px]", "{t_id}" }
                                                                            }
                                                                            div {
                                                                                class: "flex items-center gap-1.5 shrink-0",
                                                                                if t_kind == "space" || t_kind == "vault" {
                                                                                    button {
                                                                                        class: "dk-btn dk-btn-secondary dk-btn-xs",
                                                                                        onclick: move |_| {
                                                                                            let kind = if t_kind == "vault" { CloudResourceKind::Vault } else { CloudResourceKind::Space };
                                                                                            active_context.set(Some(CloudResource {
                                                                                                id: t_id.clone(),
                                                                                                name: t_title.clone(),
                                                                                                kind,
                                                                                            }));
                                                                                            active_view.set(if kind == CloudResourceKind::Vault { ActiveView::Vault } else { ActiveView::Chat });
                                                                                        },
                                                                                        Icon { d: if tool.tool_type == "vault" { ICON_VAULT } else { ICON_CHAT }, class: "dk-ic-xs" }
                                                                                        if tool.tool_type == "vault" { "Explorar" } else { "Chat" }
                                                                                    }
                                                                                }
                                                                                button {
                                                                                    class: "dk-btn dk-btn-ghost dk-btn-xs",
                                                                                    onclick: move |_| crate::dashboard::open_in_browser(&web_link),
                                                                                    Icon { d: ICON_EXTERNAL, class: "dk-ic-xs" }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Auditoría de archivos locales y agentes
                                                section {
                                                    class: "dk-card space-y-2.5",
                                                    div {
                                                        class: "dk-card-head",
                                                        div { class: "dk-card-head-icon dk-tone-info", Icon { d: ICON_SHIELD } }
                                                        div {
                                                            class: "dk-card-head-text",
                                                            h3 { "Archivos Locales y Agentes" }
                                                            p { class: "dk-muted", "Estado de seguridad y contexto en disco" }
                                                        }
                                                    }
                                                    div {
                                                        class: "flex flex-wrap gap-1.5 text-[11px]",
                                                        AuditPill { label: "kyo-project.json", ok: det.audit.kyo_project_json }
                                                        AuditPill { label: ".kyo/credentials", ok: det.audit.credentials_file }
                                                        AuditPill { label: "CLAUDE.md", ok: det.audit.claude_md }
                                                        AuditPill { label: "AGENTS.md", ok: det.audit.agents_md }
                                                        AuditPill { label: "GEMINI.md", ok: det.audit.gemini_md }
                                                        AuditPill { label: ".mcp.json", ok: det.audit.mcp_json }
                                                        AuditPill { label: "statusLine", ok: det.audit.statusline_installed }
                                                        AuditPill { label: "hook Stop", ok: det.audit.auto_log_hook_installed }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── TAB 2: EXPLORADOR DE TOOLS MCP ─────────────────────
                    if *active_tab.read() == ProjectsTab::McpTools {
                        div {
                            class: "dk-bento",
                            section {
                                class: "dk-card dk-col-main space-y-3",
                                div {
                                    class: "flex items-center justify-between gap-3",
                                    div {
                                        class: "dk-input-wrap flex-1",
                                        span { class: "dk-input-icon", Icon { d: ICON_SEARCH } }
                                        input {
                                            class: "dashboard-input dk-input",
                                            placeholder: "Filtrar tools MCP por nombre, dominio o descripción...",
                                            value: "{mcp_filter}",
                                            oninput: move |evt| mcp_filter.set(evt.value()),
                                        }
                                    }
                                    span { class: "dk-chip dk-chip-mono", "{tools_snapshot.len()} tools" }
                                }

                                if *mcp_loading.read() {
                                    p { class: "dk-muted py-4", "Consultando /api/v1/mcp..." }
                                } else if let Some(ref err) = *mcp_error.read() {
                                    div {
                                        class: "p-3 rounded-xl border border-rose-500/30 bg-rose-950/20 text-rose-200 text-xs",
                                        "{err}"
                                    }
                                } else if grouped_tools.is_empty() {
                                    p { class: "dk-muted py-4", "No se encontraron tools MCP coincidentes." }
                                } else {
                                    div {
                                        class: "space-y-4 max-h-[520px] overflow-y-auto pr-1",
                                        for (domain, list) in grouped_tools.iter() {
                                            div {
                                                key: "{domain}",
                                                class: "space-y-1.5",
                                                div {
                                                    class: "text-[11px] font-bold uppercase tracking-wider text-zinc-400 px-1",
                                                    "▸ {domain} ({list.len()})"
                                                }
                                                div {
                                                    class: "grid grid-cols-1 sm:grid-cols-2 gap-1.5",
                                                    for tool in list.iter() {
                                                        {
                                                            let t_pick = tool.clone();
                                                            let is_sel = selected_tool.read().as_ref().map(|s| s.name == tool.name).unwrap_or(false);
                                                            rsx! {
                                                                button {
                                                                    key: "{tool.name}",
                                                                    class: if is_sel {
                                                                        "text-left px-2.5 py-2 rounded-lg border border-indigo-500/60 bg-indigo-950/30 text-xs dk-mono truncate cursor-pointer"
                                                                    } else {
                                                                        "text-left px-2.5 py-2 rounded-lg border border-zinc-800/80 bg-zinc-900/40 hover:border-zinc-700 text-xs dk-mono truncate cursor-pointer"
                                                                    },
                                                                    onclick: move |_| selected_tool.set(Some(t_pick.clone())),
                                                                    if tool.dangerous {
                                                                        span { class: "text-amber-400 mr-1", "▲" }
                                                                    }
                                                                    "{tool.name}"
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

                            div {
                                class: "dk-col-side",
                                section {
                                    class: "dk-card space-y-3",
                                    div {
                                        class: "dk-card-head",
                                        div { class: "dk-card-head-icon dk-tone-accent", Icon { d: ICON_LOCK } }
                                        div {
                                            class: "dk-card-head-text",
                                            h3 { "Detalle de Tool MCP" }
                                            p { class: "dk-muted", "Esquema y propósito en el servidor Kyō" }
                                        }
                                    }
                                    if let Some(ref tool) = *selected_tool.read() {
                                        div {
                                            class: "space-y-2.5",
                                            div {
                                                class: "flex items-center gap-2 flex-wrap",
                                                code { class: "text-sm font-bold text-indigo-300 dk-mono", "{tool.name}" }
                                                span { class: "dk-chip dk-chip-accent", "{tool.domain}" }
                                                if tool.dangerous {
                                                    span { class: "dk-chip dk-chip-warn", "Destructiva" }
                                                }
                                            }
                                            p { class: "text-xs leading-relaxed text-zinc-300",
                                                if tool.description.is_empty() { "Sin descripción." } else { "{tool.description}" }
                                            }
                                        }
                                    } else {
                                        p { class: "dk-muted text-xs", "Selecciona una tool para ver su descripción." }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── MODAL: VINCULAR CARPETA A KYŌ ──────────────────────────────────
        if *is_link_modal_open.read() {
            {
                let folder_str = link_folder_input.read().clone();
                let folder_trimmed = folder_str.trim();
                let preview_root = if !folder_trimmed.is_empty() {
                    Some(find_project_root(&PathBuf::from(folder_trimmed)))
                } else {
                    None
                };
                let preview_remote = preview_root
                    .as_ref()
                    .and_then(|r| read_origin_remote(r))
                    .map(|u| strip_credentials(&u));
                let preview_already_linked = preview_root
                    .as_ref()
                    .and_then(|r| read_project_file(r))
                    .is_some();

                rsx! {
                    div {
                        class: "fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 animate-fade-in",
                        onclick: move |_| is_link_modal_open.set(false),
                        div {
                            class: "w-full max-w-xl rounded-2xl bg-zinc-900 border border-zinc-700 shadow-2xl overflow-hidden text-zinc-100",
                            onclick: move |evt| evt.stop_propagation(),

                            header {
                                class: "px-6 py-4 border-b border-zinc-800 flex items-center justify-between",
                                div {
                                    h2 { class: "text-sm font-bold", "Vincular carpeta a Proyecto de Kyō" }
                                    p { class: "text-[11px] text-zinc-400", "Equivalente nativo a `kyo-cli project link`" }
                                }
                                button {
                                    class: "p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 cursor-pointer",
                                    onclick: move |_| is_link_modal_open.set(false),
                                    Icon { d: ICON_X }
                                }
                            }

                            div {
                                class: "p-6 space-y-4 max-h-[75vh] overflow-y-auto text-xs",

                                // 1. Selección de carpeta
                                div {
                                    class: "space-y-1.5",
                                    label { class: "dk-label", "Carpeta del proyecto / repositorio" }
                                    div {
                                        class: "flex items-center gap-2",
                                        input {
                                            class: "dashboard-input dk-input-plain flex-1",
                                            placeholder: "/ruta/a/tu/proyecto o C:\\proyectos\\mi-app",
                                            value: "{link_folder_input}",
                                            oninput: move |evt| {
                                                link_folder_input.set(evt.value());
                                                link_error.set(None);
                                            },
                                        }
                                        button {
                                            class: "dk-btn dk-btn-secondary shrink-0",
                                            onclick: move |_| pick_folder_for_link(),
                                            Icon { d: ICON_FOLDER }
                                            "Examinar..."
                                        }
                                    }
                                    if let Some(ref rem) = preview_remote {
                                        p { class: "text-[11px] text-emerald-400 dk-mono truncate", "Git origin: {rem}" }
                                    }
                                    if preview_already_linked {
                                        p { class: "text-[11px] text-amber-400", "⚠ Esta carpeta ya tiene `kyo-project.json`. Marca 'Revincular (--force)' si deseas reemplazarlo." }
                                    }
                                }

                                // 2. Modo: Nuevo vs Existente
                                div {
                                    class: "grid grid-cols-2 gap-2",
                                    button {
                                        class: if *link_mode.read() == LinkMode::NewProject {
                                            "p-2.5 rounded-xl border border-indigo-500/60 bg-indigo-950/30 font-semibold text-indigo-200 cursor-pointer"
                                        } else {
                                            "p-2.5 rounded-xl border border-zinc-800 bg-zinc-950/40 text-zinc-400 hover:text-zinc-200 cursor-pointer"
                                        },
                                        onclick: move |_| link_mode.set(LinkMode::NewProject),
                                        "Crear proyecto nuevo"
                                    }
                                    button {
                                        class: if *link_mode.read() == LinkMode::ExistingProject {
                                            "p-2.5 rounded-xl border border-indigo-500/60 bg-indigo-950/30 font-semibold text-indigo-200 cursor-pointer"
                                        } else {
                                            "p-2.5 rounded-xl border border-zinc-800 bg-zinc-950/40 text-zinc-400 hover:text-zinc-200 cursor-pointer"
                                        },
                                        onclick: move |_| link_mode.set(LinkMode::ExistingProject),
                                        "Vincular a uno existente"
                                    }
                                }

                                if *link_mode.read() == LinkMode::NewProject {
                                    div {
                                        class: "space-y-3 p-3.5 rounded-xl border border-zinc-800 bg-zinc-950/50",
                                        div {
                                            class: "space-y-1",
                                            label { class: "dk-label", "Nombre del Proyecto en Kyō" }
                                            input {
                                                class: "dashboard-input dk-input-plain w-full",
                                                placeholder: "Mi App (por defecto el del repo)",
                                                value: "{link_name_input}",
                                                oninput: move |evt| link_name_input.set(evt.value()),
                                            }
                                        }
                                        label {
                                            class: "flex items-center gap-2 cursor-pointer",
                                            input {
                                                r#type: "checkbox",
                                                checked: *link_create_tools.read(),
                                                onchange: move |evt| link_create_tools.set(evt.checked()),
                                            }
                                            span { "Crear también Espacio (docs), Lienzo (Mermaid) y Bóveda (archivos)" }
                                        }
                                        if *link_create_tools.read() && !remote_spaces.read().is_empty() {
                                            div {
                                                class: "space-y-1",
                                                label { class: "dk-label", "Plantilla de Espacio (opcional)" }
                                                select {
                                                    class: "dashboard-input dk-input-plain w-full",
                                                    value: "{link_template_id}",
                                                    onchange: move |evt| link_template_id.set(evt.value()),
                                                    option { value: "", "Espacio vacío (recomendado)" }
                                                    for sp in remote_spaces.read().iter() {
                                                        option {
                                                            value: "{sp.id}",
                                                            {
                                                                let lbl = if !sp.name.is_empty() {
                                                                    sp.name.clone()
                                                                } else {
                                                                    sp.title.clone().unwrap_or_else(|| sp.id.clone())
                                                                };
                                                                lbl
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    div {
                                        class: "space-y-2 p-3.5 rounded-xl border border-zinc-800 bg-zinc-950/50",
                                        label { class: "dk-label", "Elige un Proyecto existente de Kyō" }
                                        if *loading_remote_lists.read() {
                                            p { class: "dk-muted", "Cargando proyectos..." }
                                        } else if remote_projects.read().is_empty() {
                                            p { class: "dk-muted", "No tienes proyectos activos todavía. Crea uno nuevo." }
                                        } else {
                                            select {
                                                class: "dashboard-input dk-input-plain w-full",
                                                value: "{link_existing_id}",
                                                onchange: move |evt| link_existing_id.set(evt.value()),
                                                for pr in remote_projects.read().iter() {
                                                    option { value: "{pr.id}", "{pr.title}" }
                                                }
                                            }
                                        }
                                    }
                                }

                                // 3. Opciones de agentes y hooks
                                div {
                                    class: "space-y-2 p-3.5 rounded-xl border border-zinc-800 bg-zinc-950/40",
                                    label {
                                        class: "flex items-center gap-2 cursor-pointer",
                                        input {
                                            r#type: "checkbox",
                                            checked: *link_agent_files.read(),
                                            onchange: move |evt| link_agent_files.set(evt.checked()),
                                        }
                                        span { "Escribir CLAUDE.md, AGENTS.md, GEMINI.md y .mcp.json" }
                                    }
                                    label {
                                        class: "flex items-center gap-2 cursor-pointer",
                                        input {
                                            r#type: "checkbox",
                                            checked: *link_status_line.read(),
                                            onchange: move |evt| link_status_line.set(evt.checked()),
                                        }
                                        span { "Mostrar etiqueta \"Kyō activo\" en Claude Code (statusLine)" }
                                    }
                                    label {
                                        class: "flex items-center gap-2 cursor-pointer",
                                        input {
                                            r#type: "checkbox",
                                            checked: *link_auto_log.read(),
                                            onchange: move |evt| link_auto_log.set(evt.checked()),
                                        }
                                        span { "Activar auto-log obligatorio en el Espacio (hook Stop)" }
                                    }
                                    if preview_already_linked {
                                        label {
                                            class: "flex items-center gap-2 cursor-pointer text-amber-300",
                                            input {
                                                r#type: "checkbox",
                                                checked: *link_force.read(),
                                                onchange: move |evt| link_force.set(evt.checked()),
                                            }
                                            span { "Revincular carpeta (--force)" }
                                        }
                                    }
                                }

                                if let Some(ref err) = *link_error.read() {
                                    div {
                                        class: "p-3 rounded-xl border border-rose-500/40 bg-rose-950/20 text-rose-200 text-xs whitespace-pre-line",
                                        "{err}"
                                    }
                                }
                            }

                            footer {
                                class: "px-6 py-4 border-t border-zinc-800 flex items-center justify-end gap-2",
                                button {
                                    class: "dk-btn dk-btn-ghost",
                                    onclick: move |_| is_link_modal_open.set(false),
                                    "Cancelar"
                                }
                                button {
                                    class: "dk-btn dk-btn-primary",
                                    disabled: *link_submitting.read() || link_folder_input.read().trim().is_empty() || !has_session,
                                    onclick: move |_| {
                                        let dir = PathBuf::from(link_folder_input.peek().trim());
                                        let mode = *link_mode.peek();
                                        let opts = LinkOptions {
                                            name: if mode == LinkMode::NewProject {
                                                Some(link_name_input.peek().clone()).filter(|s| !s.trim().is_empty())
                                            } else {
                                                None
                                            },
                                            project_id: if mode == LinkMode::ExistingProject {
                                                Some(link_existing_id.peek().clone()).filter(|s| !s.trim().is_empty())
                                            } else {
                                                None
                                            },
                                            template_space_id: if mode == LinkMode::NewProject && *link_create_tools.peek() {
                                                Some(link_template_id.peek().clone()).filter(|s| !s.trim().is_empty())
                                            } else {
                                                None
                                            },
                                            no_tools: mode == LinkMode::NewProject && !*link_create_tools.peek(),
                                            force: *link_force.peek(),
                                            agent_files: *link_agent_files.peek(),
                                            status_line: *link_status_line.peek(),
                                            auto_log: *link_auto_log.peek(),
                                            mcp_mode: *mcp_mode.peek(),
                                        };
                                        link_submitting.set(true);
                                        link_error.set(None);
                                        spawn(async move {
                                            match link_project(&dir, opts).await {
                                                Ok(outcome) => {
                                                    let title = outcome
                                                        .link
                                                        .workspace
                                                        .as_ref()
                                                        .map(|w| w.title.clone())
                                                        .or(outcome.link.space_name.clone())
                                                        .unwrap_or_else(|| "Proyecto".to_string());
                                                    let root_str = find_project_root(&dir).to_string_lossy().to_string();
                                                    selected_root.set(Some(root_str));
                                                    is_link_modal_open.set(false);
                                                    let mut msg = format!("Carpeta vinculada a \"{title}\".");
                                                    if outcome.credentials_tracked {
                                                        msg.push_str(" ATENCIÓN: .kyo/credentials está versionado en git.");
                                                    }
                                                    flash_msg.set(Some((true, msg)));
                                                    refresh_projects();
                                                }
                                                Err(e) => {
                                                    link_error.set(Some(e));
                                                }
                                            }
                                            link_submitting.set(false);
                                        });
                                    },
                                    if *link_submitting.read() { "Vinculando..." } else { "Vincular proyecto" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AuditPill(label: &'static str, ok: bool) -> Element {
    rsx! {
        span {
            class: if ok {
                "px-2 py-0.5 rounded-md border border-emerald-500/30 bg-emerald-500/10 text-emerald-300 dk-mono"
            } else {
                "px-2 py-0.5 rounded-md border border-zinc-800 bg-zinc-900/50 text-zinc-500 dk-mono"
            },
            if ok { "✓ {label}" } else { "○ {label}" }
        }
    }
}
