// Vista de monitoreo: estado del motor, latencia, niveles pre/post y el preset
// DSP activo en el escritorio.
//
// Espejo compacto de la "cabina" del escritorio, pensado para pantalla de
// teléfono: dial simplificado con barra de nivel y lecturas monoespaciadas.

import { StyleSheet, Text, View } from "react-native";
import type {
  AnalysisSample,
  DspState,
  EngineStatus,
  LevelSample,
  Suggestion,
} from "../lib/protocol";

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

/** Etiqueta en español del área de la voz de una sugerencia. */
const KIND_TEXT: Record<Suggestion["kind"], string> = {
  timbre: "Timbre",
  dynamics: "Dinámica",
  fatigue: "Fatiga",
  resonance: "Resonancia",
};

/** Etiqueta en español de cada tipo de banda del EQ. */
const EQ_KIND_TEXT: Record<string, string> = {
  lowShelf: "Shelf graves",
  peaking: "Pico",
  highShelf: "Shelf agudos",
};

/** Formatea una frecuencia en Hz a una etiqueta compacta. */
function formatFreq(freqHz: number): string {
  if (freqHz >= 1000) return `${(freqHz / 1000).toFixed(1)} kHz`;
  return `${freqHz} Hz`;
}

interface MonitorViewProps {
  status: EngineStatus | null;
  level: LevelSample | null;
  dsp: DspState | null;
  analysis: AnalysisSample | null;
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

export function MonitorView({ status, level, dsp, analysis }: MonitorViewProps) {
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

      {/* Ecualizador del preset activo (solo lectura) */}
      <View style={styles.infoCard}>
        <Text style={styles.cardTitle}>ECUALIZADOR</Text>
        {!dsp ? (
          <Text style={styles.emptyText}>Sin datos del ecualizador.</Text>
        ) : (
          (() => {
            const eq = dsp.links.find((link) => link.name === "eq");
            const bands = eq?.eqBands ?? null;
            if (!bands || bands.length === 0) {
              return (
                <Text style={styles.emptyText}>
                  El preset activo no tiene ecualizador.
                </Text>
              );
            }
            return (
              <View style={styles.eqGrid}>
                {bands.map((band, index) => (
                  <View
                    key={`${band.kind}-${band.freqHz}-${index}`}
                    style={[
                      styles.eqCell,
                      band.gainDb > 0
                        ? styles.eqCellBoost
                        : band.gainDb < 0
                          ? styles.eqCellCut
                          : null,
                    ]}
                  >
                    <Text style={styles.eqCellKind}>
                      {EQ_KIND_TEXT[band.kind] ?? band.kind}
                    </Text>
                    <Text style={styles.eqCellFreq}>
                      {formatFreq(band.freqHz)}
                    </Text>
                    <Text style={styles.eqCellGain}>
                      {band.gainDb > 0 ? "+" : ""}
                      {band.gainDb.toFixed(1)} dB
                    </Text>
                  </View>
                ))}
              </View>
            );
          })()
        )}
      </View>

      {/* Asistente vocal (solo lectura: el control es del escritorio) */}
      <View style={styles.infoCard}>
        <Text style={styles.cardTitle}>ASISTENTE</Text>
        {!analysis ? (
          <Text style={styles.emptyText}>
            {running
              ? "Analizando la voz… (se necesitan ~2 s de señal)."
              : "Sin análisis todavía."}
          </Text>
        ) : (
          <>
            <View style={styles.metricGrid}>
              <Metric label="Brillo" value={`${Math.round(analysis.metrics.brightness * 100)}%`} />
              <Metric
                label="Resonancia"
                value={`${Math.round(analysis.metrics.resonanceScore * 100)}%`}
              />
              <Metric
                label="Fatiga"
                value={`${Math.round(analysis.metrics.fatigueScore * 100)}%`}
              />
              <Metric
                label="Dinámica"
                value={`${analysis.metrics.dynamicRangeDb.toFixed(1)} dB`}
              />
            </View>
            {analysis.suggestions.length === 0 ? (
              <Text style={styles.emptyText}>Voz equilibrada: sin sugerencias.</Text>
            ) : (
              analysis.suggestions.map((suggestion) => (
                <View key={suggestion.id} style={styles.suggestionRow}>
                  <View style={styles.suggestionHead}>
                    <Text style={styles.suggestionKind}>
                      {KIND_TEXT[suggestion.kind] ?? suggestion.kind}
                    </Text>
                    <Text style={styles.suggestionSev}>
                      {Math.round(suggestion.severity * 100)}%
                    </Text>
                  </View>
                  <Text style={styles.suggestionMessage}>{suggestion.message}</Text>
                </View>
              ))
            )}
          </>
        )}
      </View>
    </View>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.metricCell}>
      <Text style={styles.metricValue}>{value}</Text>
      <Text style={styles.metricLabel}>{label}</Text>
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
  cardTitle: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 2,
    fontFamily: "monospace",
  },
  emptyText: {
    color: "#8a929c",
    fontSize: 12,
    fontStyle: "italic",
  },
  metricGrid: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
  },
  eqGrid: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
  },
  eqCell: {
    flexGrow: 1,
    flexBasis: "40%",
    backgroundColor: "#0c0e10",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    padding: 8,
    gap: 2,
  },
  eqCellBoost: {
    borderColor: "#4fd8ff",
  },
  eqCellCut: {
    borderColor: "#8a929c",
  },
  eqCellKind: {
    color: "#8a929c",
    fontSize: 9,
    letterSpacing: 1,
    textTransform: "uppercase",
    fontFamily: "monospace",
  },
  eqCellFreq: {
    color: "#8a929c",
    fontSize: 10,
    fontFamily: "monospace",
  },
  eqCellGain: {
    color: "#e6e9ec",
    fontSize: 13,
    fontFamily: "monospace",
    fontWeight: "700",
  },
  metricCell: {
    flexGrow: 1,
    flexBasis: "40%",
    backgroundColor: "#0c0e10",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    padding: 8,
    gap: 2,
  },
  metricValue: {
    color: "#4fd8ff",
    fontSize: 14,
    fontFamily: "monospace",
    fontWeight: "700",
  },
  metricLabel: {
    color: "#8a929c",
    fontSize: 9,
    letterSpacing: 1,
    textTransform: "uppercase",
  },
  suggestionRow: {
    backgroundColor: "#0c0e10",
    borderRadius: 8,
    borderWidth: 1,
    borderLeftWidth: 3,
    borderColor: "#2a2e34",
    borderLeftColor: "#ff4a1f",
    padding: 10,
    gap: 4,
  },
  suggestionHead: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  suggestionKind: {
    color: "#e6e9ec",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
  },
  suggestionSev: {
    color: "#8a929c",
    fontSize: 10,
    fontFamily: "monospace",
  },
  suggestionMessage: {
    color: "#8a929c",
    fontSize: 12,
    lineHeight: 17,
  },
});
