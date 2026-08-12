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
- [ ] Ajuste fino de latencia por tamaño de buffer y heurística por dispositivo.
- [ ] Antifeedback básico: high-pass, *notch* y supresión de *boominess*.

Entregable: la cabina permite aplicar presets y la latencia se mantiene
por debajo del umbral perceptible objetivo del plan.

> Pendientes de la Fase 1 (marcados arriba) y nota: el `high-pass` y la
> protección de *clipping* ya están en la cadena; el ajuste fino de buffer y
> el antifeedback completo quedan para fases posteriores junto con el slider IA.

## Fase 2 — IA: asistente vocal

Objetivo: la IA analiza la voz y sugiere ajustes sin fricción.

- [ ] Motor de análisis en `voxlfa-core` (offline o local, sin nube).
- [ ] Sugerencias en tiempo real (timbre, dinámica, fatiga, resonancia).
- [ ] Generación de presets a partir de las sugerencias (con confirmación).
- [ ] Resúmenes de sesión exportables.
- [ ] Control del motor desde el móvil (con autenticación mutua de dispositivos).

## Fase 3+ — Pulido y distribución

- [ ] Persistencia de configuración y perfiles por dispositivo.
- [ ] Paquetes de instalación (deb/AppImage/msi, APK).
- [ ] Autodetección de escritorios en la red para el móvil (mDNS).
- [ ] Telemetría opcional y anónima (con consentimiento y CSP acotada).
