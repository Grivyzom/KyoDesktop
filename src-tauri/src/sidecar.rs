use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

/// Estado administrado en Tauri para retener el puerto efímero del Sidecar Go.
#[derive(Default)]
pub struct SidecarState {
    pub port: Mutex<Option<u16>>,
}

/// Comando IPC para que el frontend consulte el puerto actual del sidecar
#[tauri::command]
pub fn sidecar_port(state: State<SidecarState>) -> Option<u16> {
    *state.port.lock().unwrap()
}

/// Helper en Rust para obtener el puerto desde cualquier parte del host Tauri
pub fn get_sidecar_port(app: &AppHandle) -> Option<u16> {
    if let Some(state) = app.try_state::<SidecarState>() {
        *state.port.lock().unwrap()
    } else {
        None
    }
}

/// Extrae el puerto de una línea como: "KYO_SIDECAR_READY port=54321 ..."
fn parse_port_from_output(line: &str) -> Option<u16> {
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

/// Inicia el subproceso Go Server y monitorea su stdout para el handshake
pub fn spawn(app: &AppHandle) {
    let app_handle = app.clone();

    // Determinar la ruta para réplicas locales u<id>.db
    let db_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.join("replicas").to_string_lossy().to_string())
        .unwrap_or_else(|_| "replicas".to_string());

    let sidecar_cmd = match app.shell().sidecar("go-server") {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!(
                "[Sidecar] Advertencia: No se pudo preparar el binario go-server (¿aún no compilado?): {err}"
            );
            return;
        }
    };

    let sidecar_cmd = sidecar_cmd
        .env("KYO_MODE", "local")
        .env("PORT", "0")
        .env("KYO_UPSTREAM", "https://kyo.broco.dev")
        .env("KYO_DB_DIR", &db_dir);

    tauri::async_runtime::spawn(async move {
        match sidecar_cmd.spawn() {
            Ok((mut rx, _child)) => {
                println!("[Sidecar] Subproceso go-server lanzado exitosamente. Esperando handshake...");

                while let Some(event) = rx.recv().await {
                    match event {
                        CommandEvent::Stdout(bytes) => {
                            let line = String::from_utf8_lossy(&bytes);
                            print!("[Sidecar stdout] {}", line);

                            if line.contains("KYO_SIDECAR_READY") || line.contains("port=") {
                                if let Some(port) = parse_port_from_output(&line) {
                                    println!("[Sidecar] ¡Handshake detectado! Puerto asignado: {port}");

                                    // Guardar el puerto en el estado administrado
                                    if let Some(state) = app_handle.try_state::<SidecarState>() {
                                        *state.port.lock().unwrap() = Some(port);
                                    }

                                    // Emitir evento global a todas las ventanas webview de Tauri
                                    let _ = app_handle.emit(
                                        "sidecar://ready",
                                        serde_json::json!({ "port": port }),
                                    );
                                }
                            }
                        }
                        CommandEvent::Stderr(bytes) => {
                            let err_line = String::from_utf8_lossy(&bytes);
                            eprint!("[Sidecar stderr] {}", err_line);
                        }
                        CommandEvent::Error(err) => {
                            eprintln!("[Sidecar error] Error en el subproceso: {err}");
                        }
                        CommandEvent::Terminated(payload) => {
                            println!(
                                "[Sidecar] Subproceso go-server finalizó con código: {:?}",
                                payload.code
                            );
                            if let Some(state) = app_handle.try_state::<SidecarState>() {
                                *state.port.lock().unwrap() = None;
                            }
                            let _ = app_handle.emit("sidecar://terminated", ());
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(err) => {
                eprintln!("[Sidecar] Error al ejecutar el subproceso go-server: {err}");
            }
        }
    });
}
