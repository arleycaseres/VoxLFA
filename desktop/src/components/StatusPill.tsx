// Píldora de estado del motor: color y texto según el estado actual.

import type { EngineState } from "../lib/types";

const STATE_TEXT: Record<EngineState, string> = {
  stopped: "DETENIDO",
  starting: "ARRANCANDO",
  running: "EN VIVO",
  stopping: "DETENIENDO",
  error: "ERROR",
};

export function StatusPill({ state }: { state: EngineState | null }) {
  const current = state ?? "stopped";
  return (
    <span className={`pill pill--${current}`} role="status">
      <span className="pill__dot" />
      {STATE_TEXT[current]}
    </span>
  );
}
