import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface SidecarHealth {
  status: string;
  mode?: string;
  port?: number;
  upstream?: string;
  db_dir?: string;
}

export type ConnectionStatus =
  | 'detecting'
  | 'connected_local'
  | 'connected_remote'
  | 'fallback_cloud'
  | 'auth_error'
  | 'offline';

export const DEFAULT_MCP_SERVER_URL = 'https://kyo.broco.dev';
export const REMOTE_CLOUD_URL = 'https://kyo.broco.dev/api/v1';

export interface SessionConfig {
  token?: string | null;
  server_url?: string | null;
  active_profile_id?: string | null;
  active_profile_name?: string | null;
  username?: string | null;
  email?: string | null;
  updated_at?: number | null;
}

/**
 * Consulta a Rust por el puerto efímero asignado al Sidecar Go.
 */
export async function getSidecarPort(): Promise<number | null> {
  try {
    const port = await invoke<number | null>('sidecar_port');
    return port ?? null;
  } catch (err) {
    console.warn('[KyoBridge] No se pudo consultar sidecar_port:', err);
    return null;
  }
}

/**
 * Escucha el evento global emitido por Rust cuando el sidecar está listo.
 */
export async function onSidecarReady(callback: (port: number) => void): Promise<UnlistenFn> {
  return await listen<{ port: number }>('sidecar://ready', (event) => {
    if (event.payload?.port) {
      callback(event.payload.port);
    }
  });
}

/**
 * Verifica la salud HTTP del sidecar en 127.0.0.1:<puerto>/health con timeout de 2.5s.
 */
export async function checkSidecarHealth(port: number): Promise<SidecarHealth | null> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 2500);

  try {
    const res = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: controller.signal,
    });
    clearTimeout(timeoutId);
    if (res.ok) {
      return (await res.json()) as SidecarHealth;
    }
  } catch {
    // Timeout o no responde
  } finally {
    clearTimeout(timeoutId);
  }
  return null;
}

/**
 * Guarda la configuración de sesión completa en Rust (%APPDATA%/kyo/session.json y variables de entorno).
 */
export async function saveSessionConfig(
  token: string,
  serverUrl?: string,
  activeProfileId?: string,
  activeProfileName?: string,
  username?: string,
  email?: string
): Promise<void> {
  try {
    await invoke('save_session_config', {
      token,
      serverUrl: serverUrl ?? null,
      activeProfileId: activeProfileId ?? null,
      activeProfileName: activeProfileName ?? null,
      username: username ?? null,
      email: email ?? null,
    });
  } catch (err) {
    console.error('[KyoBridge] Error guardando session config:', err);
    throw err;
  }
}

/**
 * Obtiene la configuración de sesión almacenada en el sistema.
 */
export async function getSessionConfig(): Promise<SessionConfig> {
  try {
    const config = await invoke<SessionConfig>('get_session_config');
    return config ?? {};
  } catch (err) {
    console.warn('[KyoBridge] No se pudo obtener session config:', err);
    return {};
  }
}

/**
 * Guarda el token de sesión en Rust (%APPDATA%/kyo/session.json y variable de entorno).
 * Mantiene compatibilidad regresiva.
 */
export async function saveSessionToken(token: string): Promise<void> {
  await saveSessionConfig(token);
}

/**
 * Obtiene el token de sesión almacenado en el sistema.
 * Mantiene compatibilidad regresiva.
 */
export async function getSessionToken(): Promise<string | null> {
  try {
    const config = await getSessionConfig();
    return config.token ?? null;
  } catch {
    return null;
  }
}
