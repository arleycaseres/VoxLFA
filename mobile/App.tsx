// VoxLFA Monitor — monitoreo remoto del procesador vocal en vivo.
//
// El escritorio expone un WebSocket autenticado con código de emparejamiento;
// esta app se conecta y muestra estado, latencia y niveles en tiempo real.

import { StatusBar } from "expo-status-bar";
import {
  KeyboardAvoidingView,
  Platform,
  SafeAreaView,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { ConnectionForm } from "./src/components/ConnectionForm";
import { ControlPanel } from "./src/components/ControlPanel";
import { MonitorView } from "./src/components/MonitorView";
import { useRemoteEngine } from "./src/hooks/useRemoteEngine";

export default function App() {
  const remote = useRemoteEngine();
  const connected = remote.connState === "connected";

  return (
    <SafeAreaView style={styles.safe}>
      <StatusBar style="light" />
      <KeyboardAvoidingView
        style={styles.flex}
        behavior={Platform.OS === "ios" ? "padding" : undefined}
      >
        <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
          <View style={styles.header}>
            <Text style={styles.brand}>
              Vox<Text style={styles.brandAccent}>LFA</Text>
            </Text>
            <Text style={styles.tagline}>monitor en vivo</Text>
          </View>

          {remote.error && <Text style={styles.error}>{remote.error}</Text>}

          {connected ? (
            <>
              <ControlPanel
                running={remote.status?.state === "running"}
                dsp={remote.dsp}
                onCommand={remote.sendCommand}
              />
              <MonitorView
                status={remote.status}
                level={remote.level}
                spectrum={remote.spectrum}
                dsp={remote.dsp}
                analysis={remote.analysis}
              />
              <ConnectionForm
                connState={remote.connState}
                onConnect={remote.connect}
                onDisconnect={remote.disconnect}
              />
            </>
          ) : (
            <ConnectionForm
              connState={remote.connState}
              onConnect={remote.connect}
              onDisconnect={remote.disconnect}
            />
          )}
        </ScrollView>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safe: {
    flex: 1,
    backgroundColor: "#0b0d0f",
  },
  flex: {
    flex: 1,
  },
  content: {
    padding: 16,
    gap: 16,
  },
  header: {
    alignItems: "center",
    gap: 2,
    paddingVertical: 12,
  },
  brand: {
    color: "#e6e9ec",
    fontSize: 26,
    fontWeight: "700",
    letterSpacing: 3,
  },
  brandAccent: {
    color: "#ff4a1f",
  },
  tagline: {
    color: "#8a929c",
    fontSize: 10,
    letterSpacing: 3,
    textTransform: "uppercase",
    fontFamily: "monospace",
  },
  error: {
    color: "#ff4a1f",
    fontSize: 12,
    textAlign: "center",
    backgroundColor: "rgba(255, 74, 31, 0.08)",
    borderWidth: 1,
    borderColor: "rgba(255, 74, 31, 0.35)",
    borderRadius: 8,
    padding: 10,
  },
});
