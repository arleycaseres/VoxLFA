// Parser del URL de emparejamiento que codifica el QR de la cabina.
//
// Formato emitido por el escritorio: `ws://<host>:<puerto>/?token=<código>`.
// El token equivale al mando remoto: no debe compartirse fuera de la red de
// confianza (ver `docs/seguridad.md`).

export interface PairingTarget {
  host: string;
  port: number;
  code: string;
}

/** Puerto por defecto del WebSocket de monitoreo (igual que el backend). */
const DEFAULT_PORT = 4356;

/**
 * Interpreta el contenido de un QR de emparejamiento.
 * Devuelve `null` si el valor no tiene el formato esperado.
 */
export function parsePairingUrl(raw: string): PairingTarget | null {
  let url: URL;
  try {
    url = new URL(raw.trim());
  } catch {
    return null;
  }
  const host = url.hostname;
  const port = url.port ? Number.parseInt(url.port, 10) : DEFAULT_PORT;
  const code = url.searchParams.get("token");
  if (!host || !code || Number.isNaN(port) || port < 1 || port > 65535) {
    return null;
  }
  return { host, port, code };
}
