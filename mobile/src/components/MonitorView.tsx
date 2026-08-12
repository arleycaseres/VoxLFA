// Vista de monitoreo: estado del motor, latencia, niveles pre/post y el preset
// DSP activo en el escritorio.
//
// Espejo compacto de la "cabina" del escritorio, pensado para pantalla de
// teléfono: dial simplificado con barra de nivel y lecturas monoespaciadas.

import { StyleSheet, Text, View } from "react-native";
import type { DspState, EngineStatus, LevelSample } from "../lib/protocol";

/** Nivel que corresponde al 100% de la barra (dBFS). */
const DB_FULL = 0;
/** Nivel que corresponde al 0% de la barra (dBFS). */
const DB_EMPTY = -48;

const STATE_TEXT: Record<string, string> = {
  stopped: "DETENIDO",
  starting: "ARRANCANDO",
  running: "EN VIVO",
  stopping: "DETENIENDO",
  error: "ERROR",
};

function fillPercent(dbfs: number): number {
  const f = Math.min(1, Math.max(0, (dbfs - DB_EMPTY) / (DB_FULL - DB_EMPTY)));
  return f * 100;
}

function formatDb(dbfs: number): string {
  if (dbfs <= -100) return "-inf";
  return `${dbfs.toFixed(1)}`;
}

/** Etiqueta en español del preset activo. */
const PRESET_TEXT: Record<string, string> = {
  dry: "Sin procesar",
  vozLimpia: "Voz limpia",
  radio: "Radio",
  warm: "Warm",
};

interface MonitorViewProps {
  status: EngineStatus | null;
  level: LevelSample | null;
  dsp: DspState | null;
}

/** Barra de nivel vertical (cian, naranja al acercarse a 0 dBFS). */
function LevelBar({ label, valueDb, peakDb }: { label: string; valueDb: number; peakDb?: number }) {
  const fill = fillPercent(valueDb);
  const hot = valueDb >= -6;
  return (
    <View style={styles.barCol}>
      <View style={styles.barTrack}>
        <View
          style={[
            styles.barFill,
            hot ? styles.barFillHot : null,
            { height: `${fill}%` },
          ]}
        />
        {peakDb !== undefined && (
          <View style={[styles.barPeak, { bottom: `${fillPercent(peakDb)}%` }]} />
        )}
      </View>
      <Text style={styles.barLabel}>{label}</Text>
      <Text style={styles.barValue}>{formatDb(valueDb)}</Text>
    </View>
  );
}

export function MonitorView({ status, level, dsp }: MonitorViewProps) {
  const state = status?.state ?? "stopped";
  const running = state === "running";

  return (
    <View style={styles.container}>
      {/* Estado y latencia */}
      <View style={styles.headerRow}>
        <View style={[styles.pill, running ? styles.pillRunning : null]}>
          <View style={[styles.dot, running ? styles.dotRunning : null]} />
          <Text style={styles.pillText}>{STATE_TEXT[state] ?? state}</Text>
        </View>
        <View style={styles.latencyBox}>
          <Text style={styles.latencyValue}>
            {level ? `${level.latencyMs.toFixed(1)} ms` : "—"}
          </Text>
          <Text style={styles.latencyLabel}>LATENCIA</Text>
        </View>
      </View>

      {/* Niveles de señal (pre/post de la cadena DSP) */}
      <View style={styles.metersRow}>
        <View style={styles.meterGroup}>
          <Text style={styles.meterGroupTitle}>ENTRADA</Text>
          <View style={styles.meterBars}>
            <LevelBar label="RMS" valueDb={level?.inputRmsDb ?? -100} peakDb={level?.inputPeakDb} />
            <LevelBar label="PICO" valueDb={level?.inputPeakDb ?? -100} />
          </View>
        </View>
        <View style={styles.meterGroup}>
          <Text style={styles.meterGroupTitle}>SALIDA</Text>
          <View style={styles.meterBars}>
            <LevelBar label="RMS" valueDb={level?.outputRmsDb ?? -100} peakDb={level?.outputPeakDb} />
            <LevelBar label="PICO" valueDb={level?.outputPeakDb ?? -100} />
          </View>
        </View>
      </View>

      {/* Preset DSP activo */}
      <View style={styles.infoCard}>
        <InfoRow
          label="Preset"
          value={dsp ? (PRESET_TEXT[dsp.preset] ?? dsp.preset) : "—"}
        />
        <InfoRow
          label="Cadena"
          value={
            dsp
              ? dsp.globalBypass
                ? "BYPASS"
                : dsp.links
                    .filter((link) => !link.bypass)
                    .map((link) => link.name)
                    .join(" › ")
              : "—"
          }
        />
        <InfoRow label="Muestreo" value={status ? `${(status.sampleRate / 1000).toFixed(1)} kHz` : "—"} />
        <InfoRow label="Buffer" value={status ? `${status.bufferSize} smp` : "—"} />
        <InfoRow label="Entrada" value={status?.inputDevice ?? "—"} />
        <InfoRow label="Salida" value={status?.outputDevice ?? "—"} />
      </View>
    </View>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.infoRow}>
      <Text style={styles.infoLabel}>{label}</Text>
      <Text style={styles.infoValue} numberOfLines={1}>
        {value}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    gap: 16,
  },
  headerRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  pill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 999,
    borderWidth: 1,
    borderColor: "#2a2e34",
  },
  pillRunning: {
    borderColor: "rgba(79, 216, 255, 0.5)",
  },
  dot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: "#8a929c",
  },
  dotRunning: {
    backgroundColor: "#4fd8ff",
  },
  pillText: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  latencyBox: {
    alignItems: "flex-end",
  },
  latencyValue: {
    color: "#4fd8ff",
    fontSize: 22,
    fontFamily: "monospace",
    fontWeight: "700",
  },
  latencyLabel: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  metersRow: {
    flexDirection: "row",
    justifyContent: "space-evenly",
    gap: 8,
    backgroundColor: "#16181c",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#2a2e34",
    paddingVertical: 16,
    paddingHorizontal: 8,
  },
  meterGroup: {
    flex: 1,
    alignItems: "center",
    gap: 8,
  },
  meterGroupTitle: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  meterBars: {
    flexDirection: "row",
    justifyContent: "center",
    gap: 16,
    height: 190,
    alignSelf: "stretch",
  },
  barCol: {
    alignItems: "center",
    gap: 6,
    flex: 1,
  },
  barTrack: {
    flex: 1,
    width: 18,
    borderRadius: 4,
    backgroundColor: "#0c0e10",
    borderWidth: 1,
    borderColor: "#2a2e34",
    overflow: "hidden",
    justifyContent: "flex-end",
  },
  barFill: {
    width: "100%",
    backgroundColor: "#4fd8ff",
    borderRadius: 2,
  },
  barFillHot: {
    backgroundColor: "#ff4a1f",
  },
  barPeak: {
    position: "absolute",
    left: 0,
    right: 0,
    height: 2,
    backgroundColor: "#e6e9ec",
  },
  barLabel: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  barValue: {
    color: "#8a929c",
    fontSize: 11,
    fontFamily: "monospace",
  },
  infoCard: {
    backgroundColor: "#16181c",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#2a2e34",
    padding: 14,
    gap: 8,
  },
  infoRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    gap: 12,
  },
  infoLabel: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
  },
  infoValue: {
    color: "#e6e9ec",
    fontSize: 12,
    fontFamily: "monospace",
    flexShrink: 1,
    textAlign: "right",
  },
});
