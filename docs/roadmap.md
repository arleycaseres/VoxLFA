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

> Los ajustes finos se aplican **en vivo** y (desde la Fase 3) se guardan por
> dispositivo de entrada para reaplicarlos al reiniciar.

## Fase 1.2 — Puerta de ruido configurable ✅

Objetivo: silenciar el ruido de fondo del micrófono entre frases con una puerta
de ruido ajustable en vivo (ítem "Noise Gate" de la Fase 1 del plan).

- [x] Módulo `NoiseGate` en `voxlfa-core` (`dsp/gate.rs`): umbral, ataque,
      liberación, *hold* y rango; envolvente pico con ataque/liberación por
      muestra, decisión abierta/cerrada con *hold* y ganancia suavizada en dB
      (sin *zipper noise*). Arranca cerrada y no asigna memoria en el callback.
- [x] Insertado en la cadena tras el pasa-altos en los presets de voz
      (vozLimpia, radio, warm); `dry` no lo incluye.
- [x] Protocolo: `NoiseGateParams` en el estado del eslabón (`gateParams`),
      variante de preset `DspModuleKind::NoiseGate` y comando
      `set_noise_gate` (reconstruye la puerta en el hilo de control y la
      conmuta en vivo, sin reiniciar el motor).
- [x] Cabina: panel "Puerta de ruido" con sliders de umbral (−80…−20 dB),
      rango (6…60 dB), ataque (0…20 ms) y liberación (20…500 ms); el *hold* se
      muestra en solo lectura (lo fija el preset).
- [x] El móvil refleja los parámetros en modo solo lectura.
- [x] Persistencia: el perfil por dispositivo guarda `gateParams` y los reaplica
      al arrancar (igual que el EQ fino).
- [x] Verificación completa (fmt, clippy, tests, builds desktop y móvil).

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
- [x] Control del motor desde el móvil (bloque 5): detener el motor, cambiar
      preset, bypass global/por módulo y ajustar el EQ por banda, ejecutados por
      el WebSocket contra el mismo gestor que la cabina. Autenticado con el
      código de emparejamiento (equivale a **mando remoto**); `start` queda
      reservado a la cabina de escritorio.

Entregable: la cabina muestra métricas de voz en vivo (brillo, resonancia,
fatiga, dinámica), sugiere ajustes con botón "Aplicar" que reconfigura la cadena
en vivo, y permite exportar el resumen de la sesión a JSON. El móvil refleja el
análisis y controla el motor de forma remota (stop, preset, bypass y EQ).

> Bloque 5 de la Fase 2 entregado: el móvil pasa de solo lectura a control
> remoto del motor usando el código de emparejamiento como mando (se descartó la
> autenticación mutua por dispositivo por decisión de producto; ver
> `docs/seguridad.md`).

## Fase 2.6 — Seguridad del emparejamiento ✅

Objetivo: mitigar la fuerza bruta del código en la red local.

- [x] `PairingState` en el backend: código vigente + contador de fallos;
      autenticación atómica por handshake (`authenticate`), sin bloquear red.
- [x] Rotación automática tras 3 handshakes fallidos consecutivos (el acierto
      reinicia el contador); el nuevo código se publica por el evento Tauri
      `pairing-event` y la cabina lo refresca al instante.
- [x] El WebSocket comparte el `PairingState` y ejecuta la rotación en el propio
      handshake (401) sin desbloquear la conexión aceptada.
- [x] Tests: 7 unitarios de `PairingState` + 1 de integración de rotación
      (`repeated_failures_rotate_the_pairing_code`).
- [x] Documentación de seguridad actualizada (rotación como mitigación).

## Fase 2.7 — Autodetección del escritorio (mDNS + QR) ✅

Objetivo: que el móvil descubra el escritorio sin teclear la IP.

- [x] El escritorio se anuncia por mDNS como `_voxlfa._tcp.local.` (`mdns-sd`)
      con la IP LAN y el puerto del WebSocket; los TXT solo llevan metadatos no
      sensibles (nunca el token). Test de integración registra y resuelve el
      servicio (`advertise_then_browse_finds_the_service`).
- [x] La cabina muestra un **QR** que codifica `ws://<ip>:<puerto>/?token=<código>`
      (se regenera al rotar el código).
- [x] La app móvil escanea el QR (`expo-camera`, compatible con Expo Go),
      rellena IP/puerto/código y conecta automáticamente.
- [x] Se descarta el cliente mDNS nativo en el móvil: rompe Expo Go y exigiría
      un build con dev tools; el QR resuelve la autodetección sin compilar.
- [x] Documentación actualizada (protocolo, seguridad, arquitectura).

## Fase 2.8 — Visualizador de espectro (FFT) en tiempo real ✅

Objetivo: ver el espectro de la voz capturada en vivo (Fase 5 del plan, que
podía ir en paralelo con la Fase 2).

- [x] `SpectrumAnalyzer` en `voxlfa-core` (`analysis/fft.rs`): FFT de 2048
      puntos con ventana Hann y 50 % de solapamiento (`rustfft`), sin asignar
      memoria en el callback (ventana, anillo, buffers y scratch preasignados).
- [x] La FFT se reduce a **32 bandas logarítmicas** (20 Hz → Nyquist/20 kHz);
      cada banda reporta el pico en dBFS suavizado con ataque rápido (one-pole)
      y release lineal en dB (60 dB/s) para una vista estable.
- [x] Evento `EngineEvent::Spectrum` (máx. cada 50 ms) sobre la entrada
      pre-DSP; `SpectrumSample` con `binsDb`, `sampleRate` y `capturedAtMs`.
- [x] El escritorio lo reenvía a la cabina y al móvil; `EngineManager` guarda el
      último espectro (`get_last_spectrum` para el render inicial).
- [x] Cabina: `SpectrumView` (canvas) con barras en escala logarítmica de
      frecuencia y dB, rejilla y zona caliente (≥ -6 dBFS) en naranja.
- [x] Móvil: barras del espectro en `MonitorView` (mismo contrato y guard de
      tipos `isEngineEvent`).
- [x] Verificación completa (fmt, clippy, 118 tests, builds desktop y móvil).

> El análisis vocal de la Fase 2 sigue coexistiendo: las bandas (biquads) para
> métricas/sugerencias y la FFT para el visualizador. La FFT también servirá de
> base a la detección de feedback de la Fase 2 original (análisis espectral del
> acople).

## Fase 3 — Persistencia y perfiles por dispositivo ✅

Objetivo: que la cabina recuerde la configuración del usuario y la reaplique al
volver a conectar el mismo dispositivo.

- [x] Esquema de configuración en `voxlfa-core` (`config.rs`): `AppConfig` con
      perfiles por dispositivo de entrada (`DeviceProfile`: preset, bandas del
      EQ, bypasses) y selector de dispositivos/buffer recordados.
- [x] Persistencia en `config.json` (`$XDG_CONFIG_HOME/voxlfa/config.json`),
      tolerante a fallos (archivo ausente/corrupto → configuración vacía).
- [x] `DspHandle::set_eq_bands`: reaplica el ajuste fino del EQ de un perfil de
      una vez (sin reconstruir el resto de la cadena).
- [x] `EngineManager` orquesta la persistencia: aplica el perfil al arrancar,
      guarda preset/bypasses al cambiar, vuelca el EQ fino al detener.
- [x] Comando `get_config` y precarga de los selectores de la cabina con la
      última selección (si el dispositivo sigue conectado).
- [x] Verificación completa (fmt, clippy, 97 tests, builds desktop y móvil).

> La clave de perfil es el **nombre del dispositivo de entrada** elegido
> (`"default"` si se usa el predeterminado del sistema). Si el nombre cambia
> (p. ej. se mueve de puerto USB), se parte de cero para ese nombre.

## Fase 3+ — Pulido y distribución

- [x] Paquetes de instalación (deb/AppImage/msi/dmg/APK): workflow de GitHub Actions
      (`.github/workflows/release.yml`) que construye automáticamente para
      Linux (.deb + .AppImage), Windows (.msi + .exe), macOS (.dmg, universal
      binary) y Android (APK vía EAS Build) al hacer `git tag v*`. Artefactos
      publicados como GitHub Release.
- [ ] Navegación mDNS nativa desde el móvil (opcional): el escritorio ya se
      anuncia; solo faltaría un cliente DNS-SD nativo (requiere dev build).
- [x] Telemetría opcional y anónima (con consentimiento y CSP acotada):
      módulo `telemetry` en `voxlfa-core` con eventos tipados (app started,
      session started/ended, module toggled, consent changed) y cola `mpsc`
      thread-safe. El `EngineManager` emite eventos al arrancar/detener el
      motor y al cambiar de preset. Consentimiento opt-in con diálogo en la
      cabina (primera vez) y persistencia en `config.json`. Envío HTTP con
      `reqwest::blocking` en un hilo dedicado; endpoint configurable via
      constante `TELEMETRY_ENDPOINT` (vacío = desactivado, solo debug log).
      Payload JSON por evento, timeout 5 s, errores best-effort a stderr.
- [x] Empaquetado y descarga de modelos ONNX: módulo `models.rs` en
      `voxlfa-core` con `ModelStatus`, `models_dir()` (XDG/Linux, macOS,
      Windows), `download_models()` desde assets de GitHub releases, y
      `download_file()` con `ureq`. Comandos Tauri `get_model_status` y
      `download_models` (con eventos `model-download-progress`). UI:
      `ModelManager.tsx` con estado de archivos, barra de progreso y botón
      de descarga. Denoise auto-detecta modelos y cae a RNNoise o
      passthrough si no están.

## Fase 2 ONNX — Denoise + Feedback ✅

Objetivo: supresión de ruido de alta calidad (ONNX DeepFilterNet3) y
supresión de feedback adaptativa.

- [x] Denoise RNNoise (`denoise.rs`): módulo `RnnoiseDenoise` con 480 muestras
      por frame, escala I16, descarte de fade-in, integrado en la cadena con
      `DenoiseMix` (wet/dry). Feature `rnnoise`.
- [x] Denoise ONNX (`denoise_onnx.rs`): pipeline completo de DeepFilterNet3:
      STFT/ISTFT con ventana Hann y overlap-add, filtro ERB (32 bandas,
      Glasberg & Moore), 3 sesiones ONNX (encoder, ERB decoder, DF decoder),
      normalización exponencial móvil, máscara ERB + coeficientes DF. Feature
      `onnx`. Fallback a passthrough si los modelos no existen.
- [x] Supresión de feedback adaptativa (`feedback.rs`): análisis FFT de 2048
      puntos con ventana Hann, detección de picos por umbral, hasta 4 filtros
      notch adaptativos con ataque/liberación suavizados.
- [x] Protocolo: `DenoiseParams`, `FeedbackSuppressorParams` en `protocol/dsp.rs`,
      variantes `DspModuleKind::Denoise` y `DspModuleKind::FeedbackSuppressor`,
      comandos `set_denoise` y `set_feedback`.
- [x] Persistencia: `denoise_params` y `feedback_params` en `DeviceProfile`.
- [x] UI: paneles `DenoisePanel` (mix) y `FeedbackPanel` (threshold + Q) en
      la cabina, labels/iconos en `DspChain`.
- [x] Verificación completa (fmt, clippy, 95 tests, builds desktop y móvil).

> **Empaquetado de modelos**: ver Fase 3+ (descarga desde GitHub releases,
> UI de gestión en `ModelManager.tsx`).

## Fase 4 — IA: corrección de tono en tiempo real ✅

Objetivo: detectar y corregir el pitch de la voz en tiempo real con latencia
mínima.

- [x] Detección de pitch con algoritmo **YIN** (`dsp/pitch_correction.rs`):
      buffer circular de 2048 muestras a 48 kHz, CDF normalizado, umbral 0.15,
      búsqueda de mínimo local en ventana de 16 tau. No asigna memoria en el
      callback.
- [x] Escalas musicales: `MusicalScale` (cromática, mayor, menor natural,
      menor armónica, pentatónica mayor/menor, blues) y `MusicalNote` (C–B con
      sostenidos). Selección de la nota más cercana en la escala con wrap-around.
- [x] Resíntesis **PSOLA** simplificada: detección de épocas por picos de
      energía, ventanas Hann pitch-sincronizadas, reubicación con el nuevo
      espaciamiento y overlap-add. Paso directo si la señal es silenciosa.
- [x] Procesador `PitchCorrection` implementando `AudioProcessor` con parámetros
      `strength` (0–1), `mix` (seco/húmedo), `scale` y `root`. No asigna
      memoria en el callback (buffers preasignados).
- [x] Protocolo: `PitchCorrectionParams`, `MusicalScale`, `MusicalNote` en
      `protocol/dsp.rs`, variante `DspModuleKind::PitchCorrection`, campo
      `pitch_correction_params` en `DspLinkState`.
- [x] Cadena DSP: `set_pitch_correction` en `DspHandle` y `DspCommand`,
      mapeo en `build_processor`, `module_name`, `pitch_correction_params_of`.
- [x] Persistencia: `pitch_correction_params` en `DeviceProfile` (`config.rs`),
      reaplicado al arrancar, guardado al detener.
- [x] Cabina: comando Tauri `set_pitch_correction` registrado.
- [x] Desktop TS: tipos `PitchCorrectionParams`, `MusicalScale`, `MusicalNote`
      en `types.ts`, función `setPitchCorrection` en `tauri.ts` y `mock.ts`.
- [x] Móvil: tipos espejo en `protocol.ts` (`DspLinkState` con
      `pitchCorrectionParams`, `PitchCorrectionParams`, `MusicalScale`,
      `MusicalNote`).
- [x] Tests: 13 unitarios (detector, escalas, shift ratio, PSOLA passthrough,
      creación, passthrough a strength/mix 0, intervalos, notas).
- [x] Verificación completa (fmt, clippy, 120 tests, builds desktop y móvil).

> **Nota de producto**: el plan original advierte que la corrección de tono es la
> fase técnicamente más difícil y sugiere dejarla para después de tener usuarios
> reales. Se implementa aquí porque la arquitectura estaba lista y la detección
> YIN + PSOLA es un camino probado. El `strength` bajo (0.3–0.5) da un efecto
> sutil tipo "pitch assistance"; valores altos (0.8–1.0) dan el efecto
> Auto-Tune marcado.

## Fase 8 — Interoperabilidad con otras herramientas ✅ (parcial)

Objetivo: que el usuario pueda usar el software junto con ecualizadores físicos
u otras herramientas externas sin conflictos.

- [x] **Enrutamiento de audio flexible**: `AudioEngineConfig` soporta
      `input_device`, `output_device` y `audio_host` independientes; el usuario
      puede intercalar un EQ físico externo entre el micrófono y la entrada del
      software o entre la salida del software y el amplificador.
- [x] **Host de audio configurable**: enumeración de hosts disponibles (ALSA,
      JACK, PipeWire, etc.) con `AudioEngineInfo`, comandos Tauri
      `list_audio_hosts` / `list_devices_for_host`, selector de host en la
      configuración persistida (`default_host`).
- [x] **JACK habilitado**: feature `jack` en cpal para routing profesional
      multi-dispositivo en Linux (conexión directa entre apps de audio).
- [x] **Protocolo actualizado**: `EngineStatus` incluye `audio_host`, tipos TS
      desktop/móvil sincronizados (`AudioHostInfo`, `HostList`,
      `AppConfig.defaultHost`).
- [x] **Persistencia**: el host seleccionado se guarda en `config.json` y se
      reaplica al arrancar.
- [x] **Móvil**: `MonitorView` muestra el host de audio activo.
- [x] Verificación completa (fmt, clippy, 163 tests, builds desktop y móvil).

> **VST/AU**: el soporte de plugins VST/AU queda como mejora futura (requiere
> crates dedicados como `nih_plug` o `vst3-sys` y un UI de gestión de plugins;
> es un feature significativamente más complejo que el enrutamiento de audio).
>
> **Loopback macOS/Windows**: para routing de audio entre apps, el usuario
> puede usar BlackHole (macOS) o VB-CABLE (Windows) como dispositivos
> virtuales; VoxLFA los detecta y lista normalmente.
