use dioxus::prelude::*;
use ui::{ChatContainer, Sidebar};


#[cfg(feature = "desktop")]
use dioxus::desktop::{Config, LogicalPosition, LogicalSize, WindowBuilder};

const MAIN_CSS: Asset = asset!("/assets/main.css");

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
            .with_title("Dioxus Chat")
            .with_decorations(false)
            .with_resizable(true)
            .with_always_on_top(false)
            .with_inner_size(LogicalSize::new(1200.0, 800.0))
            .with_position(LogicalPosition::new(100.0, 100.0));

        let cfg = Config::new()
            .with_window(window)
            .with_menu(None)
            .with_on_window(|window, _dom| {
                // Dynamically center window on screen if monitor info is available
                if let Some(monitor) = window.current_monitor().or_else(|| window.primary_monitor()) {
                    let screen_size = monitor.size();
                    let window_size = window.outer_size();
                    let x = ((screen_size.width as i32 - window_size.width as i32) / 2).max(50);
                    let y = ((screen_size.height as i32 - window_size.height as i32) / 2).max(50);
                    window.set_outer_position(dioxus::desktop::tao::dpi::PhysicalPosition::new(x, y));
                }
                window.set_minimized(false);
                window.set_visible(true);
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
        let mut is_max = use_signal(|| false);

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
                            dioxus::desktop::window().close();
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
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        div {
            class: "flex flex-col h-screen w-screen overflow-hidden bg-zinc-950",
            TitleBar {}
            div {
                class: "flex flex-1 min-h-0 w-full overflow-hidden relative",
                Sidebar {}
                div {
                    class: "flex-1 min-w-0 h-full overflow-hidden",
                    ChatContainer {}
                }
            }
        }
    }
}

