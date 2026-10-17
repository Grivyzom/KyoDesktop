//! Gestor nativo del subproceso Sidecar en Go para Kyo Desktop.
//!
//! Reemplaza por completo el módulo de sidecar de Tauri:
//! - Localiza el binario `go-server.exe` en desarrollo y producción.
//! - Lanza el proceso sin ventana emergente en Windows (`CREATE_NO_WINDOW`).
//! - Detecta el puerto efímero asignado vía stdout (`KYO_SIDECAR_READY port=<N>`).
//! - Provee comprobación de salud (health check) y cálculo de latencia en ms.
//! - Garantiza el apagado limpio del proceso hijo al cerrar la aplicación.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Bandera de Windows para evitar que se abra una consola negra al lanzar el subproceso.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Referencia global al subproceso Go para garantizar su terminación al salir.
static SIDECAR_CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

/// Estado de conectividad de la aplicación.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SidecarConnectionStatus {
    Detecting,
    ConnectedLocal,
    FallbackCloud,
    Disconnected,
}

impl SidecarConnectionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Detecting => "Detectando...",
            Self::ConnectedLocal => "Conectado Local (Sidecar)",
            Self::FallbackCloud => "Fallback Cloud (Remoto)",
            Self::Disconnected => "Desconectado",
        }
    }

    pub fn color_classes(&self) -> (&'static str, &'static str, &'static str) {
        // (dot_bg, text_color, badge_bg)
        match self {
            Self::ConnectedLocal => ("bg-emerald-400", "text-emerald-400", "bg-emerald-500/10 border-emerald-500/20 text-emerald-300"),
            Self::FallbackCloud => ("bg-amber-400", "text-amber-400", "bg-amber-500/10 border-amber-500/20 text-amber-300"),
            Self::Detecting => ("bg-indigo-400 animate-pulse", "text-indigo-400", "bg-indigo-500/10 border-indigo-500/20 text-indigo-300"),
            Self::Disconnected => ("bg-rose-500", "text-rose-400", "bg-rose-500/10 border-rose-500/20 text-rose-300"),
        }
    }
}

/// Estado reactivo compartido del Sidecar en Dioxus.
#[derive(Clone, Debug, PartialEq)]
pub struct SidecarState {
    pub status: SidecarConnectionStatus,
    pub port: Option<u16>,
    pub latency_ms: Option<u64>,
    pub upstream: String,
    pub db_dir: String,
    pub error: Option<String>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            status: SidecarConnectionStatus::Detecting,
            port: None,
            latency_ms: None,
            upstream: "https://kyo.broco.dev".to_string(),
            db_dir: "replicas".to_string(),
            error: None,
        }
    }
}

/// Intenta localizar el ejecutable compilado del servidor Go (`go-server.exe`).
pub fn find_sidecar_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Relativo al ejecutable actual de Dioxus (desktop.exe)
    if let Ok(mut cur) = std::env::current_exe() {
        for _ in 0..5 {
            if cur.pop() {
                candidates.push(cur.join("go-server.exe"));
                candidates.push(cur.join("binaries").join("go-server-x86_64-pc-windows-msvc.exe"));
                candidates.push(cur.join("src-tauri").join("binaries").join("go-server-x86_64-pc-windows-msvc.exe"));
                candidates.push(cur.join("src-tauri").join("target").join("release").join("go-server.exe"));
                candidates.push(cur.join("sidecar").join("go-server.exe"));
            }
        }
    }

    // NOTA: No buscar en current_dir() por seguridad — evita ejecución de
    // binarios maliciosos colocados en el directorio de trabajo.

    // 3. Rutas fijas del proyecto de desarrollo
    candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\src-tauri\binaries\go-server-x86_64-pc-windows-msvc.exe"));
    candidates.push(PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\src-tauri\target\release\go-server.exe"));

    // Filtrar candidatos existentes y ordenar por fecha de modificación más reciente
    let mut existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.is_file()).collect();
    existing.sort_by(|a, b| {
        let time_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let time_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    existing.into_iter().next()
}

/// Extrae el puerto numérico de una línea de handshake como: "KYO_SIDECAR_READY port=54321 ..."
fn parse_port_from_line(line: &str) -> Option<u16> {
    if let Some(idx) = line.find("port=") {
        let after_port = &line[idx + 5..];
        let num_str: String = after_port
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        return num_str.parse::<u16>().ok();
    }
    None
}

/// Realiza un ping HTTP a `/health` en el puerto especificado para calcular la latencia en milisegundos.
pub async fn check_sidecar_health(port: u16) -> Option<u64> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2500))
        .build()
        .ok()?;

    let url = format!("http://127.0.0.1:{port}/health");
    let start = std::time::Instant::now();
    let res = client.get(&url).send().await.ok()?;

    if res.status().is_success() {
        let elapsed = start.elapsed().as_millis() as u64;
        Some(elapsed)
    } else {
        None
    }
}

/// Realiza un sondeo a la nube remota de Kyo (`/api/v1/health`) para medir latencia y validar conectividad.
pub async fn check_cloud_health() -> Option<u64> {
    let upstream = std::env::var("KYO_UPSTREAM")
        .or_else(|_| std::env::var("KYO_SERVER_URL"))
        .unwrap_or_else(|_| "https://kyo.broco.dev".to_string());

    let token = std::env::var("KYO_AUTH_TOKEN").ok();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(3500))
        .build()
        .ok()?;

    let clean = upstream.trim_end_matches('/');
    let url = if clean.ends_with("/api/v1") {
        format!("{clean}/health")
    } else {
        format!("{clean}/api/v1/health")
    };

    let mut req = client.get(&url);
    if let Some(ref t) = token {
        if !t.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", t.trim()));
        }
    }

    let start = std::time::Instant::now();
    let res = req.send().await.ok()?;

    if res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND || res.status() == reqwest::StatusCode::UNAUTHORIZED {
        Some(start.elapsed().as_millis() as u64)
    } else {
        None
    }
}

/// Inicia el subproceso Go en segundo plano y enlaza la detección a signals de Dioxus.
pub fn spawn_sidecar_service(mut sidecar_state: Signal<SidecarState>) {
    // Si ya hay un puerto establecido por el entorno, asignarlo de inmediato
    if let Ok(p_str) = std::env::var("KYO_SIDECAR_PORT") {
        if let Ok(p) = p_str.trim().parse::<u16>() {
            let mut state = sidecar_state.write();
            state.port = Some(p);
            state.status = SidecarConnectionStatus::ConnectedLocal;
            drop(state);
            return;
        }
    }

    let exe_path = match find_sidecar_executable() {
        Some(p) => p,
        None => {
            eprintln!("[Sidecar] No se encontró el ejecutable go-server. Activando Fallback Cloud.");
            let mut state = sidecar_state.write();
            state.status = SidecarConnectionStatus::FallbackCloud;
            state.error = Some("Binario go-server.exe no encontrado localmente".to_string());
            return;
        }
    };

    println!("[Sidecar] Iniciando go-server desde: {}", exe_path.display());

    // Configurar directorio de réplicas en APPDATA
    let db_dir = if let Ok(appdata) = std::env::var("APPDATA") {
        let path = Path::new(&appdata).join("kyo").join("replicas");
        let _ = std::fs::create_dir_all(&path);
        path.to_string_lossy().to_string()
    } else {
        "replicas".to_string()
    };

    let mut cmd = std::process::Command::new(&exe_path);
    cmd.current_dir(exe_path.parent().unwrap_or(Path::new(".")));
    cmd.env("PORT", "0");
    cmd.env("KYO_MODE", "local");
    cmd.env("KYO_UPSTREAM", "https://kyo.broco.dev");
    cmd.env("KYO_DB_DIR", &db_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.spawn() {
        Ok(mut child) => {
            let stdout = child.stdout.take();

            // Guardar child en mutex global para cierre garantizado al salir.
            // Terminar proceso anterior si existe para evitar zombies.
            if let Ok(mut lock) = SIDECAR_CHILD.lock() {
                if let Some(mut prev) = lock.take() {
                    eprintln!("[Sidecar] Terminando instancia anterior de go-server...");
                    let _ = prev.kill();
                    let _ = prev.wait();
                }
                *lock = Some(child);
            }

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u16>();

            // Hilo ligero para procesar stdout y detectar el puerto
            if let Some(stdout_stream) = stdout {
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader};
                    let reader = BufReader::new(stdout_stream);

                    for line_res in reader.lines() {
                        if let Ok(line) = line_res {
                            println!("[Sidecar stdout] {}", line);

                            if line.contains("KYO_SIDECAR_READY") || line.contains("port=") {
                                if let Some(port) = parse_port_from_line(&line) {
                                    println!("[Sidecar] ¡Handshake recibido! Puerto asignado: {port}");
                                    std::env::set_var("KYO_SIDECAR_PORT", port.to_string());
                                    let _ = tx.send(port);
                                    // NO hacer break: seguir leyendo stdout para evitar broken pipe en Go
                                }
                            }
                        }
                    }
                });
            }

            // Tarea asíncrona en el contexto de Dioxus para actualizar la señal reactiva
            let mut sc = sidecar_state;
            dioxus::prelude::spawn(async move {
                if let Some(port) = rx.recv().await {
                    let mut state = sc.write();
                    state.port = Some(port);
                    state.status = SidecarConnectionStatus::ConnectedLocal;
                    state.error = None;
                    drop(state);

                    if let Some(lat) = check_sidecar_health(port).await {
                        sc.write().latency_ms = Some(lat);
                    }
                }
            });
        }
        Err(err) => {
            eprintln!("[Sidecar Error] No se pudo lanzar el subproceso go-server: {err}");
            let mut state = sidecar_state.write();
            state.status = SidecarConnectionStatus::FallbackCloud;
            state.error = Some(format!("Error al ejecutar go-server: {err}"));
        }
    }
}

/// Termina el subproceso Go de manera segura si está activo.
pub fn terminate_sidecar() {
    if let Ok(mut lock) = SIDECAR_CHILD.lock() {
        if let Some(mut child) = lock.take() {
            println!("[Sidecar] Terminando subproceso go-server...");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
