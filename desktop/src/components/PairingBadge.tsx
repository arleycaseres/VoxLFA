// Badge de emparejamiento: muestra el código y la dirección LAN para que el
// usuario conecte la app móvil de monitoreo remoto. Al hacer clic, muestra el
// QR que la app móvil puede escanear para rellenar IP, puerto y código.

import { useEffect, useState } from "react";
import QRCode from "qrcode";
import type { PairingInfo } from "../lib/tauri";

/** URL WebSocket que codifica el QR (espejo del protocolo del backend). */
function pairingUrl(pairing: PairingInfo): string {
  const host = pairing.lanAddress ?? "127.0.0.1";
  return `ws://${host}:${pairing.port}/?token=${pairing.code}`;
}

export function PairingBadge({ pairing }: { pairing: PairingInfo | null }) {
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    if (!pairing) {
      setQrDataUrl(null);
      setExpanded(false);
      return;
    }
    let cancelled = false;
    QRCode.toDataURL(pairingUrl(pairing), {
      width: 240,
      margin: 1,
      color: { dark: "#0b0d0f", light: "#ffffff" },
    })
      .then((url) => {
        if (!cancelled) setQrDataUrl(url);
      })
      // Un fallo de generación no debe tumbar la cabina; el código y la
      // dirección siguen visibles en el badge.
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [pairing]);

  if (!pairing) return null;

  const canScan = Boolean(pairing.lanAddress);

  return (
    <div className="pairing" title="Datos para conectar la app móvil">
      <span className="pairing__label">Móvil</span>
      <span className="pairing__code">{pairing.code}</span>
      {pairing.lanAddress && <span className="pairing__addr">{pairing.lanAddress}</span>}
      {canScan && (
        <button
          className="pairing__qr-button"
          onClick={() => setExpanded(true)}
          type="button"
        >
          Ver código QR
        </button>
      )}

      {expanded && qrDataUrl && (
        <div
          className="pairing__overlay"
          onClick={() => setExpanded(false)}
          role="presentation"
        >
          <div
            className="pairing__modal"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
          >
            <img src={qrDataUrl} alt="Código QR de emparejamiento" className="pairing__qr" />
            <p className="pairing__hint">Escanea con la app VoxLFA Monitor</p>
            <button
              className="pairing__close"
              onClick={() => setExpanded(false)}
              type="button"
            >
              Cerrar
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
