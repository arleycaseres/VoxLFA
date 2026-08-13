// Escáner del QR de emparejamiento que muestra la cabina del escritorio.
//
// Al detectar un QR con el formato `ws://<host>:<puerto>/?token=<código>`
// parsea el destino, invoca `onScanned` y se cierra. Pide permiso de cámara
// si hace falta (funciona dentro de Expo Go).

import { useEffect, useState } from "react";
import { StyleSheet, Text, TouchableOpacity, View } from "react-native";
import { CameraView, useCameraPermissions } from "expo-camera";
import { parsePairingUrl, type PairingTarget } from "../lib/pairingUrl";

interface QrScannerProps {
  onScanned: (target: PairingTarget) => void;
  onCancel: () => void;
}

export function QrScanner({ onScanned, onCancel }: QrScannerProps) {
  const [permission, requestPermission] = useCameraPermissions();
  const [handled, setHandled] = useState(false);

  useEffect(() => {
    if (permission && !permission.granted && permission.canAskAgain) {
      void requestPermission();
    }
  }, [permission, requestPermission]);

  const handleScan = (result: { data: string; type: string }) => {
    if (handled || result.type !== "qr") return;
    const target = parsePairingUrl(result.data);
    if (!target) return;
    setHandled(true);
    onScanned(target);
  };

  if (!permission) {
    return (
      <View style={styles.card}>
        <Text style={styles.hint}>Solicitando permiso de cámara…</Text>
      </View>
    );
  }

  if (!permission.granted) {
    return (
      <View style={styles.card}>
        <Text style={styles.hint}>
          Se necesita la cámara para leer el QR de la cabina.
        </Text>
        {permission.canAskAgain && (
          <TouchableOpacity style={styles.button} onPress={requestPermission} activeOpacity={0.8}>
            <Text style={styles.buttonText}>Conceder permiso</Text>
          </TouchableOpacity>
        )}
        <TouchableOpacity style={styles.buttonGhost} onPress={onCancel} activeOpacity={0.8}>
          <Text style={styles.buttonText}>Cancelar</Text>
        </TouchableOpacity>
      </View>
    );
  }

  return (
    <View style={styles.card}>
      <CameraView
        style={styles.camera}
        facing="back"
        onBarcodeScanned={handleScan}
        barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
      />
      <Text style={styles.hint}>Apunta al QR de la cabina (código de emparejamiento)</Text>
      <TouchableOpacity style={styles.buttonGhost} onPress={onCancel} activeOpacity={0.8}>
        <Text style={styles.buttonText}>Cancelar</Text>
      </TouchableOpacity>
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
    gap: 12,
    alignItems: "center",
  },
  camera: {
    width: "100%",
    aspectRatio: 1,
    borderRadius: 8,
    overflow: "hidden",
  },
  hint: {
    color: "#8a929c",
    fontSize: 12,
    textAlign: "center",
  },
  button: {
    backgroundColor: "rgba(79, 216, 255, 0.12)",
    borderColor: "#4fd8ff",
    borderRadius: 8,
    borderWidth: 1,
    paddingVertical: 12,
    paddingHorizontal: 24,
    alignItems: "center",
    alignSelf: "stretch",
  },
  buttonGhost: {
    borderColor: "#2a2e34",
    borderRadius: 8,
    borderWidth: 1,
    paddingVertical: 12,
    paddingHorizontal: 24,
    alignItems: "center",
    alignSelf: "stretch",
  },
  buttonText: {
    color: "#e6e9ec",
    fontSize: 13,
    fontWeight: "600",
    letterSpacing: 1,
    textTransform: "uppercase",
  },
});
