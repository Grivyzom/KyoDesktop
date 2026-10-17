import { ref, computed } from 'vue';
import {
  getSidecarPort,
  onSidecarReady,
  checkSidecarHealth,
  saveSessionConfig,
  getSessionConfig,
  DEFAULT_MCP_SERVER_URL,
  type ConnectionStatus,
  type SidecarHealth,
} from '../lib/sidecarBridge';
import {
  probeMcpConnection,
  parseAndValidateJwt,
  sanitizeToken,
  validateServerUrl,
  type ConnectionProbeResult,
  type JwtValidationResult,
  type KyoAccountInfo,
  type KyoProfileInfo,
} from '../lib/mcpSecurity';

const sidecarPort = ref<number | null>(null);
const status = ref<ConnectionStatus>('detecting');
const healthData = ref<SidecarHealth | null>(null);
const serverUrl = ref<string>(DEFAULT_MCP_SERVER_URL);
const jwtToken = ref<string>('');
const latencyMs = ref<number | null>(null);
const lastProbeResult = ref<ConnectionProbeResult | null>(null);
const isVerifying = ref<boolean>(false);
const isInitialized = ref(false);

// Contador para ignorar respuestas obsoletas de refreshHealth concurrentes (M4)
let healthRequestId = 0;

const accountInfo = ref<KyoAccountInfo | null>(null);
const availableProfiles = ref<KyoProfileInfo[]>([]);
const activeProfileId = ref<string | null>(null);

const activeProfile = computed<KyoProfileInfo | null>(() => {
  if (!availableProfiles.value.length) return null;
  if (activeProfileId.value) {
    const found = availableProfiles.value.find((p) => p.id === activeProfileId.value);
    if (found) return found;
  }
  return availableProfiles.value[0] || null;
});

const activeApiBase = computed(() => {
  if (status.value === 'connected_local' && sidecarPort.value) {
    return `http://127.0.0.1:${sidecarPort.value}/api/v1`;
  }
  const cleanUrl = serverUrl.value.replace(/\/+$/, '');
  if (cleanUrl.endsWith('/api/v1')) {
    return cleanUrl;
  }
  return `${cleanUrl}/api/v1`;
});

const isConnected = computed(() =>
  status.value === 'connected_local' ||
  status.value === 'connected_remote' ||
  status.value === 'fallback_cloud'
);

const tokenValidation = computed<JwtValidationResult>(() => {
  if (!jwtToken.value) {
    return { valid: false, isExpired: false };
  }
  return parseAndValidateJwt(jwtToken.value);
});

export function useKyoBridge() {
  /**
   * Ejecuta el sondeo de conectividad: primero intenta con el Sidecar local (puerto efímero).
   * Si no hay sidecar o no responde, sondea la URL del servidor web / MCP remoto con medición de latencia y verificación de token.
   */
  async function refreshHealth(targetUrl?: string, targetToken?: string): Promise<ConnectionProbeResult | null> {
    const urlToTest = targetUrl !== undefined ? targetUrl : serverUrl.value;
    const tokenToTest = targetToken !== undefined ? targetToken : jwtToken.value;

    const currentRequestId = ++healthRequestId;
    isVerifying.value = true;

    // 1. Probar Sidecar Local en Go
    const port = sidecarPort.value ?? (await getSidecarPort());
    if (currentRequestId !== healthRequestId) { isVerifying.value = false; return null; }

    if (port) {
      sidecarPort.value = port;
      const start = performance.now();
      const health = await checkSidecarHealth(port);
      const loopbackLatency = Math.round(performance.now() - start);

      if (health) {
        latencyMs.value = loopbackLatency;
        healthData.value = health;
        status.value = 'connected_local';
        isVerifying.value = false;
        lastProbeResult.value = {
          reachable: true,
          authenticated: Boolean(tokenToTest),
          status: 'connected_verified',
          latencyMs: loopbackLatency,
          message: `Sidecar local activo en 127.0.0.1:${port}`,
        };
        return lastProbeResult.value;
      }
    }

    // 2. Si no hay sidecar o no responde, sondeo activo contra la aplicación web / endpoint MCP remoto
    healthData.value = null;
    const probe = await probeMcpConnection(urlToTest, tokenToTest);
    if (currentRequestId !== healthRequestId) { isVerifying.value = false; return probe; }

    lastProbeResult.value = probe;
    latencyMs.value = probe.reachable ? probe.latencyMs : null;

    if (probe.account) {
      accountInfo.value = probe.account;
    }
    if (probe.profiles && probe.profiles.length > 0) {
      availableProfiles.value = probe.profiles;
      if (!activeProfileId.value) {
        activeProfileId.value = probe.profiles[0].id;
      }
    }

    if (!probe.reachable) {
      status.value = 'offline';
    } else if (probe.status === 'invalid_token') {
      status.value = 'auth_error';
    } else if (probe.authenticated) {
      status.value = 'connected_remote';
    } else {
      status.value = 'fallback_cloud';
    }

    isVerifying.value = false;
    return probe;
  }

  async function initBridge() {
    if (isInitialized.value) return;
    isInitialized.value = true;

    // 1. Cargar configuración de sesión almacenada en Rust
    try {
      const config = await getSessionConfig();
      if (config.token) {
        jwtToken.value = config.token;
      }
      if (config.server_url) {
        serverUrl.value = config.server_url;
      }
      if (config.active_profile_id) {
        activeProfileId.value = config.active_profile_id;
      }
      if (config.username || config.email) {
        accountInfo.value = {
          id: '',
          username: config.username || '',
          name: config.username || '',
          email: config.email || '',
        };
      }
    } catch {
      // Ignorar si aún no hay configuración
    }

    // 2. Escuchar evento sidecar://ready
    try {
      await onSidecarReady(async (port) => {
        sidecarPort.value = port;
        await refreshHealth();
      });
    } catch (e) {
      console.warn('[useKyoBridge] No se pudo registrar listener sidecar://ready:', e);
    }

    // 3. Consulta y verificación inicial
    await refreshHealth();
  }

  /**
   * Cambia el perfil activo seleccionado y lo persiste en session.json.
   */
  async function setActiveProfile(profileId: string) {
    const found = availableProfiles.value.find((p) => p.id === profileId);
    if (!found) return;

    activeProfileId.value = found.id;
    await saveSessionConfig(
      jwtToken.value,
      serverUrl.value,
      found.id,
      found.name,
      accountInfo.value?.username,
      accountInfo.value?.email
    );
  }

  /**
   * Guarda de forma segura el enlace del servidor y el token JWT tras validación.
   */
  async function saveConnectionSettings(
    newUrl: string,
    newToken: string
  ): Promise<{ success: boolean; message: string; probe?: ConnectionProbeResult }> {
    const urlValidation = validateServerUrl(newUrl);
    if (!urlValidation.valid) {
      return {
        success: false,
        message: urlValidation.error || 'La URL ingresada no es válida.',
      };
    }

    const sanitized = sanitizeToken(newToken);

    if (sanitized) {
      const jwtCheck = parseAndValidateJwt(sanitized);
      if (!jwtCheck.valid && jwtCheck.isExpired) {
        return {
          success: false,
          message: jwtCheck.error || 'El token ingresado ha expirado.',
        };
      }
      if (!jwtCheck.valid) {
        return {
          success: false,
          message: jwtCheck.error || 'El formato del token no es un JWT válido.',
        };
      }
    }

    // Probar conectividad real contra el servidor antes de consolidar
    const probe = await refreshHealth(urlValidation.normalizedUrl, sanitized);

    if (probe && !probe.reachable) {
      return {
        success: false,
        message: `No se pudo conectar con el servidor: ${probe.message}`,
        probe,
      };
    }

    if (probe && probe.status === 'invalid_token') {
      return {
        success: false,
        message: `El servidor rechazó el token: ${probe.message}`,
        probe,
      };
    }

    // Persistir en backend Rust (%APPDATA%/kyo/session.json y variables de entorno)
    serverUrl.value = urlValidation.normalizedUrl;
    jwtToken.value = sanitized;
    const currentProfile = activeProfile.value;
    await saveSessionConfig(
      sanitized,
      urlValidation.normalizedUrl,
      currentProfile?.id,
      currentProfile?.name,
      accountInfo.value?.username,
      accountInfo.value?.email
    );

    return {
      success: true,
      message: sanitized
        ? 'Conexión verificada y token guardado correctamente.'
        : 'Conexión con el servidor guardada (acceso anónimo).',
      probe: probe ?? undefined,
    };
  }

  async function updateToken(newToken: string) {
    const sanitized = sanitizeToken(newToken);
    jwtToken.value = sanitized;
    const currentProfile = activeProfile.value;
    await saveSessionConfig(
      sanitized,
      serverUrl.value,
      currentProfile?.id,
      currentProfile?.name,
      accountInfo.value?.username,
      accountInfo.value?.email
    );
    await refreshHealth();
  }

  return {
    sidecarPort,
    status,
    healthData,
    serverUrl,
    jwtToken,
    latencyMs,
    activeApiBase,
    isConnected,
    lastProbeResult,
    tokenValidation,
    isVerifying,
    accountInfo,
    availableProfiles,
    activeProfileId,
    activeProfile,
    setActiveProfile,
    initBridge,
    refreshHealth,
    updateToken,
    saveConnectionSettings,
  };
}
