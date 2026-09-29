//! Production-Ready AI Agent Chat System in Rust using Dioxus.
//!
//! Features:
//! - Idiomatic Signals (`use_signal`), async spawning with non-blocking tasks (`spawn`).
//! - Complete data model: `Role`, `MessageStatus`, `ToolCall`, `Message`.
//! - Streaming emulation with token-by-token emission and auto-scroll snapping via `document::eval`.
//! - Cancellation support via task handles (`Stop Generating`).
//! - Resilient error handling with retry capability and graceful UI fallbacks.
//! - Responsive Tailwind CSS dark-mode layout with sticky auto-expanding input bar.

use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;

// ============================================================================
// KYO AI PROTOCOL DATA MODELS (POST /api/v1/ai/ask/stream)
// ============================================================================

#[derive(Serialize, Debug)]
pub struct AskRequest {
    pub question: String,
    pub use_context: bool,
    pub history: Vec<HistoryMessage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

/// event: delta -> { "text": "..." }
#[derive(Deserialize, Debug)]
pub struct DeltaPayload {
    pub text: String,
}

/// event: sources -> { "sources": [ { "title": "...", "score": 0.8 } ] }
#[derive(Deserialize, Debug)]
pub struct SourcesPayload {
    pub sources: Vec<SourceItem>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SourceItem {
    pub title: String,
    pub score: Option<f64>,
}

/// event: actions -> { "actions": [...] }
#[derive(Deserialize, Debug)]
pub struct ActionsPayload {
    pub actions: Vec<serde_json::Value>,
}

/// event: done -> { "answer": "...", "model": "...", "usage": {...} }
#[derive(Deserialize, Debug)]
pub struct DonePayload {
    pub answer: Option<String>,
    pub model: Option<String>,
}

/// event: error -> { "error": "..." }
#[derive(Deserialize, Debug)]
pub struct ErrorPayload {
    pub error: String,
}

// ============================================================================
// 1. DATA MODELS & STRONG TYPING
// ============================================================================

/// Represents the role of a message participant in the LLM conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System(String),
}

/// Lifecycle status of an individual message.
#[derive(Clone, Debug, PartialEq)]
pub enum MessageStatus {
    Sending,
    Sent,
    Streaming,
    Error(String),
}

/// Agent tool or function invocation structure.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
    pub output: Option<String>,
}

/// Represents a chat message within the conversation thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
    pub timestamp: u64,
    pub status: MessageStatus,
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    /// Creates a new message with a generated timestamp and ID.
    pub fn new(role: Role, content: impl Into<String>, status: MessageStatus) -> Self {
        Self {
            id: generate_unique_id(),
            role,
            content: content.into(),
            timestamp: current_timestamp_secs(),
            status,
            tool_calls: Vec::new(),
        }
    }

    /// Helper constructor for user messages.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content, MessageStatus::Sent)
    }

    /// Helper constructor for assistant streaming messages.
    pub fn assistant_streaming() -> Self {
        Self::new(Role::Assistant, "", MessageStatus::Streaming)
    }

    /// Helper constructor for system notifications/prompts.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(
            Role::System("System Guardrail".to_string()),
            content,
            MessageStatus::Sent,
        )
    }

    /// Formats the unix timestamp into a readable HH:MM:SS format without external datetime crates.
    pub fn formatted_time(&self) -> String {
        let secs = self.timestamp % 86400;
        let hours = (secs / 3600) % 24;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

/// Thread-safe generator for unique message IDs.
fn generate_unique_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = current_timestamp_secs();
    format!("msg-{ts}-{id}")
}

/// Returns the current Unix timestamp in seconds.
fn current_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Triggers smooth scrolling to the bottom of the message viewport via Dioxus JS evaluation.
pub fn trigger_auto_scroll() {
    let _ = document::eval(
        r#"
        const el = document.getElementById("chat-scroll-container");
        if (el) {
            el.scrollTop = el.scrollHeight;
        }
        "#,
    );
}

// ============================================================================
// 2. ROOT CONTAINER & CONTROLLER (ChatContainer)
// ============================================================================

const CHAT_CSS: &str = include_str!("../assets/styling/chat.css");

/// Main layout shell with split viewport (header, scrollable message stream, and sticky input dock).
#[component]
pub fn ChatContainer() -> Element {
    let active_view = try_use_context::<Signal<crate::models::ActiveView>>();

    // Reactive root states: No hardcoded chats or mock messages
    let mut messages = use_signal(Vec::<Message>::new);

    let mut is_generating = use_signal(|| false);
    let mut input_text = use_signal(String::new);
    let mut generation_task: Signal<Option<dioxus::dioxus_core::Task>> = use_signal(|| None);

    // Aborts active agent generation
    let mut stop_generation = move || {
        if let Some(task) = generation_task.write().take() {
            task.cancel();
        }
        is_generating.set(false);

        // Transition any active streaming messages to Sent so partial output is retained
        let mut msgs = messages.write();
        for msg in msgs.iter_mut() {
            if msg.status == MessageStatus::Streaming {
                msg.status = MessageStatus::Sent;
                if msg.content.is_empty() {
                    msg.content = "*(Generation stopped by user)*".to_string();
                } else {
                    msg.content.push_str(" *(stopped)*");
                }
            }
        }
        drop(msgs);
        trigger_auto_scroll();
    };

    // Submits the current input prompt and triggers the streaming agent loop
    let mut send_prompt = move |prompt: String| {
        let trimmed = prompt.trim().to_string();
        if trimmed.is_empty() || *is_generating.read() {
            return;
        }

        // Reset input field
        input_text.set(String::new());

        // Append user message
        messages.write().push(Message::user(&trimmed));
        trigger_auto_scroll();

        // Acotar el historial previo a los últimos 6 mensajes (tope de tokens de Kyo)
        let history: Vec<HistoryMessage> = {
            let msgs = messages.read();
            msgs.iter()
                .filter(|m| m.role == Role::User || matches!(m.role, Role::Assistant))
                .map(|m| HistoryMessage {
                    role: match m.role {
                        Role::User => "user".to_string(),
                        Role::Assistant => "assistant".to_string(),
                        _ => "user".to_string(),
                    },
                    content: m.content.clone(),
                })
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        };

        // Prepare placeholder for streaming assistant response
        let assistant_id = generate_unique_id();
        let placeholder = Message {
            id: assistant_id.clone(),
            role: Role::Assistant,
            content: String::new(),
            timestamp: current_timestamp_secs(),
            status: MessageStatus::Streaming,
            tool_calls: Vec::new(),
        };
        messages.write().push(placeholder);
        is_generating.set(true);
        trigger_auto_scroll();

        let prompt_clone = trimmed.clone();
        let target_id = assistant_id.clone();

        // Spawn non-blocking background streaming task
        let task = spawn(async move {
            let res = stream_kyo_ask(
                prompt_clone,
                history,
                target_id.clone(),
                messages,
                is_generating,
            )
            .await;

            if let Err(err) = res {
                eprintln!("[Kyo Stream Error] {err}");
                let mut msgs = messages.write();
                if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_id) {
                    msg.status = MessageStatus::Error(err);
                }
            }

            // Garantizar estado inactivo al finalizar
            is_generating.set(false);
            generation_task.set(None);
            trigger_auto_scroll();
        });

        generation_task.set(Some(task));
    };

    // Retry callback for failed messages
    let on_retry = move |failed_msg_id: String| {
        // Find failed message and its preceding user message
        let msgs = messages.read();
        let mut retry_prompt = None;

        for (idx, msg) in msgs.iter().enumerate() {
            if msg.id == failed_msg_id && idx > 0 {
                if let Some(prev) = msgs.get(idx - 1) {
                    if prev.role == Role::User {
                        retry_prompt = Some(prev.content.clone());
                    }
                }
                break;
            }
        }
        drop(msgs);

        // Remove the failed message
        messages.write().retain(|m| m.id != failed_msg_id);

        if let Some(prompt) = retry_prompt {
            send_prompt(prompt);
        }
    };

    rsx! {
        // High-performance bundled local stylesheet (0ms cold-start, 100% offline)
        document::Style {
            href: "chat-css".to_string(),
            {CHAT_CSS}
        }

        div {
            class: "flex flex-col h-full w-full bg-zinc-950 text-zinc-100 antialiased font-sans selection:bg-indigo-500 selection:text-white",

            // Top Navigation / Agent Control Bar
            header {
                class: "flex-none h-14 border-b border-zinc-800/80 bg-zinc-900/60 backdrop-blur-md px-6 flex items-center justify-between z-10",
                div {
                    class: "flex items-center gap-3",
                    div {
                        class: "relative flex items-center justify-center w-8 h-8 rounded-lg bg-gradient-to-tr from-indigo-600 to-violet-500 text-white shadow-md shadow-indigo-500/20",
                        svg {
                            class: "w-4 h-4 fill-none stroke-current stroke-2",
                            view_box: "0 0 24 24",
                            path { d: "M12 2v4m0 12v4M4.93 4.93l2.83 2.83m8.48 8.48l2.83 2.83M2 12h4m12 0h4M4.93 19.07l2.83-2.83m8.48-8.48l2.83-2.83" }
                        }
                    }
                    div {
                        div { class: "flex items-center gap-2",
                            span { class: "font-semibold text-sm tracking-tight text-zinc-100", "Antigravity Agent" }
                            span { class: "px-2 py-0.5 text-[10px] font-medium tracking-wide bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 rounded-full", "Dioxus 0.6+" }
                        }
                        span { class: "text-[11px] text-zinc-400 font-mono", "Rust 2024 Engine • Local Memory Active" }
                    }
                }

                div {
                    class: "flex items-center gap-2",
                    if let Some(mut av) = active_view {
                        button {
                            class: "chat-header-btn flex items-center gap-1.5",
                            title: "Ir al Panel de control",
                            onclick: move |_| {
                                av.set(crate::models::ActiveView::Dashboard);
                            },
                            svg {
                                class: "w-3.5 h-3.5 text-indigo-400",
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
                    if *is_generating.read() {
                        div {
                            class: "flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-indigo-950/60 border border-indigo-700/50 text-indigo-300 text-xs",
                            span { class: "w-2 h-2 rounded-full bg-indigo-400 animate-ping" }
                            span { class: "font-mono text-[11px]", "Streaming tokens..." }
                        }
                    }
                    button {
                        class: "chat-header-btn",
                        onclick: move |_| {
                            messages.write().clear();
                        },
                        "Clear Chat"
                    }
                }
            }

            // Middle Viewport: Scrollable Message List
            MessageList {
                messages,
                is_generating,
                on_retry,
            }

            // Bottom Dock: Sticky Input & Control Area
            ChatInput {
                input_text,
                is_generating,
                on_send: move |_| {
                    let text = input_text.read().clone();
                    send_prompt(text);
                },
                on_stop: move |_| {
                    stop_generation();
                },
            }
        }
    }
}

// ============================================================================
// 3. MESSAGE VIEWPORT & LIST (MessageList)
// ============================================================================

/// Scrollable message viewport maintaining bottom anchor and auto-scrolling synchronization.
#[component]
pub fn MessageList(
    messages: Signal<Vec<Message>>,
    is_generating: Signal<bool>,
    on_retry: EventHandler<String>,
) -> Element {
    let msgs = messages.read();

    rsx! {
        main {
            id: "chat-scroll-container",
            class: "flex-1 overflow-y-auto px-4 md:px-8 py-6 space-y-6 scroll-smooth",

            if msgs.is_empty() {
                div {
                    class: "flex flex-col items-center justify-center h-full min-h-[360px] text-center max-w-md mx-auto space-y-4",
                    div {
                        class: "p-4 rounded-2xl bg-zinc-900/80 border border-zinc-800 shadow-inner",
                        svg {
                            class: "w-8 h-8 text-zinc-500 mx-auto fill-none stroke-current stroke-1.5",
                            view_box: "0 0 24 24",
                            path { d: "M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" }
                        }
                    }
                    h3 { class: "text-base font-medium text-zinc-200", "No conversation history" }
                    p { class: "text-xs text-zinc-400 leading-relaxed",
                        "Send a message below to begin interacting with the agent. Type "
                        code { class: "text-indigo-400 bg-zinc-800 px-1 py-0.5 rounded", "/error" }
                        " to test resilience and error fallback recovery."
                    }
                }
            } else {
                for message in msgs.iter() {
                    MessageBubble {
                        key: "{message.id}",
                        message: message.clone(),
                        on_retry: move |id| on_retry.call(id),
                    }
                }
            }

            // Bottom scroll tracking anchor
            div { id: "chat-bottom-anchor", class: "h-px w-full" }
        }
    }
}

// ============================================================================
// 4. INDIVIDUAL MESSAGE BUBBLE (MessageBubble)
// ============================================================================

/// Individual message bubble with distinct styles for User, Assistant, and System roles.
#[component]
pub fn MessageBubble(
    message: Message,
    on_retry: EventHandler<String>,
) -> Element {
    match &message.role {
        Role::System(source) => rsx! {
            div {
                class: "flex justify-center my-3",
                div {
                    class: "flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-zinc-900 border border-zinc-800 text-zinc-400 text-xs font-mono shadow-sm",
                    svg {
                        class: "w-3.5 h-3.5 text-zinc-500 fill-none stroke-current stroke-2",
                        view_box: "0 0 24 24",
                        circle { cx: "12", cy: "12", r: "10" }
                        line { x1: "12", y1: "16", x2: "12", y2: "12" }
                        line { x1: "12", y1: "8", x2: "12.01", y2: "8" }
                    }
                    span { class: "font-semibold text-zinc-300", "{source}:" }
                    span { "{message.content}" }
                }
            }
        },

        Role::User => rsx! {
            div {
                class: "flex justify-end gap-3 max-w-4xl ml-auto group",
                div {
                    class: "flex flex-col items-end max-w-[80%]",
                    div {
                        class: "relative px-4 py-3 rounded-2xl rounded-tr-sm bg-gradient-to-br from-indigo-600 to-indigo-700 text-white shadow-md text-sm leading-relaxed whitespace-pre-wrap select-text",
                        "{message.content}"
                    }
                    span {
                        class: "mt-1 text-[10px] font-mono text-zinc-500 pr-1",
                        "{message.formatted_time()}"
                    }
                }
                // User Avatar
                div {
                    class: "flex-none w-8 h-8 rounded-full bg-zinc-700 border border-zinc-600 flex items-center justify-center text-xs font-semibold text-zinc-200 shadow-sm",
                    "U"
                }
            }
        },

        Role::Assistant => rsx! {
            div {
                class: "flex justify-start gap-3 max-w-4xl mr-auto group",
                // Agent Avatar
                div {
                    class: "flex-none w-8 h-8 rounded-lg bg-zinc-900 border border-indigo-500/40 flex items-center justify-center text-indigo-400 shadow-sm shadow-indigo-950/40",
                    svg {
                        class: "w-4 h-4 fill-none stroke-current stroke-2",
                        view_box: "0 0 24 24",
                        path { d: "M12 2a10 10 0 0110 10c0 5.523-4.477 10-10 10S2 17.523 2 12A10 10 0 0112 2z" }
                        path { d: "M12 6v6l4 2" }
                    }
                }

                div {
                    class: "flex flex-col items-start max-w-[85%] space-y-2",

                    // Tool Calls Section (if any were invoked)
                    if !message.tool_calls.is_empty() {
                        div {
                            class: "w-full space-y-1.5",
                            for tool in message.tool_calls.iter() {
                                ToolCallBadge {
                                    key: "{tool.id}",
                                    tool: tool.clone(),
                                }
                            }
                        }
                    }

                    // Main Content Card
                    div {
                        class: "w-full px-4 py-3 rounded-2xl rounded-tl-sm bg-zinc-900/90 border border-zinc-800 text-zinc-200 text-sm leading-relaxed shadow-sm",

                        // Content or Streaming State
                        if message.content.is_empty() && message.status == MessageStatus::Streaming {
                            div {
                                class: "flex items-center gap-2 text-zinc-400 italic text-xs py-1",
                                span { class: "w-1.5 h-1.5 rounded-full bg-indigo-400 animate-bounce" }
                                span { class: "w-1.5 h-1.5 rounded-full bg-indigo-400 animate-bounce [animation-delay:0.2s]" }
                                span { class: "w-1.5 h-1.5 rounded-full bg-indigo-400 animate-bounce [animation-delay:0.4s]" }
                                span { "Agent is thinking and preparing tokens..." }
                            }
                        } else {
                            FormattedMessageContent {
                                content: message.content.clone(),
                                is_streaming: message.status == MessageStatus::Streaming,
                            }
                        }

                        // Error Resilience Banner
                        if let MessageStatus::Error(err_msg) = &message.status {
                            div {
                                class: "mt-3 p-3 rounded-xl bg-red-950/50 border border-red-800/60 text-red-300 text-xs flex flex-col gap-2",
                                div {
                                    class: "flex items-center gap-2 font-medium",
                                    svg {
                                        class: "w-4 h-4 text-red-400 flex-none fill-none stroke-current stroke-2",
                                        view_box: "0 0 24 24",
                                        circle { cx: "12", cy: "12", r: "10" }
                                        line { x1: "12", y1: "8", x2: "12", y2: "12" }
                                        line { x1: "12", y1: "16", x2: "12.01", y2: "16" }
                                    }
                                    span { "Generation interrupted" }
                                }
                                p { class: "text-red-400/90 pl-6 leading-normal font-mono text-[11px]", "{err_msg}" }
                                div {
                                    class: "pl-6 pt-1",
                                    button {
                                        class: "inline-flex items-center gap-1 px-2.5 py-1 bg-red-900/60 hover:bg-red-800/80 text-red-200 border border-red-700/50 rounded-lg text-xs font-medium transition-colors",
                                        onclick: {
                                            let id = message.id.clone();
                                            move |_| on_retry.call(id.clone())
                                        },
                                        svg {
                                            class: "w-3 h-3 fill-none stroke-current stroke-2",
                                            view_box: "0 0 24 24",
                                            path { d: "M1 4v6h6M23 20v-6h-6" }
                                            path { d: "M20.49 9A9 9 0 005.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 013.51 15" }
                                        }
                                        "Retry Query"
                                    }
                                }
                            }
                        }
                    }

                    span {
                        class: "text-[10px] font-mono text-zinc-500 pl-1",
                        "{message.formatted_time()}"
                    }
                }
            }
        }
    }
}

// ============================================================================
// 5. TOOL CALL BADGE & DRAWER (ToolCallBadge)
// ============================================================================

/// Renders tool invocations with an expandable inspected drawer for arguments and output.
#[component]
pub fn ToolCallBadge(tool: ToolCall) -> Element {
    let mut is_expanded = use_signal(|| false);

    rsx! {
        div {
            class: "w-full rounded-xl bg-zinc-900 border border-zinc-800/80 overflow-hidden text-xs shadow-sm transition-all",
            button {
                class: "w-full px-3 py-2 flex items-center justify-between bg-zinc-900/70 hover:bg-zinc-800/60 transition-colors text-left font-mono",
                onclick: move |_| is_expanded.set(!is_expanded()),
                div {
                    class: "flex items-center gap-2",
                    span {
                        class: "px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 text-[10px] font-semibold uppercase tracking-wider",
                        "Tool"
                    }
                    span { class: "text-zinc-200 font-semibold", "{tool.name}" }
                }
                div {
                    class: "flex items-center gap-2 text-zinc-400 text-[11px]",
                    if tool.output.is_some() {
                        span { class: "text-emerald-400 flex items-center gap-1",
                            span { class: "w-1.5 h-1.5 rounded-full bg-emerald-400" }
                            "Success"
                        }
                    } else {
                        span { class: "text-indigo-400 flex items-center gap-1",
                            span { class: "w-1.5 h-1.5 rounded-full bg-indigo-400 animate-pulse" }
                            "Running..."
                        }
                    }
                    span { class: "transform transition-transform text-zinc-500",
                        if is_expanded() { "▲" } else { "▼" }
                    }
                }
            }

            if is_expanded() {
                div {
                    class: "p-3 bg-zinc-950/70 border-t border-zinc-800/60 space-y-2 font-mono text-[11px]",
                    div {
                        span { class: "text-zinc-500 block mb-1 font-sans text-[10px] uppercase font-bold tracking-wider", "Arguments" }
                        pre { class: "p-2 rounded-lg bg-zinc-900 text-zinc-300 overflow-x-auto border border-zinc-800", "{tool.args}" }
                    }
                    if let Some(output) = &tool.output {
                        div {
                            span { class: "text-zinc-500 block mb-1 font-sans text-[10px] uppercase font-bold tracking-wider", "Result Output" }
                            pre { class: "p-2 rounded-lg bg-zinc-900 text-emerald-400 overflow-x-auto border border-zinc-800", "{output}" }
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// 6. FORMATTED MESSAGE CONTENT & STREAMING CURSOR
// ============================================================================

/// Renders text with basic code block awareness and an active streaming pulse cursor.
#[component]
pub fn FormattedMessageContent(content: String, is_streaming: bool) -> Element {
    // Parse content for markdown triple backtick code fences: ```lang ... ```
    let parts: Vec<&str> = content.split("```").collect();

    rsx! {
        div {
            class: "space-y-3 whitespace-pre-wrap leading-relaxed",
            for (idx, part) in parts.iter().enumerate() {
                if idx % 2 == 1 {
                    // Code block
                    div {
                        class: "my-2 rounded-xl bg-zinc-950 border border-zinc-800 overflow-hidden text-xs font-mono shadow-sm",
                        div {
                            class: "px-3 py-1.5 bg-zinc-900/90 border-b border-zinc-800 flex justify-between items-center text-zinc-400 text-[11px]",
                            span { "rust" }
                            span { class: "text-[10px] uppercase tracking-wider text-zinc-500", "Code Snippet" }
                        }
                        pre {
                            class: "p-3 overflow-x-auto text-zinc-200 leading-normal",
                            code { "{part.trim()}" }
                        }
                    }
                } else {
                    // Normal text segment
                    span {
                        "{part}"
                    }
                }
            }

            // Blinking cursor during token generation
            if is_streaming {
                span {
                    class: "inline-block w-2 h-4 ml-1 align-middle bg-indigo-400 animate-pulse rounded-[1px]",
                }
            }
        }
    }
}

// ============================================================================
// 7. CHAT INPUT BAR & GENERATION CONTROLS (ChatInput)
// ============================================================================

/// Textarea with keyboard handling (Enter to submit, Shift+Enter for newline) and dynamic Stop button.
#[component]
pub fn ChatInput(
    mut input_text: Signal<String>,
    is_generating: Signal<bool>,
    on_send: EventHandler<()>,
    on_stop: EventHandler<()>,
) -> Element {
    let generating = *is_generating.read();
    let current_val = input_text.read().clone();
    let has_text = !current_val.trim().is_empty();

    rsx! {
        footer {
            class: "flex-none p-4 md:p-6 bg-zinc-950 border-t border-zinc-800/80",
            div {
                class: "max-w-4xl mx-auto space-y-2",
                div {
                    class: "relative flex items-end rounded-2xl bg-zinc-900 border border-zinc-700/70 shadow-lg focus-within:border-indigo-500/80 focus-within:ring-2 focus-within:ring-indigo-500/20 transition-all p-2",

                    // Textarea Input
                    textarea {
                        class: "w-full max-h-36 min-h-[52px] bg-transparent text-zinc-100 placeholder-zinc-500 px-3 py-2 text-sm leading-relaxed resize-none focus:outline-none scrollbar-thin scrollbar-thumb-zinc-700",
                        placeholder: if generating { "Agent is currently replying..." } else { "Message Antigravity Agent... (Press Enter to send, Shift+Enter for new line)" },
                        value: "{current_val}",
                        rows: "1",
                        disabled: generating,
                        oninput: move |evt| input_text.set(evt.value()),
                        onkeydown: move |evt: KeyboardEvent| {
                            if evt.key() == Key::Enter && !evt.modifiers().shift() {
                                evt.prevent_default();
                                if has_text && !generating {
                                    on_send.call(());
                                }
                            }
                        }
                    }

                    // Dynamic Submit / Stop Button
                    div {
                        class: "flex-none ml-2 mb-1",
                        if generating {
                            button {
                                class: "flex items-center gap-1.5 px-3 py-2 rounded-xl bg-rose-500/10 text-rose-400 border border-rose-500/30 hover:bg-rose-500/20 transition-colors text-xs font-semibold shadow-sm",
                                onclick: move |_| on_stop.call(()),
                                svg {
                                    class: "w-3.5 h-3.5 fill-current",
                                    view_box: "0 0 24 24",
                                    rect { x: "6", y: "6", width: "12", height: "12", rx: "2" }
                                }
                                span { "Stop" }
                            }
                        } else {
                            button {
                                class: if has_text {
                                    "flex items-center justify-center w-9 h-9 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/30 transition-all cursor-pointer"
                                } else {
                                    "flex items-center justify-center w-9 h-9 rounded-xl bg-zinc-800 text-zinc-500 transition-all cursor-not-allowed"
                                },
                                disabled: !has_text,
                                onclick: move |_| {
                                    if has_text {
                                        on_send.call(());
                                    }
                                },
                                svg {
                                    class: "w-4 h-4 fill-none stroke-current stroke-2 transform -rotate-45 translate-x-0.5 -translate-y-0.5",
                                    view_box: "0 0 24 24",
                                    line { x1: "22", y1: "2", x2: "11", y2: "13" }
                                    polygon { points: "22 2 15 22 11 13 2 9 22 2" }
                                }
                            }
                        }
                    }
                }

                // Footer Prompt Hints
                div {
                    class: "flex items-center justify-between text-[11px] text-zinc-500 px-2 font-mono",
                    div {
                        class: "flex items-center gap-3",
                        span { "↵ Send" }
                        span { "⇧↵ New line" }
                    }
                    div {
                        span { "Antigravity UI • Rust Full-Stack Engine" }
                    }
                }
            }
        }
    }
}

// ============================================================================
// 8. KYO BACKEND CLIENT (HTTP / SSE STREAMING)
// ============================================================================

/// Resuelve la URL base: Sidecar local (si KYO_SIDECAR_PORT está presente) o Cloud Upstream.
pub fn resolve_api_base() -> String {
    if let Ok(port) = std::env::var("KYO_SIDECAR_PORT") {
        if let Ok(p) = port.trim().parse::<u16>() {
            return format!("http://127.0.0.1:{p}/api/v1");
        }
    }
    "https://kyo.broco.dev/api/v1".to_string()
}

/// Obtiene el token JWT desde la variable de entorno KYO_AUTH_TOKEN o del archivo %APPDATA%/kyo/session.json.
pub fn resolve_jwt_token() -> Option<String> {
    if let Ok(token) = std::env::var("KYO_AUTH_TOKEN") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let session_path = PathBuf::from(appdata).join("kyo").join("session.json");
        if session_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&session_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(token) = json.get("token").and_then(|t| t.as_str()) {
                        return Some(token.to_string());
                    }
                    if let Some(token) = json.get("access_token").and_then(|t| t.as_str()) {
                        return Some(token.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Consume el endpoint de streaming de IA de Kyo (POST /api/v1/ai/ask/stream) en tiempo real.
pub async fn stream_kyo_ask(
    prompt: String,
    history: Vec<HistoryMessage>,
    target_msg_id: String,
    mut messages: Signal<Vec<Message>>,
    mut is_generating: Signal<bool>,
) -> Result<(), String> {
    let api_base = resolve_api_base();
    let token = resolve_jwt_token().unwrap_or_default();
    let url = format!("{api_base}/ai/ask/stream");

    let payload = AskRequest {
        question: prompt,
        use_context: true,
        history,
    };

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let response = req
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            is_generating.set(false);
            format!("Error conectando con {url}: {e}")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        is_generating.set(false);
        return Err(format!("Error {status}: {err_text}"));
    }

    let mut event_stream = response.bytes_stream().eventsource();

    while let Some(event_res) = event_stream.next().await {
        match event_res {
            Ok(event) => {
                match event.event.as_str() {
                    // Fragmentos de texto en tiempo real
                    "delta" => {
                        if let Ok(delta) = serde_json::from_str::<DeltaPayload>(&event.data) {
                            let mut msgs = messages.write();
                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_msg_id) {
                                msg.content.push_str(&delta.text);
                            }
                            drop(msgs);
                            trigger_auto_scroll();
                        }
                    }

                    // Fuentes RAG encontradas en el workspace
                    "sources" => {
                        if let Ok(src_data) = serde_json::from_str::<SourcesPayload>(&event.data) {
                            println!("[Kyo RAG Sources] {} fuentes encontradas", src_data.sources.len());
                        }
                    }

                    // Acciones / Tool calls de agentes ejecutadas en segundo plano
                    "actions" => {
                        if let Ok(actions) = serde_json::from_str::<ActionsPayload>(&event.data) {
                            let mut msgs = messages.write();
                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_msg_id) {
                                for act in actions.actions {
                                    let tool_name = act
                                        .get("tool")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("tool")
                                        .to_string();
                                    msg.tool_calls.push(ToolCall {
                                        id: generate_unique_id(),
                                        name: tool_name,
                                        args: act.to_string(),
                                        output: None,
                                    });
                                }
                            }
                            drop(msgs);
                            trigger_auto_scroll();
                        }
                    }

                    // Acción MCP propuesta esperando confirmación humana
                    "pending" => {
                        if let Ok(pending_val) = serde_json::from_str::<serde_json::Value>(&event.data) {
                            println!("[Kyo Pending Action] Requiere confirmación: {:?}", pending_val);
                            let mut msgs = messages.write();
                            if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_msg_id) {
                                msg.tool_calls.push(ToolCall {
                                    id: generate_unique_id(),
                                    name: "mcp_pending_confirmation".to_string(),
                                    args: pending_val.to_string(),
                                    output: Some("Acción en espera de confirmación del usuario.".to_string()),
                                });
                            }
                            drop(msgs);
                            trigger_auto_scroll();
                        }
                    }

                    // Fin del stream: actualizar mensaje y desactivar bandera de generación
                    "done" => {
                        let mut msgs = messages.write();
                        if let Some(msg) = msgs.iter_mut().find(|m| m.id == target_msg_id) {
                            if let Ok(done) = serde_json::from_str::<DonePayload>(&event.data) {
                                // Fallback: si el texto consolidado viene en done (respuestas en caché)
                                if msg.content.trim().is_empty() {
                                    if let Some(answer) = done.answer {
                                        msg.content = answer;
                                    }
                                }
                            }
                            msg.status = MessageStatus::Sent;
                        }
                        drop(msgs);
                        is_generating.set(false);
                        trigger_auto_scroll();
                        break;
                    }

                    // Error retornado desde el backend
                    "error" => {
                        is_generating.set(false);
                        if let Ok(err_payload) = serde_json::from_str::<ErrorPayload>(&event.data) {
                            return Err(err_payload.error);
                        } else {
                            return Err(format!("Error SSE: {}", event.data));
                        }
                    }

                    _ => {}
                }
            }
            Err(err) => {
                eprintln!("[Kyo SSE Stream Error] {err}");
            }
        }
    }

    is_generating.set(false);
    Ok(())
}
