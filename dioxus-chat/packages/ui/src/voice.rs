//! Módulo de Transcripción de Voz en Tiempo Real para Kyo Chat.
//!
//! Utiliza la Web Speech API (`SpeechRecognition` / `webkitSpeechRecognition`)
//! junto con captura de micrófono mediante `MediaDevices` y `AudioContext`
//! para visualización reactiva de niveles de audio y dictado en tiempo real.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// Eventos enviados desde el puente JavaScript en el WebView hacia Rust.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpeechEvent {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "interim")]
    Interim { text: String },
    #[serde(rename = "final")]
    Final { text: String },
    #[serde(rename = "audio_level")]
    AudioLevel { level: f32 },
    #[serde(rename = "error")]
    Error { error: String, message: String },
    #[serde(rename = "ended")]
    Ended,
}

/// Estado reactivo del reconocedor y transcriptor de voz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceState {
    pub is_listening: Signal<bool>,
    pub interim_transcript: Signal<String>,
    pub speech_error: Signal<Option<String>>,
    pub audio_level: Signal<f32>,
    pub lang: Signal<String>,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            is_listening: Signal::new(false),
            interim_transcript: Signal::new(String::new()),
            speech_error: Signal::new(None),
            audio_level: Signal::new(0.0),
            lang: Signal::new("es-ES".to_string()),
        }
    }
}

/// Inicia la transcripción en tiempo real mediante la Web Speech API en el WebView.
pub fn start_transcription(
    mut state: VoiceState,
    mut input_text: Signal<String>,
) {
    if *state.is_listening.read() {
        return;
    }

    state.speech_error.set(None);
    state.interim_transcript.set(String::new());
    state.is_listening.set(true);
    state.audio_level.set(0.0);

    let current_lang = state.lang.read().clone();
    let safe_lang = if current_lang.trim().is_empty() {
        "es-ES"
    } else {
        current_lang.trim()
    };

    let js_code = format!(
        r#"
        (() => {{
            const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (!SpeechRecognition) {{
                dioxus.send({{
                    type: "error",
                    error: "not_supported",
                    message: "Web Speech API no está disponible en este WebView2. Verifica que las características de voz de Windows y permisos de micrófono estén activos."
                }});
                return;
            }}

            if (window.__kyo_stop_speech) {{
                try {{ window.__kyo_stop_speech(); }} catch(e) {{}}
            }}

            let recognition = new SpeechRecognition();
            window.__kyo_recognition = recognition;
            recognition.continuous = true;
            recognition.interimResults = true;
            recognition.maxAlternatives = 1;
            recognition.lang = '{safe_lang}';

            let isManuallyStopped = false;

            window.__kyo_stop_speech = function() {{
                isManuallyStopped = true;
                try {{ recognition.stop(); }} catch(e) {{}}
                if (window.__kyo_mic_stream) {{
                    try {{ window.__kyo_mic_stream.getTracks().forEach(t => t.stop()); }} catch(e) {{}}
                    window.__kyo_mic_stream = null;
                }}
                if (window.__kyo_audio_ctx) {{
                    try {{ window.__kyo_audio_ctx.close(); }} catch(e) {{}}
                    window.__kyo_audio_ctx = null;
                }}
            }};

            // Captura de audio y analizador de frecuencia para el ecualizador reactivo
            if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {{
                navigator.mediaDevices.getUserMedia({{ audio: true }}).then(stream => {{
                    if (isManuallyStopped) {{
                        stream.getTracks().forEach(t => t.stop());
                        return;
                    }}
                    window.__kyo_mic_stream = stream;
                    try {{
                        const AudioCtx = window.AudioContext || window.webkitAudioContext;
                        if (AudioCtx) {{
                            const ctx = new AudioCtx();
                            window.__kyo_audio_ctx = ctx;
                            const src = ctx.createMediaStreamSource(stream);
                            const analyser = ctx.createAnalyser();
                            analyser.fftSize = 32;
                            src.connect(analyser);
                            const buf = new Uint8Array(analyser.frequencyBinCount);

                            let lastLvl = 0;
                            function checkLvl() {{
                                if (isManuallyStopped || !window.__kyo_audio_ctx) return;
                                analyser.getByteFrequencyData(buf);
                                let sum = 0;
                                for (let i = 0; i < buf.length; i++) sum += buf[i];
                                let avg = sum / buf.length;
                                let norm = Math.min(1.0, avg / 90.0);
                                if (Math.abs(norm - lastLvl) > 0.04) {{
                                    lastLvl = norm;
                                    dioxus.send({{ type: "audio_level", level: norm }});
                                }}
                                requestAnimationFrame(checkLvl);
                            }}
                            requestAnimationFrame(checkLvl);
                        }}
                    }} catch(e) {{
                        console.warn("[Kyo Voice] AudioContext no iniciado:", e);
                    }}
                }}).catch(err => {{
                    console.warn("[Kyo Voice] getUserMedia advertencia:", err);
                }});
            }}

            recognition.onstart = () => {{
                dioxus.send({{ type: "started" }});
            }};

            recognition.onresult = (event) => {{
                let interim = "";
                let finalChunk = "";
                for (let i = event.resultIndex; i < event.results.length; ++i) {{
                    const res = event.results[i];
                    if (res.isFinal) {{
                        finalChunk += res[0].transcript;
                    }} else {{
                        interim += res[0].transcript;
                    }}
                }}
                if (finalChunk.trim().length > 0) {{
                    dioxus.send({{ type: "final", text: finalChunk }});
                }}
                if (interim.trim().length > 0) {{
                    dioxus.send({{ type: "interim", text: interim }});
                }}
            }};

            recognition.onerror = (event) => {{
                if (isManuallyStopped) return;
                let msg = "Error en reconocimiento (" + event.error + ")";
                if (event.error === 'no-speech') {{
                    msg = "No se detectó voz. Intenta hablar más cerca del micrófono.";
                }} else if (event.error === 'not-allowed') {{
                    msg = "Permiso de micrófono denegado. Revisa la configuración de privacidad en Windows.";
                }} else if (event.error === 'network') {{
                    msg = "Error de red en el servicio de voz de WebView2. Verifica tu conexión.";
                }} else if (event.error === 'audio-capture') {{
                    msg = "No se encontró ningún micrófono de audio disponible.";
                }}
                dioxus.send({{ type: "error", error: event.error, message: msg }});
            }};

            recognition.onend = () => {{
                if (!isManuallyStopped) {{
                    try {{
                        recognition.start();
                        return;
                    }} catch(e) {{}}
                }}
                if (window.__kyo_mic_stream) {{
                    try {{ window.__kyo_mic_stream.getTracks().forEach(t => t.stop()); }} catch(e) {{}}
                    window.__kyo_mic_stream = null;
                }}
                if (window.__kyo_audio_ctx) {{
                    try {{ window.__kyo_audio_ctx.close(); }} catch(e) {{}}
                    window.__kyo_audio_ctx = null;
                }}
                dioxus.send({{ type: "ended" }});
            }};

            try {{
                recognition.start();
            }} catch(err) {{
                dioxus.send({{
                    type: "error",
                    error: "start_failed",
                    message: "No se pudo iniciar el reconocedor de voz: " + err.message
                }});
            }}
        }})();
        "#
    );

    spawn(async move {
        let mut eval = document::eval(&js_code);

        while let Ok(event) = eval.recv::<SpeechEvent>().await {
            match event {
                SpeechEvent::Started => {
                    state.is_listening.set(true);
                    state.speech_error.set(None);
                }
                SpeechEvent::Interim { text } => {
                    state.interim_transcript.set(text);
                }
                SpeechEvent::Final { text } => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let mut cur = input_text.read().clone();
                        if !cur.is_empty() && !cur.ends_with(' ') && !cur.ends_with('\n') {
                            cur.push(' ');
                        }
                        cur.push_str(trimmed);
                        cur.push(' ');
                        input_text.set(cur);
                    }
                    state.interim_transcript.set(String::new());
                }
                SpeechEvent::AudioLevel { level } => {
                    state.audio_level.set(level);
                }
                SpeechEvent::Error { error, message } => {
                    if error != "no-speech" {
                        state.speech_error.set(Some(message));
                    }
                    state.is_listening.set(false);
                    state.audio_level.set(0.0);
                    state.interim_transcript.set(String::new());
                    break;
                }
                SpeechEvent::Ended => {
                    state.is_listening.set(false);
                    state.audio_level.set(0.0);
                    state.interim_transcript.set(String::new());
                    break;
                }
            }
        }
    });
}

/// Detiene activamente la sesión de dictado y limpia los recursos del micrófono.
pub fn stop_transcription(mut state: VoiceState) {
    state.is_listening.set(false);
    state.audio_level.set(0.0);
    state.interim_transcript.set(String::new());

    let _ = document::eval("if (window.__kyo_stop_speech) { window.__kyo_stop_speech(); }");
}

/// Componente de ecualizador de audio visual animado con barras de amplitud.
#[component]
pub fn AudioWaveBars(level: f32) -> Element {
    let lvl = level.clamp(0.0, 1.0);
    // Calcular alturas para 5 barras con variaciones armónicas
    let h1 = (15.0 + lvl * 85.0 * 0.7).min(100.0);
    let h2 = (25.0 + lvl * 85.0 * 1.0).min(100.0);
    let h3 = (20.0 + lvl * 85.0 * 0.85).min(100.0);
    let h4 = (30.0 + lvl * 85.0 * 0.95).min(100.0);
    let h5 = (15.0 + lvl * 85.0 * 0.6).min(100.0);

    rsx! {
        div {
            class: "flex items-center gap-[3px] h-4 px-1",
            div {
                class: "w-1 bg-rose-400 rounded-full transition-all duration-75",
                style: format!("height: {h1}%;")
            }
            div {
                class: "w-1 bg-rose-500 rounded-full transition-all duration-75",
                style: format!("height: {h2}%;")
            }
            div {
                class: "w-1 bg-rose-400 rounded-full transition-all duration-75",
                style: format!("height: {h3}%;")
            }
            div {
                class: "w-1 bg-rose-500 rounded-full transition-all duration-75",
                style: format!("height: {h4}%;")
            }
            div {
                class: "w-1 bg-rose-400 rounded-full transition-all duration-75",
                style: format!("height: {h5}%;")
            }
        }
    }
}
