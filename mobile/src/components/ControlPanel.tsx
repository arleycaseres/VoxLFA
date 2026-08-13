// Panel de control remoto del motor desde el móvil.
//
// Envía comandos `ControlCommand` por el WebSocket; el escritorio los ejecuta y
// el resultado vuelve como eventos (`dsp`, `status`…) que la UI ya refleja.
//
// El rango de ganancia del EQ y los pasos coinciden con la cabina de escritorio
// (±18 dB en pasos de 1 dB); el servidor acota el valor recibido por red.

import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import type {
  ControlCommand,
  DspState,
  EqBandKind,
  PresetId,
} from "../lib/protocol";

/** Rango de ganancia por banda (dB), igual que la cabina de escritorio. */
const GAIN_MIN = -18;
const GAIN_MAX = 18;
/** Paso de cada pulsación +/− (dB). */
const GAIN_STEP = 1;

/** Presets de la cabina con su etiqueta en español. */
const PRESETS: Array<{ id: PresetId; label: string }> = [
  { id: "dry", label: "Sin procesar" },
  { id: "vozLimpia", label: "Voz limpia" },
  { id: "radio", label: "Radio" },
  { id: "warm", label: "Warm" },
];

/** Nombre legible de cada tipo de banda (en español). */
const EQ_KIND_TEXT: Record<EqBandKind, string> = {
  lowShelf: "Shelf graves",
  peaking: "Pico",
  highShelf: "Shelf agudos",
};

/** Formatea una frecuencia en Hz a una etiqueta compacta. */
function formatFreq(freqHz: number): string {
  if (freqHz >= 1000) return `${(freqHz / 1000).toFixed(1)} kHz`;
  return `${freqHz} Hz`;
}

interface ControlPanelProps {
  /** `true` si el motor está en vivo (los comandos DSP fallan si no). */
  running: boolean;
  /** Último estado de la cadena DSP del escritorio. */
  dsp: DspState | null;
  /** Envía un comando de control al escritorio. */
  onCommand: (command: ControlCommand) => void;
}

export function ControlPanel({ running, dsp, onCommand }: ControlPanelProps) {
  const eqLink = dsp?.links.find((link) => link.name === "eq") ?? null;
  const bands = eqLink?.eqBands ?? null;
  const eqAvailable = bands !== null && bands.length > 0;
  const controlsEnabled = running && dsp !== null;

  const setGain = (bandIndex: number, deltaDb: number) => {
    const band = bands?.[bandIndex];
    if (!band) return;
    const clamped = Math.min(GAIN_MAX, Math.max(GAIN_MIN, band.gainDb + deltaDb));
    onCommand({ type: "setEqBand", bandIndex, gainDb: clamped });
  };

  return (
    <View style={styles.card}>
      <Text style={styles.cardTitle}>CONTROL REMOTO</Text>

      {/* Detener el motor (mando de emergencia) */}
      <TouchableOpacity
        style={[styles.stopButton, !running ? styles.controlDisabled : null]}
        onPress={() => onCommand({ type: "stop" })}
        disabled={!running}
        activeOpacity={0.8}
      >
        <Text style={styles.stopButtonText}>DETENER MOTOR</Text>
      </TouchableOpacity>

      {/* Preset activo */}
      <Text style={styles.sectionLabel}>Preset</Text>
      <View style={styles.chipRow}>
        {PRESETS.map((preset) => {
          const active = dsp?.preset === preset.id;
          return (
            <TouchableOpacity
              key={preset.id}
              style={[
                styles.chip,
                active ? styles.chipActive : null,
                !controlsEnabled ? styles.controlDisabled : null,
              ]}
              onPress={() => onCommand({ type: "setPreset", preset: preset.id })}
              disabled={!controlsEnabled}
              activeOpacity={0.7}
            >
              <Text style={[styles.chipText, active ? styles.chipTextActive : null]}>
                {preset.label}
              </Text>
            </TouchableOpacity>
          );
        })}
      </View>

      {/* Bypass global */}
      <TouchableOpacity
        style={[
          styles.bypassButton,
          dsp?.globalBypass ? styles.bypassButtonOn : null,
          !controlsEnabled ? styles.controlDisabled : null,
        ]}
        onPress={() =>
          onCommand({ type: "setGlobalBypass", bypass: !dsp?.globalBypass })
        }
        disabled={!controlsEnabled}
        activeOpacity={0.8}
      >
        <Text
          style={[
            styles.bypassText,
            dsp?.globalBypass ? styles.bypassTextOn : null,
          ]}
        >
          {dsp?.globalBypass ? "BYPASS GLOBAL ACTIVO" : "BYpass GLOBAL"}
        </Text>
      </TouchableOpacity>

      {/* Ecualizador por banda */}
      <Text style={styles.sectionLabel}>Ecualizador</Text>
      {!eqAvailable || !dsp ? (
        <Text style={styles.emptyText}>
          {!running
            ? "Arranca el motor para ajustar el EQ."
            : "El preset activo no incluye ecualizador."}
        </Text>
      ) : (
        <View style={styles.eqList}>
          {bands.map((band, index) => {
            const bypassed = (eqLink?.bypass ?? false) || dsp.globalBypass;
            return (
              <View key={`${band.kind}-${band.freqHz}-${index}`} style={styles.eqRow}>
                <View style={styles.eqInfo}>
                  <Text style={styles.eqKind}>
                    {EQ_KIND_TEXT[band.kind] ?? band.kind}
                  </Text>
                  <Text style={styles.eqFreq}>{formatFreq(band.freqHz)}</Text>
                </View>
                <View style={styles.eqGainRow}>
                  <Text style={styles.eqGain}>
                    {band.gainDb > 0 ? "+" : ""}
                    {band.gainDb.toFixed(1)} dB
                  </Text>
                  <TouchableOpacity
                    style={[styles.stepButton, !controlsEnabled || bypassed ? styles.controlDisabled : null]}
                    onPress={() => setGain(index, -GAIN_STEP)}
                    disabled={!controlsEnabled || bypassed}
                    activeOpacity={0.7}
                  >
                    <Text style={styles.stepText}>−</Text>
                  </TouchableOpacity>
                  <TouchableOpacity
                    style={[styles.stepButton, !controlsEnabled || bypassed ? styles.controlDisabled : null]}
                    onPress={() => setGain(index, +GAIN_STEP)}
                    disabled={!controlsEnabled || bypassed}
                    activeOpacity={0.7}
                  >
                    <Text style={styles.stepText}>+</Text>
                  </TouchableOpacity>
                </View>
              </View>
            );
          })}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    backgroundColor: "#16181c",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#2a2e34",
    padding: 14,
    gap: 8,
  },
  cardTitle: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  sectionLabel: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
    marginTop: 6,
  },
  stopButton: {
    marginTop: 4,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#ff4a1f",
    backgroundColor: "rgba(255, 74, 31, 0.12)",
    paddingVertical: 12,
    alignItems: "center",
  },
  stopButtonText: {
    color: "#ff4a1f",
    fontSize: 13,
    fontWeight: "700",
    letterSpacing: 1,
    textTransform: "uppercase",
  },
  chipRow: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
  },
  chip: {
    flexGrow: 1,
    flexBasis: "45%",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    backgroundColor: "#0c0e10",
    paddingVertical: 10,
    paddingHorizontal: 8,
    alignItems: "center",
  },
  chipActive: {
    borderColor: "#4fd8ff",
    backgroundColor: "rgba(79, 216, 255, 0.12)",
  },
  chipText: {
    color: "#8a929c",
    fontSize: 12,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  chipTextActive: {
    color: "#4fd8ff",
  },
  bypassButton: {
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    backgroundColor: "#0c0e10",
    paddingVertical: 12,
    alignItems: "center",
  },
  bypassButtonOn: {
    borderColor: "#4fd8ff",
    backgroundColor: "rgba(79, 216, 255, 0.12)",
  },
  bypassText: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
    fontWeight: "600",
  },
  bypassTextOn: {
    color: "#4fd8ff",
  },
  emptyText: {
    color: "#8a929c",
    fontSize: 12,
    fontStyle: "italic",
  },
  eqList: {
    gap: 8,
  },
  eqRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    backgroundColor: "#0c0e10",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    padding: 8,
    gap: 8,
  },
  eqInfo: {
    gap: 2,
    flex: 1,
  },
  eqKind: {
    color: "#8a929c",
    fontSize: 9,
    letterSpacing: 1,
    textTransform: "uppercase",
    fontFamily: "monospace",
  },
  eqFreq: {
    color: "#8a929c",
    fontSize: 10,
    fontFamily: "monospace",
  },
  eqGainRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  eqGain: {
    color: "#e6e9ec",
    fontSize: 13,
    fontFamily: "monospace",
    fontWeight: "700",
    minWidth: 64,
    textAlign: "right",
  },
  stepButton: {
    width: 36,
    height: 36,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    backgroundColor: "#16181c",
    alignItems: "center",
    justifyContent: "center",
  },
  stepText: {
    color: "#e6e9ec",
    fontSize: 18,
    fontWeight: "700",
    lineHeight: 20,
  },
  controlDisabled: {
    opacity: 0.4,
  },
});
