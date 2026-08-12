// Badge de emparejamiento: muestra el código y la dirección LAN para que el
// usuario conecte la app móvil de monitoreo remoto.

import type { PairingInfo } from "../lib/tauri";

export function PairingBadge({ pairing }: { pairing: PairingInfo | null }) {
  if (!pairing) return null;
  return (
    <div className="pairing" title="Datos para conectar la app móvil">
      <span className="pairing__label">Móvil</span>
      <span className="pairing__code">{pairing.code}</span>
      {pairing.lanAddress && <span className="pairing__addr">{pairing.lanAddress}</span>}
    </div>
  );
}
