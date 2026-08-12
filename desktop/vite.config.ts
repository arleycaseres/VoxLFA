import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuración de Vite para la UI de VoxLFA.
// El puerto 1420 es la convención de Tauri (devUrl en tauri.conf.json).
export default defineConfig({
  plugins: [react()],
  // En desarrollo Tauri sirve los assets desde este servidor; el frontend
  // no necesita `strictPort` porque Tauri usa este puerto explícitamente.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Ignorar los cambios de Rust para que Vite no recargue innecesario.
      ignored: ["**/src-tauri/**"],
    },
  },
});
