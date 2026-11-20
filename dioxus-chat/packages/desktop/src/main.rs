#![cfg_attr(windows, windows_subsystem = "windows")]

use dioxus::prelude::*;
use ui::{
    detect_windows_theme_is_dark, load_font_family, load_theme_mode, load_theme_preference,
    load_downloads_dir, spawn_music_monitor, spawn_sidecar_service, terminate_sidecar, ActiveView,
    AppDownloadsDir, AppFont, AppTheme, AppThemeMode, ChatContainer, Dashboard, IsSettingsOpen,
    KyoCloud, MusicContext, ProjectsView, SettingsModal, SettingsTab, Sidebar, SidecarState,
    ThemePreference, VaultView, Workspace,
};


#[cfg(feature = "desktop")]
use dioxus::desktop::{Config, LogicalPosition, LogicalSize, WindowBuilder};

const MAIN_CSS: &str = include_str!("../assets/main.css");
const CHAT_CSS: &str = include_str!("../../ui/assets/styling/chat.css");

fn main() {
    #[cfg(feature = "desktop")]
    {
        // 1. Clear any corrupted window-state.json from Dioxus session cache
        // which might have saved minimized coordinates (-32000, -32000).
        if let Ok(dir) = std::env::var("DIOXUS_SESSION_CACHE_DIR") {
            let p = std::path::PathBuf::from(&dir).join("window-state.json");
            let _ = std::fs::remove_file(&p);
        }
        let temp_state = std::env::temp_dir().join("window-state.json");
        let _ = std::fs::remove_file(&temp_state);

        // 2. Configure borderless window with explicit position to prevent Dioxus's
        // resume_from_state from restoring off-screen (-32000, -32000) coordinates.
        let window = WindowBuilder::new()
            .with_title("Kyo Desktop")
            .with_decorations(false)
            .with_resizable(true)
            .with_always_on_top(false)
            .with_inner_size(LogicalSize::new(1200.0, 800.0))
            .with_position(LogicalPosition::new(100.0, 100.0))
            .with_maximized(true);

        let mut custom_head = String::with_capacity(MAIN_CSS.len() + CHAT_CSS.len() + 256);
        custom_head.push_str("<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n<style id=\"main-css\">\n");
        custom_head.push_str(MAIN_CSS);
        custom_head.push_str("\n</style>\n<style id=\"chat-css\">\n");
        custom_head.push_str(CHAT_CSS);
        custom_head.push_str("\n</style>\n");

        let initial_is_dark = load_theme_preference();
        let initial_bg = if initial_is_dark { (9, 9, 11, 255) } else { (255, 255, 255, 255) };

        let cfg = Config::new()
            .with_window(window)
            .with_menu(None)
            .with_background_color(initial_bg)
            .with_custom_head(custom_head)
            .with_on_window(|window, _dom| {
                // Abrir siempre en modo ventana completa (maximizada) y nunca minimizada
                window.set_minimized(false);
                window.set_visible(true);
                if !window.is_maximized() {
                    window.set_maximized(true);
                }
                window.set_focus();
            });

        LaunchBuilder::desktop()
            .with_cfg(cfg)
            .launch(App);
    }
    #[cfg(not(feature = "desktop"))]
    {
        dioxus::launch(App);
    }
}

/// Sleek custom window titlebar with native-like drag and window control buttons.
#[component]
fn TitleBar() -> Element {
    #[cfg(feature = "desktop")]
    {
        let mut is_max = use_signal(|| dioxus::desktop::window().is_maximized());

        rsx! {
            header {
                class: "titlebar-container",
                ondoubleclick: move |_| {
                    let d = dioxus::desktop::window();
                    let next = !d.is_maximized();
                    d.set_maximized(next);
                    is_max.set(next);
                },

                // Entire bar is a draggable region
                div {
                    class: "titlebar-drag-area",
                    onmousedown: move |_| {
                        dioxus::desktop::window().drag();
                    },
                }

                // Window control buttons (Minimize, Maximize/Restore, Close)
                div {
                    class: "titlebar-controls",
                    onmousedown: move |evt| {
                        evt.stop_propagation();
                    },

                    // Minimize
                    button {
                        class: "titlebar-btn",
                        title: "Minimizar",
                        onclick: move |_| {
                            dioxus::desktop::window().set_minimized(true);
                        },
                        svg {
                            class: "w-3 h-3",
                            view_box: "0 0 10 10",
                            path {
                                fill: "currentColor",
                                d: "M1 5h8v1H1z"
                            }
                        }
                    }

                    // Maximize / Restore
                    button {
                        class: "titlebar-btn",
                        title: if *is_max.read() { "Restaurar" } else { "Maximizar" },
                        onclick: move |_| {
                            let d = dioxus::desktop::window();
                            let next = !d.is_maximized();
                            d.set_maximized(next);
                            is_max.set(next);
                        },
                        if *is_max.read() {
                            svg {
                                class: "w-3 h-3",
                                view_box: "0 0 10 10",
                                path {
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1",
                                    d: "M3 1h6v6H3z M1 3h6v6H1z"
                                }
                            }
                        } else {
                            svg {
                                class: "w-3 h-3",
                                view_box: "0 0 10 10",
                                path {
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1",
                                    d: "M1 1h8v8H1z"
                                }
                            }
                        }
                    }

                    // Close
                    button {
                        class: "titlebar-btn titlebar-btn-close",
                        title: "Cerrar",
                        onclick: move |_| {
                            terminate_sidecar();
                            dioxus::desktop::window().close();
                            std::thread::spawn(|| {
                                std::process::exit(0);
                            });
                        },
                        svg {
                            class: "w-3 h-3",
                            view_box: "0 0 10 10",
                            path {
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1.2",
                                stroke_linecap: "round",
                                d: "M2 2l6 6M8 2l-6 6"
                            }
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(feature = "desktop"))]
    {
        rsx! {}
    }
}

#[component]
fn App() -> Element {
    let active_view = use_context_provider(|| Signal::new(ActiveView::Dashboard));
    let _workspaces = use_context_provider(|| Signal::new(Vec::<Workspace>::new()));
    // Estado compartido de la conexión con Kyō Web (Dashboard ↔ Sidebar); sobrevive al cambio de vista
    let _kyo_cloud = use_context_provider(KyoCloud::new);
    let is_settings_open = use_context_provider(|| IsSettingsOpen(Signal::new(false)));
    let _settings_target_tab = use_context_provider(|| Signal::new(Option::<SettingsTab>::None));
    let _downloads_dir = use_context_provider(|| AppDownloadsDir(Signal::new(load_downloads_dir())));
    let sidecar_state = use_context_provider(|| Signal::new(SidecarState::default()));
    let music_ctx = use_context_provider(MusicContext::new);
    let app_font = use_context_provider(|| AppFont(Signal::new(load_font_family())));
    // Theme state: follows Windows 11 system theme by default, or explicit user override
    let app_theme_mode = use_context_provider(|| AppThemeMode(Signal::new(load_theme_mode())));
    let mut app_theme = use_context_provider(|| AppTheme(Signal::new(load_theme_preference())));
    let is_dark = *app_theme.0.read();
    let theme_class = if is_dark { "dark" } else { "light" };

    let font = app_font.0.read().clone();
    let sanitized_font = font.replace(|c: char| c == '\'' || c == '"' || c == ';' || c == '<' || c == '>', "").trim().to_string();
    let target_font_css = if sanitized_font.is_empty() || sanitized_font == "Segoe UI" || sanitized_font == "Default" || sanitized_font == "Kyo" || sanitized_font == "Comic Sans MS" {
        "'Segoe UI', system-ui, -apple-system, BlinkMacSystemFont, 'Inter', Roboto, sans-serif".to_string()
    } else {
        format!("'{sanitized_font}', 'Segoe UI', system-ui, -apple-system, BlinkMacSystemFont, 'Inter', Roboto, sans-serif")
    };
    let dynamic_font_css = format!(
        ":root {{ --kyo-font-family: {target_font_css}; }} html, body, div, span, p, h1, h2, h3, h4, h5, h6, label, button, input, select, textarea, aside, header, main, section, nav, a {{ font-family: {target_font_css} !important; }} *, *::before, *::after {{ font-family: inherit; }} code, kbd, samp, pre, .font-mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace !important; }}"
    );

    use_effect(move || {
        let f = app_font.0.read().clone();
        let s = f.replace(|c: char| c == '\'' || c == '"' || c == ';' || c == '<' || c == '>', "").trim().to_string();
        let target = if s.is_empty() || s == "Segoe UI" || s == "Default" || s == "Kyo" || s == "Comic Sans MS" {
            "'Segoe UI', system-ui, -apple-system, BlinkMacSystemFont, 'Inter', Roboto, sans-serif".to_string()
        } else {
            format!("'{s}', 'Segoe UI', system-ui, -apple-system, BlinkMacSystemFont, 'Inter', Roboto, sans-serif")
        };
        #[cfg(feature = "desktop")]
        {
            let script = format!(
                r#"
                document.documentElement.style.setProperty('--kyo-font-family', "{target}");
                if (document.body) document.body.style.setProperty('--kyo-font-family', "{target}");
                let st = document.getElementById('kyo-runtime-font');
                if (!st) {{
                    st = document.createElement('style');
                    st.id = 'kyo-runtime-font';
                    document.head.appendChild(st);
                }}
                st.textContent = `
                    :root {{ --kyo-font-family: {target}; }}
                    html, body, div, span, p, h1, h2, h3, h4, h5, h6, label, button, input, select, textarea, aside, header, main, section, nav, a {{
                        font-family: {target} !important;
                    }}
                    *, *::before, *::after {{
                        font-family: inherit;
                    }}
                    code, kbd, samp, pre, .font-mono {{
                        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace !important;
                    }}
                `;
                "#
            );
            spawn(async move {
                let eval = document::eval(&script);
                let _ = eval.await;
            });
        }
    });

    use_effect(move || {
        let is_dark = *app_theme.0.read();
        #[cfg(feature = "desktop")]
        {
            let class_op = if is_dark { "remove" } else { "add" };
            let script = format!(
                "document.documentElement.classList.{class_op}('light'); document.body.classList.{class_op}('light');"
            );
            spawn(async move {
                let eval = document::eval(&script);
                let _ = eval.await;
            });
        }
    });

    // Sincronización en vivo con el tema de Windows 11 cuando el modo está en Sistema
    #[cfg(feature = "desktop")]
    use_hook(move || {
        spawn(async move {
            loop {
                ui::sleep_secs(2).await;
                if *app_theme_mode.0.read() == ThemePreference::System {
                    let win_is_dark = detect_windows_theme_is_dark();
                    if *app_theme.0.read() != win_is_dark {
                        app_theme.0.set(win_is_dark);
                    }
                }
            }
        });
    });

    // Iniciar el microservicio Go Server y el detector musical en segundo plano de manera no bloqueante
    use_hook(move || {
        spawn_sidecar_service(sidecar_state);
        spawn_music_monitor(music_ctx);
    });

    rsx! {
        document::Style { {MAIN_CSS} }
        document::Style { {CHAT_CSS} }
        document::Style { "{dynamic_font_css}" }
        div {
            class: "flex flex-col h-screen w-screen overflow-hidden {theme_class}",
            style: format!("--kyo-font-family: {target_font_css} !important; font-family: {target_font_css} !important; background-color: var(--kyo-surface-0); color: var(--kyo-fg-primary);"),
            TitleBar {}
            div {
                class: "flex flex-1 min-h-0 w-full overflow-hidden relative",
                Sidebar {}
                div {
                    class: "flex-1 min-w-0 h-full overflow-hidden",
                    match *active_view.read() {
                        ActiveView::Dashboard => rsx! { Dashboard {} },
                        ActiveView::Projects => rsx! { ProjectsView {} },
                        ActiveView::Chat => rsx! { ChatContainer {} },
                        ActiveView::Vault => rsx! { VaultView {} },
                    }
                }
            }
        }

        // Modal de Ajustes y Conexión Kyo
        if *is_settings_open.0.read() {
            div {
                class: "{theme_class}",
                SettingsModal {
                    is_open: is_settings_open.0,
                }
            }
        }
    }
}

