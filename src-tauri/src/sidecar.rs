use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct SidecarState {
    pub child: Arc<Mutex<Option<Child>>>,
    pub port: Arc<Mutex<Option<u16>>>,
}

impl Default for SidecarState {
    fn default() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            port: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
pub fn get_sidecar_port(state: State<'_, SidecarState>) -> Result<u16, String> {
    let port_guard = state.port.lock().map_err(|_| "Mutex poisoned")?;
    port_guard.ok_or_else(|| "Sidecar aún no ha reportado puerto".to_string())
}

#[tauri::command]
pub fn sidecar_port(state: State<'_, SidecarState>) -> Option<u16> {
    state.port.lock().ok().and_then(|guard| *guard)
}

pub fn get_sidecar_port_from_app(app: &AppHandle) -> Option<u16> {
    if let Some(state) = app.try_state::<SidecarState>() {
        state.port.lock().ok().and_then(|guard| *guard)
    } else {
        None
    }
}

pub fn kill_sidecar(app: &AppHandle) {
    if let Some(state) = app.try_state::<SidecarState>() {
        if let Ok(mut child_guard) = state.child.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Extrae el puerto de líneas como: "KYO_SIDECAR_PORT=54321" o "KYO_SIDECAR_READY port=54321 ..."
fn parse_port_from_output(line: &str) -> Option<u16> {
    if let Some(idx) = line.find("KYO_SIDECAR_PORT=") {
        let after = &line[idx + 17..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(p) = num_str.parse::<u16>() {
            return Some(p);
        }
    }
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

/// Localiza el binario ejecutable del Sidecar Go de forma segura y multiplataforma.
pub fn resolve_sidecar_exe() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    let bin_name = if cfg!(windows) { "go-server.exe" } else { "go-server" };
    let triple_bin = if cfg!(windows) {
        "go-server-x86_64-pc-windows-msvc.exe"
    } else {
        "go-server-x86_64-unknown-linux-gnu"
    };

    if let Ok(cur_exe) = std::env::current_exe() {
        if let Some(exe_dir) = cur_exe.parent() {
            candidates.push(exe_dir.join(bin_name));
            candidates.push(exe_dir.join(triple_bin));
            candidates.push(exe_dir.join("binaries").join(triple_bin));
        }
        let mut cur = cur_exe.clone();
        for _ in 0..5 {
            if cur.pop() {
                candidates.push(cur.join(bin_name));
                candidates.push(cur.join(triple_bin));
                candidates.push(cur.join("sidecar").join(bin_name));
                candidates.push(cur.join("src-tauri").join("binaries").join(triple_bin));
                candidates.push(cur.join("binaries").join(triple_bin));
            }
        }
    }

    // Rutas fijas de desarrollo
    candidates.push(std::path::PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\src-tauri\binaries\go-server-x86_64-pc-windows-msvc.exe"));
    candidates.push(std::path::PathBuf::from(r"V:\Proyectos\Grivyzom Ideas\tauri-app\sidecar\go-server.exe"));

    let mut existing: Vec<std::path::PathBuf> = candidates.into_iter().filter(|p| p.is_file()).collect();
    existing.sort_by(|a, b| {
        let time_b = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let time_a = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        time_b.cmp(&time_a)
    });

    existing.into_iter().next()
}

/// Inicia el subproceso Go Server y monitorea su stdout para el handshake
pub fn spawn(app: &AppHandle) {
    let app_handle = app.clone();

    // Determinar la ruta para réplicas locales u<id>.db
    let db_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.join("replicas").to_string_lossy().to_string())
        .unwrap_or_else(|_| "replicas".to_string());

    let exe = match resolve_sidecar_exe() {
        Some(path) => path,
        None => {
            eprintln!(
                "[Sidecar] Advertencia: No se pudo preparar el binario go-server (¿aún no compilado?)"
            );
            return;
        }
    };

    let parent_dir = exe.parent().unwrap_or(Path::new("."));
    let mut cmd = Command::new(&exe);
    cmd.current_dir(parent_dir);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("KYO_MODE", "local")
        .env("PORT", "0")
        .env("KYO_PORT", "0")
        .env("KYO_UPSTREAM", "https://kyo.broco.dev")
        .env("KYO_DB_DIR", &db_dir);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[Sidecar] Error al ejecutar el subproceso {}: {err}", exe.display());
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            eprintln!("[Sidecar] No se pudo capturar stdout del subproceso");
            return;
        }
    };

    let stderr = child.stderr.take();

    // Guardar la referencia del proceso hijo en el estado de Tauri
    if let Some(state) = app.try_state::<SidecarState>() {
        if let Ok(mut lock) = state.child.lock() {
            *lock = Some(child);
        }
    }

    println!("[Sidecar] Subproceso go-server lanzado exitosamente. Esperando handshake...");

    // Hilo para leer stderr del sidecar
    if let Some(stderr) = stderr {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    eprintln!("[Sidecar stderr] {l}");
                }
            }
        });
    }

    // Hilo para leer stdout y procesar el handshake
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    print!("[Sidecar stdout] {}\n", line);

                    if line.contains("KYO_SIDECAR_READY")
                        || line.contains("port=")
                        || line.contains("KYO_SIDECAR_PORT=")
                    {
                        if let Some(port) = parse_port_from_output(&line) {
                            println!("[Sidecar] ¡Handshake detectado! Puerto asignado: {port}");

                            // Guardar el puerto en el estado administrado
                            if let Some(state) = app_handle.try_state::<SidecarState>() {
                                if let Ok(mut port_lock) = state.port.lock() {
                                    *port_lock = Some(port);
                                }
                            }

                            // Emitir evento global a todas las ventanas webview de Tauri
                            let _ = app_handle.emit(
                                "sidecar://ready",
                                serde_json::json!({ "port": port }),
                            );
                        }
                    }
                }
                Err(err) => {
                    eprintln!("[Sidecar stdout error] {err}");
                    break;
                }
            }
        }

        // Si el stream de stdout termina, limpiar el puerto y emitir terminated
        if let Some(state) = app_handle.try_state::<SidecarState>() {
            if let Ok(mut port_lock) = state.port.lock() {
                *port_lock = None;
            }
        }
        let _ = app_handle.emit("sidecar://terminated", ());
    });
}
