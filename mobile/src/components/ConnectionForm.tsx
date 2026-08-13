// Formulario de conexión con el escritorio VoxLFA.
//
// Pide IP, puerto (predeterminado 4356) y el código de emparejamiento que
// muestra la cabina del escritorio.

import { useState } from "react";
import {
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from "react-native";
import type { ConnectionState } from "../hooks/useRemoteEngine";
import type { PairingTarget } from "../lib/pairingUrl";
import { QrScanner } from "./QrScanner";

interface ConnectionFormProps {
  connState: ConnectionState;
  onConnect: (host: string, port: number, code: string) => void;
  onDisconnect: () => void;
}

const DEFAULT_PORT = "4356";

const BUTTON_LABEL: Record<ConnectionState, string> = {
  idle: "Conectar",
  connecting: "Conectando…",
  connected: "Conectado",
  reconnecting: "Reconectando…",
  error: "Reintentar",
};

export function ConnectionForm({ connState, onConnect, onDisconnect }: ConnectionFormProps) {
  const [host, setHost] = useState("");
  const [port, setPort] = useState(DEFAULT_PORT);
  const [code, setCode] = useState("");
  const [scanning, setScanning] = useState(false);

  const connected = connState === "connected";

  const handleSubmit = () => {
    const portNumber = Number.parseInt(port, 10);
    if (connected) {
      onDisconnect();
    } else {
      onConnect(host, portNumber, code);
    }
  };

  const handleScanned = (target: PairingTarget) => {
    setHost(target.host);
    setPort(String(target.port));
    setCode(target.code);
    setScanning(false);
    onConnect(target.host, target.port, target.code);
  };

  if (scanning) {
    return (
      <QrScanner
        onScanned={handleScanned}
        onCancel={() => setScanning(false)}
      />
    );
  }

  return (
    <View style={styles.card}>
      <Text style={styles.title}>Conectar al escritorio</Text>

      <Text style={styles.label}>Dirección IP</Text>
      <TextInput
        style={styles.input}
        value={host}
        onChangeText={setHost}
        placeholder="192.168.1.10"
        placeholderTextColor="#5b636d"
        autoCapitalize="none"
        autoCorrect={false}
        editable={!connected}
        keyboardType="decimal-pad"
      />

      <View style={styles.row}>
        <View style={styles.rowItem}>
          <Text style={styles.label}>Puerto</Text>
          <TextInput
            style={styles.input}
            value={port}
            onChangeText={setPort}
            placeholder={DEFAULT_PORT}
            placeholderTextColor="#5b636d"
            keyboardType="number-pad"
            editable={!connected}
          />
        </View>
        <View style={styles.rowItem}>
          <Text style={styles.label}>Código de emparejamiento</Text>
          <TextInput
            style={[styles.input, styles.codeInput]}
            value={code}
            onChangeText={setCode}
            placeholder="ABC123"
            placeholderTextColor="#5b636d"
            autoCapitalize="characters"
            autoCorrect={false}
            editable={!connected}
            maxLength={12}
          />
        </View>
      </View>

      <TouchableOpacity
        style={[styles.button, connected ? styles.buttonStop : styles.buttonStart]}
        onPress={handleSubmit}
        activeOpacity={0.8}
      >
        <Text style={styles.buttonText}>{BUTTON_LABEL[connState]}</Text>
      </TouchableOpacity>

      {!connected && (
        <TouchableOpacity
          style={styles.buttonGhost}
          onPress={() => setScanning(true)}
          activeOpacity={0.8}
        >
          <Text style={styles.buttonText}>Escanear código QR</Text>
        </TouchableOpacity>
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
    padding: 16,
    gap: 6,
  },
  title: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 2,
    textTransform: "uppercase",
    fontFamily: "monospace",
    marginBottom: 4,
  },
  label: {
    color: "#8a929c",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
    marginTop: 6,
  },
  input: {
    backgroundColor: "#1e2228",
    color: "#e6e9ec",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "#2a2e34",
    paddingHorizontal: 12,
    paddingVertical: 10,
    fontSize: 15,
  },
  codeInput: {
    fontFamily: "monospace",
    letterSpacing: 4,
  },
  row: {
    flexDirection: "row",
    gap: 10,
  },
  rowItem: {
    flex: 1,
  },
  button: {
    marginTop: 12,
    borderRadius: 8,
    paddingVertical: 14,
    alignItems: "center",
    borderWidth: 1,
  },
  buttonStart: {
    backgroundColor: "rgba(79, 216, 255, 0.12)",
    borderColor: "#4fd8ff",
  },
  buttonStop: {
    backgroundColor: "rgba(255, 74, 31, 0.12)",
    borderColor: "#ff4a1f",
  },
  buttonText: {
    color: "#e6e9ec",
    fontSize: 14,
    fontWeight: "600",
    letterSpacing: 1,
    textTransform: "uppercase",
  },
  buttonGhost: {
    marginTop: 8,
    borderColor: "#2a2e34",
    borderRadius: 8,
    borderWidth: 1,
    paddingVertical: 12,
    alignItems: "center",
  },
});
