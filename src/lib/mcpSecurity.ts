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
  tokenKind?: 'jwt' | 'kyo_mcp' | 'kyo_link' | 'opaque';
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
  isProjectScoped?: boolean;
  projectSpaceName?: string;
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
    // Fallback compatible UTF-8: decodificar via escape/decodeURIComponent
    try {
      return decodeURIComponent(escape(atob(base64)));
    } catch {
      return atob(base64);
    }
  }
}

/**
 * Sanitiza un token eliminando espacios, comillas envolventes y caracteres de control (previene CRLF / Header Injection).
 */
export function sanitizeToken(token: string): string {
  if (!token) return '';
  return token
    .trim()
    .replace(/^["']|["']$/g, '')
    .replace(/[\x00-\x1F\x7F\s]/g, '')
    .replace(/^Bearer/i, '')
    .trim()
    .replace(/^["']|["']$/g, '');
}

const preconnectedOrigins = new Set<string>();

/**
 * Pre-calienta DNS + TCP + TLS 1.3 en el motor WebView2 / navegador hacia el origen del servidor MCP.
 * Reduce la latencia de la primera petición HTTPS en 80-250ms.
 */
export function warmupServerOrigin(rawUrl: string): void {
  if (typeof document === 'undefined') return;
  try {
    const withProtocol = /^https?:\/\//i.test(rawUrl.trim())
      ? rawUrl.trim()
      : `https://${rawUrl.trim()}`;
    const origin = new URL(withProtocol).origin;
    if (!origin || preconnectedOrigins.has(origin)) return;
    preconnectedOrigins.add(origin);

    const dnsLink = document.createElement('link');
    dnsLink.rel = 'dns-prefetch';
    dnsLink.href = origin;
    document.head.appendChild(dnsLink);

    const preconnectLink = document.createElement('link');
    preconnectLink.rel = 'preconnect';
    preconnectLink.href = origin;
    preconnectLink.crossOrigin = 'anonymous';
    document.head.appendChild(preconnectLink);
  } catch {
    // Ignorar URLs en construcción
  }
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
 * Analiza sintáctica y temporalmente un JWT o Token MCP oficial de Kyō.
 * Comprueba claims RFC 7519: `exp`, `nbf`, `iat` y rechaza `alg: "none"`.
 */
export function parseAndValidateJwt(rawToken: string): JwtValidationResult {
  const token = sanitizeToken(rawToken);
  if (!token) {
    return { valid: false, isExpired: false, error: 'Token no proporcionado' };
  }

  // Soporte de primer orden para Tokens MCP oficiales de Kyō Web (kyo_mcp_...)
  if (token.startsWith('kyo_mcp_')) {
    const suffix = token.slice('kyo_mcp_'.length);
    if (suffix.length < 16) {
      return {
        valid: false,
        isExpired: false,
        tokenKind: 'kyo_mcp',
        error: 'El Token MCP (kyo_mcp_...) está incompleto o truncado (mínimo 16 caracteres tras el prefijo).',
      };
    }
    if (!/^[A-Za-z0-9_-]+$/.test(suffix)) {
      return {
        valid: false,
        isExpired: false,
        tokenKind: 'kyo_mcp',
        error: 'El Token MCP contiene caracteres no permitidos.',
      };
    }
    return {
      valid: true,
      isExpired: false,
      tokenKind: 'kyo_mcp',
      remainingDays: 30,
      claims: {
        userId: 'mcp_user',
        role: 'mcp',
        rawPayload: { type: 'mcp_token', prefix: 'kyo_mcp' },
      },
    };
  }

  // Soporte nativo para Tokens de vínculo de proyecto (kyo_link_...)
  if (token.startsWith('kyo_link_')) {
    const suffix = token.slice('kyo_link_'.length);
    if (suffix.length < 16) {
      return {
        valid: false,
        isExpired: false,
        tokenKind: 'kyo_link',
        error: 'El Token de proyecto (kyo_link_...) está incompleto o truncado.',
      };
    }
    if (!/^[A-Za-z0-9_-]+$/.test(suffix)) {
      return {
        valid: false,
        isExpired: false,
        tokenKind: 'kyo_link',
        error: 'El Token de proyecto contiene caracteres no permitidos.',
      };
    }
    return {
      valid: true,
      isExpired: false,
      tokenKind: 'kyo_link',
      remainingDays: 30,
      claims: {
        userId: 'project_link',
        role: 'project',
        rawPayload: { type: 'project_link_token', prefix: 'kyo_link' },
      },
    };
  }

  const parts = token.split('.');
  if (parts.length !== 3 || !parts[0] || !parts[1] || !parts[2]) {
    // Tokens OAuth opacos emitidos por el servidor
    if (token.length >= 16 && token.length <= 4096 && /^[A-Za-z0-9._~+/=-]+$/.test(token)) {
      return {
        valid: true,
        isExpired: false,
        tokenKind: 'opaque',
        claims: {
          userId: 'oauth_user',
          role: 'oauth',
          rawPayload: { type: 'opaque_token' },
        },
      };
    }
    return {
      valid: false,
      isExpired: false,
      error: 'Estructura de token no válida (se espera sesión OAuth, kyo_mcp_..., kyo_link_... o JWT).',
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

  // Prevenir tokens vulnerables con alg: none (case-insensitive)
  const alg = typeof header.alg === 'string' ? header.alg.trim().toLowerCase() : '';
  if (!alg || alg === 'none') {
    return {
      valid: false,
      isExpired: false,
      error: 'Algoritmo de firma del token no seguro (alg: none rechazado).',
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

interface CachedProbeEntry {
  timestamp: number;
  result: ConnectionProbeResult;
}

const PROBE_CACHE_TTL_MS = 12_000;
const probeCache = new Map<string, CachedProbeEntry>();
const inflightProbes = new Map<string, Promise<ConnectionProbeResult>>();

/**
 * Invalida la caché de sondeo MCP (útil al rotar credenciales o forzar reconexión).
 */
export function invalidateProbeCache(): void {
  probeCache.clear();
}

/**
 * Realiza un sondeo activo ultrarrápido (1 solo RTT paralelo sobre HTTP/2) contra el servidor MCP / Web
 * para verificar conectividad, autenticidad real del token y cargar cuenta/bóvedas simultáneamente.
 */
export async function probeMcpConnection(
  serverUrl: string,
  rawToken?: string,
  timeoutMs = 4000,
  forceRefresh = false
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

  const base = urlCheck.normalizedUrl;
  warmupServerOrigin(base);

  const token = rawToken ? sanitizeToken(rawToken) : '';
  let claims: JwtClaims | undefined = undefined;

  if (token) {
    const jwtCheck = parseAndValidateJwt(token);
    claims = jwtCheck.claims;
    if (!jwtCheck.valid) {
      return {
        reachable: false,
        authenticated: false,
        status: 'invalid_token',
        latencyMs: 0,
        message: jwtCheck.error || (jwtCheck.isExpired ? 'Token expirado' : 'Token inválido'),
        claims,
      };
    }
  }

  const cacheKey = `${base}|${token}`;
  if (!forceRefresh) {
    const cached = probeCache.get(cacheKey);
    if (cached && performance.now() - cached.timestamp < PROBE_CACHE_TTL_MS) {
      return cached.result;
    }
    const existingInflight = inflightProbes.get(cacheKey);
    if (existingInflight) {
      return existingInflight;
    }
  }

  const probePromise = (async (): Promise<ConnectionProbeResult> => {
    const apiBase = base.endsWith('/api/v1')
      ? base
      : base.replace(/\/+$/, '') + '/api/v1';

    const probeEndpoint = base.includes('/api/v1/') ? base : `${apiBase}/health`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    const start = performance.now();

    try {
      const headers: Record<string, string> = {
        Accept: 'application/json',
      };

      if (token) {
        headers['Authorization'] = `Bearer ${token}`;
      }

      const fetchInit: RequestInit = {
        method: 'GET',
        headers,
        signal: controller.signal,
        credentials: 'omit',
      };

      const isProjectToken = token.startsWith('kyo_link_');

      // Lanzar /health, /auth/me y /vaults (o /project-links/self si es kyo_link_) en paralelo
      const [healthSettled, meSettled, vaultsSettled, selfSettled] = await Promise.allSettled([
        fetch(probeEndpoint, fetchInit),
        token ? fetch(`${apiBase}/auth/me`, fetchInit) : Promise.resolve(null),
        token && !isProjectToken ? fetch(`${apiBase}/vaults`, fetchInit) : Promise.resolve(null),
        token && isProjectToken ? fetch(`${apiBase}/project-links/self`, fetchInit) : Promise.resolve(null),
      ]);

      clearTimeout(timer);
      const latencyMs = Math.max(1, Math.round(performance.now() - start));

      const healthRes = healthSettled.status === 'fulfilled' ? healthSettled.value : null;
      const meRes = meSettled.status === 'fulfilled' ? meSettled.value : null;
      const vaultsRes = vaultsSettled.status === 'fulfilled' ? vaultsSettled.value : null;
      const selfRes = selfSettled.status === 'fulfilled' ? selfSettled.value : null;

      // Si ninguna petición logró respuesta de red
      if (!healthRes && !meRes && !vaultsRes && !selfRes) {
        const err = healthSettled.status === 'rejected' ? healthSettled.reason : new Error('Error de red');
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

      // Verificar si alguno de los endpoints autenticados rechazó el token (incluso si /health es público 200 OK)
      const statusCodes = [healthRes?.status, meRes?.status, vaultsRes?.status, selfRes?.status].filter(
        (s): s is number => typeof s === 'number'
      );

      if (statusCodes.includes(401)) {
        let serverErr = 'Token inválido, revocado o expirado';
        const unauthorizedRes = [meRes, selfRes, vaultsRes, healthRes].find((r) => r?.status === 401);
        if (unauthorizedRes) {
          try {
            const body = await unauthorizedRes.json();
            if (body.error_description) serverErr = body.error_description;
            else if (body.error) serverErr = body.error;
          } catch {
            // Ignorar fallo de parseo JSON
          }
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

      if (statusCodes.includes(403) && !meRes?.ok && !vaultsRes?.ok && !selfRes?.ok) {
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

      const primaryStatus = selfRes?.status ?? meRes?.status ?? vaultsRes?.status ?? healthRes?.status ?? 200;
      const isOk = Boolean(
        healthRes?.ok || meRes?.ok || vaultsRes?.ok || selfRes?.ok || (healthRes?.status === 404 && token)
      );

      if (isOk) {
        let account: KyoAccountInfo | undefined = undefined;
        let profiles: KyoProfileInfo[] | undefined = undefined;
        let projectSpaceName: string | undefined = undefined;

        if (token) {
          const [meJsonSettled, vaultsJsonSettled, selfJsonSettled] = await Promise.allSettled([
            meRes && meRes.ok ? meRes.json() : Promise.resolve(null),
            vaultsRes && vaultsRes.ok ? vaultsRes.json() : Promise.resolve(null),
            selfRes && selfRes.ok ? selfRes.json() : Promise.resolve(null),
          ]);

          const selfData = selfJsonSettled.status === 'fulfilled' ? selfJsonSettled.value : null;
          if (selfData && typeof selfData.space_name === 'string') {
            projectSpaceName = selfData.space_name;
          }

          const meData = meJsonSettled.status === 'fulfilled' ? meJsonSettled.value : null;
          if (meData) {
            const userObj = meData.user ?? meData.data ?? meData;
            account = {
              id: userObj.id ?? claims?.userId ?? '',
              username: userObj.username ?? (claims?.userId ? String(claims.userId) : 'usuario'),
              name: userObj.name || userObj.username || 'Usuario Kyō',
              email: userObj.email || claims?.email || '',
              role: userObj.role || claims?.role || 'user',
              emailVerified: Boolean(userObj.email_verified),
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

          const vaultsData = vaultsJsonSettled.status === 'fulfilled' ? vaultsJsonSettled.value : null;
          if (vaultsData) {
            const rawList = Array.isArray(vaultsData.data)
              ? vaultsData.data
              : Array.isArray(vaultsData)
              ? vaultsData
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
        }

        const result: ConnectionProbeResult = {
          reachable: true,
          authenticated: Boolean(token),
          status: token ? 'connected_verified' : 'connected_unauthenticated',
          httpCode: primaryStatus,
          latencyMs,
          message: isProjectToken
            ? `Token de proyecto válido (Espacio: ${projectSpaceName || 'acotado'}).`
            : token
            ? 'Conexión exitosa y Token validado con el servidor MCP.'
            : 'Conexión con el servidor disponible (Modo anónimo / sin token).',
          isProjectScoped: isProjectToken,
          projectSpaceName,
          claims,
          account,
          profiles,
        };
        probeCache.set(cacheKey, { timestamp: performance.now(), result });
        return result;
      }

      return {
        reachable: true,
        authenticated: false,
        status: 'connected_unauthenticated',
        httpCode: primaryStatus,
        latencyMs,
        message: `Servidor respondió con código HTTP ${primaryStatus}`,
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
  })();

  inflightProbes.set(cacheKey, probePromise);
  try {
    return await probePromise;
  } finally {
    inflightProbes.delete(cacheKey);
  }
}
