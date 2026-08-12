# Roadmap de VoxLFA

Fases derivadas del plan del producto (`plan-procesador-vocal-ia(1).md`).
Cada fase termina con la verificación completa (fmt, clippy, tests, builds).

## Fase 0 — Base: passthrough seguro con latencia medida ✅

Objetivo: el esqueleto del producto funcionando, con cero procesamiento.

- [x] Monorepo (`core` Rust, `desktop` Tauri v2 + React, `mobile` Expo).
- [x] Motor de audio en `voxlfa-core`: captura → ring buffer lock-free →
      salida, con medición de latencia en tiempo real.
- [x] Protocolo serde (`EngineEvent`, `ControlCommand`) con espejo TS.
- [x] Cabina de escritorio: dial de instrumento, medidores RMS/Pico, selector de
      dispositivos, control arrancar/detener, latencia y muestreo en vivo.
- [x] Emparejamiento: código de 6 caracteres sin ambiguos.
- [x] WebSocket local autenticado por token (rechazo 401) para el móvil.
- [x] App móvil Expo: monitor de estado, latencia y niveles con reconexión.
- [x] Documentación: arquitectura, protocolo, seguridad, este roadmap.

> Nota de compilación: en entornos Flatpak sin `webkit2gtk-4.1`, el backend de
> escritorio se comprueba con `cargo check -p voxlfa-desktop
> --no-default-features` (sin el feature `webview`).

## Fase 1 — DSP: procesamiento de voz en tiempo real ✅

Objetivo: transformar la señal vocal con efectos de calidad baja en latencia.

- [x] Pipeline de procesadores encadenables (`AudioProcessor`) con bypass.
- [x] Módulos DSP: compresor, EQ paramétrico, de-esser, saturación/armónicos.
- [x] Reverb y delay con buffers de cola (pre-delay, feedback, mezcla).
- [x] Medidores por canal (pre/post) y protección de *clipping* (`limiter`).
- [x] Presets aplicables desde la cabina (Voz Limpia, Radio, Warm, …).
- [x] Ajuste fino de latencia por tamaño de buffer y heurística por dispositivo.
- [x] Antifeedback básico: high-pass, *notch* y supresión de *boominess*.

Entregable: la cabina permite aplicar presets, elegir el buffer (o dejarlo en
"Auto" con heurística por dispositivo) y la cadena incluye antifeedback
(pasa-altos, muesca y supresión de *boominess*), manteniendo la latencia por
debajo del umbral perceptible objetivo del plan.

> La Fase 1 queda completa. Pendiente de fases posteriores: el asistente de IA
> (entregado en la Fase 2).

## Fase 1.1 — Ajuste fino del ecualizador ✅

Objetivo: ajustar el EQ por banda, en vivo y sin reiniciar el motor.

- [x] El estado de la cadena (`DspLinkState`) expone las bandas actuales del
      ecualizador (`eqBands`) en los tres lados (Rust, TS desktop, TS móvil).
- [x] Comando `set_eq_band` (desktop): reconstruye solo el módulo EQ en el hilo
      de control con la ganancia nueva y la conmuta en el callback de audio
      (`DspCommand::SetLinkProcessor`), sin asignar memoria en el camino real.
- [x] Cabina: panel "Ecualizador" con un slider por banda (rango −18…+18 dB,
      paso 0.1 dB) y botón de restablecimiento a 0 dB; se deshabilita con el
      motor parado o el EQ en bypass.
- [x] El móvil refleja las bandas del EQ en modo solo lectura.
- [x] Verificación completa (fmt, clippy, 83 tests, builds desktop y móvil).

> Los ajustes finos son **por sesión**: se pierden al cambiar de preset o
> reiniciar el motor (la persistencia está planificada en la Fase 3).

## Fase 2 — IA: asistente vocal ✅ (bloques 1-4)

Objetivo: la IA analiza la voz y sugiere ajustes sin fricción.

- [x] Motor de análisis en `voxlfa-core` (100 % local, sin nube ni FFT):
      divisor de bandas con biquads en el callback (O(n), sin asignación),
      ventana deslizante de métricas, seguimiento de sesión y reglas
      heurísticas de sugerencias en un hilo dedicado.
- [x] Sugerencias en tiempo real (timbre, dinámica, fatiga, resonancia) emitidas
      como `EngineEvent::Analysis` y mostradas en la cabina.
- [x] Aplicación de sugerencias con confirmación (`apply_suggestion`: aplica el
      preset sugerido en vivo) y resumen de sesión exportable a JSON.
- [x] El móvil muestra el análisis y las sugerencias en modo solo lectura.
- [ ] Control del motor desde el móvil (con autenticación mutua de dispositivos).

Entregable: la cabina muestra métricas de voz en vivo (brillo, resonancia,
fatiga, dinámica), sugiere ajustes con botón "Aplicar" que reconfigura la cadena
en vivo, y permite exportar el resumen de la sesión a JSON. El móvil refleja el
análisis en modo monitoreo.

> Pendiente de la Fase 2: el bloque 5 (control del motor desde el móvil con
> autenticación mutua) se dejó fuera de esta tanda por decisión de producto.

## Fase 3+ — Pulido y distribución

- [ ] Persistencia de configuración y perfiles por dispositivo.
- [ ] Paquetes de instalación (deb/AppImage/msi, APK).
- [ ] Autodetección de escritorios en la red para el móvil (mDNS).
- [ ] Telemetría opcional y anónima (con consentimiento y CSP acotada).
