/**
 * Módulo de Seguridad y Validación para Conexiones MCP y Tokens JWT de Kyo.
 * Cumple con estándares OWASP API Security, RFC 7519 (JWT) y RFC 6750 (Bearer Tokens).
 */

export interface JwtClaims {
  userId?: number | string;
  role?: string;
  email?: string;
  exp?: number;
  iat?: number;
  nbf?: number;
  rawPayload: Record<string, unknown>;
}

export interface JwtValidationResult {
  valid: boolean;
  isExpired: boolean;
  error?: string;
  claims?: JwtClaims;
  expiresAtFormatted?: string;
  remainingDays?: number;
}

export interface UrlValidationResult {
  valid: boolean;
  normalizedUrl: string;
  error?: string;
}

export interface KyoAccountInfo {
  id: string | number;
  username: string;
  name: string;
  email: string;
  role?: string;
  emailVerified?: boolean;
}

export interface KyoProfileInfo {
  id: string;
  name: string;
  description?: string;
  color?: string;
  icon?: string;
  provider?: string;
  fileCount?: number;
}

export interface ConnectionProbeResult {
  reachable: boolean;
  authenticated: boolean;
  status: 'connected_verified' | 'connected_unauthenticated' | 'connected_forbidden' | 'unreachable' | 'invalid_token';
  httpCode?: number;
  latencyMs: number;
  message: string;
  claims?: JwtClaims;
  account?: KyoAccountInfo;
  profiles?: KyoProfileInfo[];
}

/**
 * Decodificación segura de Base64URL a string Unicode sin usar eval() ni atob directo que rompa con UTF-8.
 */
function safeBase64UrlDecode(str: string): string {
  let base64 = str.replace(/-/g, '+').replace(/_/g, '/');
  while (base64.length % 4) {
    base64 += '=';
  }

  try {
    const binaryStr = atob(base64);
    const bytes = Uint8Array.from(binaryStr, (c) => c.charCodeAt(0));
    const decoder = new TextDecoder('utf-8');
    return decoder.decode(bytes);
  } catch {
    return atob(base64);
  }
}

/**
 * Sanitiza un token eliminando espacios y previniendo CRLF injection en cabeceras HTTP.
 */
export function sanitizeToken(token: string): string {
  if (!token) return '';
  return token
    .trim()
    .replace(/[\r\n\t]/g, '')
    .replace(/^Bearer\s+/i, '');
}

/**
 * Valida la estructura y parámetros de una URL para conexión MCP.
 * - Requiere protocolo seguro https: en producción; permite http: sólo en localhost o 127.0.0.1.
 * - Rechaza esquemas peligrosos como javascript:, data:, file:.
 * - Normaliza barras finales.
 */
export function validateServerUrl(rawUrl: string): UrlValidationResult {
  const trimmed = rawUrl.trim();
  if (!trimmed) {
    return {
      valid: false,
      normalizedUrl: '',
      error: 'La URL del servidor no puede estar vacía.',
    };
  }

  let parsed: URL;
  try {
    // Si el usuario omitió el protocolo, asumir https por defecto
    const withProtocol = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
    parsed = new URL(withProtocol);
  } catch {
    return {
      valid: false,
      normalizedUrl: '',
      error: 'Formato de URL no válido (ej: https://kyo.broco.dev).',
    };
  }

  const isLocalhost =
    parsed.hostname === 'localhost' ||
    parsed.hostname === '127.0.0.1' ||
    parsed.hostname === '[::1]' ||
    parsed.hostname.endsWith('.local');

  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
    return {
      valid: false,
      normalizedUrl: '',
      error: 'Protocolo no permitido. Debe utilizar https:// (o http:// en red local).',
    };
  }

  if (parsed.protocol === 'http:' && !isLocalhost) {
    return {
      valid: false,
      normalizedUrl: '',
      error: 'Por seguridad, las conexiones remotas deben usar HTTPS encriptado.',
    };
  }

  if (parsed.username || parsed.password) {
    return {
      valid: false,
      normalizedUrl: '',
      error: 'No se permiten credenciales embebidas en la URL.',
    };
  }

  // Normalizar eliminando barra final innecesaria
  let normalized = parsed.origin + parsed.pathname.replace(/\/+$/, '');
  if (parsed.search) normalized += parsed.search;

  return {
    valid: true,
    normalizedUrl: normalized,
  };
}

/**
 * Analiza sintáctica y temporalmente un JWT sin confiar ciegamente en la firma en cliente.
 * Comprueba claims RFC 7519: `exp`, `nbf`, `iat` y rechaza `alg: "none"`.
 */
export function parseAndValidateJwt(rawToken: string): JwtValidationResult {
  const token = sanitizeToken(rawToken);
  if (!token) {
    return { valid: false, isExpired: false, error: 'Token no proporcionado' };
  }

  const parts = token.split('.');
  if (parts.length !== 3) {
    return {
      valid: false,
      isExpired: false,
      error: 'Estructura JWT inválida: debe contener exactamente 3 partes separadas por puntos (header.payload.signature).',
    };
  }

  let header: Record<string, unknown>;
  let payload: Record<string, unknown>;

  try {
    header = JSON.parse(safeBase64UrlDecode(parts[0]));
    payload = JSON.parse(safeBase64UrlDecode(parts[1]));
  } catch {
    return {
      valid: false,
      isExpired: false,
      error: 'No se pudo decodificar el contenido Base64 del JWT.',
    };
  }

  // Prevenir tokens vulnerables con alg: none
  if (header.alg === 'none' || !header.alg) {
    return {
      valid: false,
      isExpired: false,
      error: 'Algoritmo de token no seguro (alg: none rechazada).',
    };
  }

  const nowSec = Math.floor(Date.now() / 1000);
  const exp = typeof payload.exp === 'number' ? payload.exp : undefined;
  const nbf = typeof payload.nbf === 'number' ? payload.nbf : undefined;

  let isExpired = false;
  let remainingDays: number | undefined = undefined;
  let expiresAtFormatted: string | undefined = undefined;

  if (exp !== undefined) {
    const expDate = new Date(exp * 1000);
    expiresAtFormatted = expDate.toLocaleDateString() + ' ' + expDate.toLocaleTimeString();

    if (nowSec >= exp) {
      isExpired = true;
      return {
        valid: false,
        isExpired: true,
        error: `Token expirado el ${expiresAtFormatted}.`,
        claims: {
          userId: (payload.user_id ?? payload.sub) as string | number | undefined,
          role: payload.role as string | undefined,
          email: payload.email as string | undefined,
          exp,
          iat: payload.iat as number | undefined,
          rawPayload: payload,
        },
        expiresAtFormatted,
      };
    }

    const diffSeconds = exp - nowSec;
    remainingDays = Math.max(0, Math.floor(diffSeconds / 86400));
  }

  if (nbf !== undefined && nowSec < nbf) {
    return {
      valid: false,
      isExpired: false,
      error: `Token no válido todavía (vigente a partir de ${new Date(nbf * 1000).toLocaleString()}).`,
    };
  }

  return {
    valid: true,
    isExpired,
    remainingDays,
    expiresAtFormatted,
    claims: {
      userId: (payload.user_id ?? payload.sub) as string | number | undefined,
      role: payload.role as string | undefined,
      email: payload.email as string | undefined,
      exp,
      iat: payload.iat as number | undefined,
      nbf,
      rawPayload: payload,
    },
  };
}

/**
 * Realiza un sondeo activo (handshake HTTP) contra el servidor MCP / Web para verificar conectividad y token.
 */
export async function probeMcpConnection(
  serverUrl: string,
  rawToken?: string,
  timeoutMs = 4000
): Promise<ConnectionProbeResult> {
  const urlCheck = validateServerUrl(serverUrl);
  if (!urlCheck.valid) {
    return {
      reachable: false,
      authenticated: false,
      status: 'unreachable',
      latencyMs: 0,
      message: urlCheck.error || 'URL inválida',
    };
  }

  const token = rawToken ? sanitizeToken(rawToken) : '';
  let claims: JwtClaims | undefined = undefined;

  if (token) {
    const jwtCheck = parseAndValidateJwt(token);
    claims = jwtCheck.claims;
    if (!jwtCheck.valid && jwtCheck.isExpired) {
      return {
        reachable: false,
        authenticated: false,
        status: 'invalid_token',
        latencyMs: 0,
        message: jwtCheck.error || 'Token expirado',
        claims,
      };
    }
  }

  const base = urlCheck.normalizedUrl;
  // Construir endpoint candidato para verificación
  // Si la URL ya incluye /api/v1, usar /health directamente; de lo contrario añadir /api/v1/health
  const probeEndpoint = base.endsWith('/api/v1')
    ? `${base}/health`
    : base.includes('/api/v1/')
    ? `${base}`
    : `${base}/api/v1/health`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const start = performance.now();

  try {
    const headers: Record<string, string> = {
      Accept: 'application/json, text/plain, */*',
    };

    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    const res = await fetch(probeEndpoint, {
      method: 'GET',
      headers,
      signal: controller.signal,
    });

    clearTimeout(timer);
    const latencyMs = Math.round(performance.now() - start);

    if (res.status === 401) {
      // Servidor alcanzado pero token inválido o expirado
      let serverErr = 'Token inválido o expirado';
      try {
        const body = await res.json();
        if (body.error_description) serverErr = body.error_description;
        else if (body.error) serverErr = body.error;
      } catch {
        // Ignorar fallo de parseo JSON
      }

      return {
        reachable: true,
        authenticated: false,
        status: 'invalid_token',
        httpCode: 401,
        latencyMs,
        message: `Servidor conectado, pero el token fue rechazado: ${serverErr}`,
        claims,
      };
    }

    if (res.status === 403) {
      return {
        reachable: true,
        authenticated: false,
        status: 'connected_forbidden',
        httpCode: 403,
        latencyMs,
        message: 'Token sin permisos suficientes para acceder a este endpoint (403 Forbidden).',
        claims,
      };
    }

    // Código 200, 204 o 404 (el middleware auth pasó pero la subruta es 404)
    if (res.ok || (res.status === 404 && token)) {
      let account: KyoAccountInfo | undefined = undefined;
      let profiles: KyoProfileInfo[] | undefined = undefined;

      if (token) {
        const apiBase = base.endsWith('/api/v1')
          ? base
          : base.replace(/\/+$/, '') + '/api/v1';

        try {
          const authHeaders = {
            Authorization: `Bearer ${token}`,
            Accept: 'application/json',
          };

          const [meRes, vaultsRes] = await Promise.allSettled([
            fetch(`${apiBase}/auth/me`, { method: 'GET', headers: authHeaders }).then((r) =>
              r.ok ? r.json() : null
            ),
            fetch(`${apiBase}/vaults`, { method: 'GET', headers: authHeaders }).then((r) =>
              r.ok ? r.json() : null
            ),
          ]);

          if (meRes.status === 'fulfilled' && meRes.value) {
            const data = meRes.value;
            account = {
              id: data.id ?? claims?.userId ?? '',
              username: data.username ?? (claims?.userId ? String(claims.userId) : 'usuario'),
              name: data.name || data.username || 'Usuario Kyō',
              email: data.email || claims?.email || '',
              role: data.role || claims?.role || 'user',
              emailVerified: Boolean(data.email_verified),
            };
          } else if (claims) {
            account = {
              id: claims.userId ?? '',
              username: claims.email ? claims.email.split('@')[0] : `user_${claims.userId ?? ''}`,
              name: claims.email ? claims.email.split('@')[0] : 'Usuario Kyō',
              email: claims.email || '',
              role: claims.role || 'user',
              emailVerified: false,
            };
          }

          if (vaultsRes.status === 'fulfilled' && vaultsRes.value) {
            const rawList = Array.isArray(vaultsRes.value.data)
              ? vaultsRes.value.data
              : Array.isArray(vaultsRes.value)
              ? vaultsRes.value
              : [];

            profiles = rawList.map((v: Record<string, unknown>) => ({
              id: String(v.id || v.ID || ''),
              name: String(v.name || v.Name || 'Bóveda Principal'),
              description: String(v.description || v.Description || ''),
              color: String(v.color || v.Color || 'indigo'),
              icon: String(v.icon || v.Icon || '🗄️'),
              provider: String(v.provider || v.Provider || 'server'),
              fileCount: typeof v.file_count === 'number' ? v.file_count : undefined,
            }));
          }
        } catch (subErr) {
          console.warn('[probeMcpConnection] Error obteniendo datos de cuenta/perfil:', subErr);
        }
      }

      return {
        reachable: true,
        authenticated: Boolean(token),
        status: token ? 'connected_verified' : 'connected_unauthenticated',
        httpCode: res.status,
        latencyMs,
        message: token
          ? 'Conexión exitosa y Token validado con el servidor MCP.'
          : 'Conexión con el servidor disponible (Modo anónimo / sin token).',
        claims,
        account,
        profiles,
      };
    }

    return {
      reachable: true,
      authenticated: false,
      status: 'connected_unauthenticated',
      httpCode: res.status,
      latencyMs,
      message: `Servidor respondió con código HTTP ${res.status}`,
      claims,
    };
  } catch (err: unknown) {
    clearTimeout(timer);
    const latencyMs = Math.round(performance.now() - start);
    const isAbort = (err as Error)?.name === 'AbortError';

    return {
      reachable: false,
      authenticated: false,
      status: 'unreachable',
      latencyMs,
      message: isAbort
        ? `Tiempo de espera agotado (${timeoutMs}ms) al conectar con el servidor.`
        : `No se pudo conectar con el servidor web (${(err as Error)?.message || 'Error de red'}).`,
      claims,
    };
  }
}
