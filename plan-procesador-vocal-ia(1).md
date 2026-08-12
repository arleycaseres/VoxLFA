# Plan de Trabajo — Procesador Vocal en Vivo con IA

## 1. Visión del producto

Una herramienta multiplataforma (desktop + móvil) que toma el audio de un micrófono en vivo y lo limpia/mejora en tiempo real: elimina feedback/Larsen, reduce ruido de fondo, mejora la claridad de la voz y corrige el tono — todo con latencia lo suficientemente baja para uso en vivo (conciertos, iglesias, karaoke, streaming).

**Diferenciador:** mientras que herramientas como TC-Helicon VoiceLive dependen de hardware dedicado, esta es 100% software, corre local (sin depender de internet) y usa modelos de IA livianos en vez de solo DSP clásico.

---

## 2. Arquitectura multiplataforma

### Núcleo compartido (lo más importante del proyecto)
- **Motor DSP + IA en Rust**, compilado como librería compartida (`cdylib`)
- Esta librería es el "cerebro" y se reutiliza en desktop y móvil, evitando reescribir la lógica de audio dos veces
- Modelos de IA exportados a **ONNX Runtime** (tiene bindings de Rust y corre en CPU/NPU en ambas plataformas)

### Desktop (Windows, macOS, Linux)
- **Tauri v2** + React/TypeScript (tu stack actual) para la UI
- Captura/salida de audio con `cpal` (Rust) — acceso directo a drivers ASIO (Windows), CoreAudio (macOS), ALSA/JACK (Linux)
- Ventaja: en desktop tienes control total de la latencia y acceso a interfaces de audio profesionales (tarjetas externas)

### Móvil (iOS/Android) — App de monitoreo/control remoto
**Decisión tomada:** el móvil NO procesa audio en tiempo real. Es una app de **monitoreo y control remoto** del motor que corre en el desktop — el sonidista/usuario ve medidores, espectro y sugerencias de IA desde el celular, y puede ajustar parámetros a distancia sin estar frente a la laptop.

- Construida en **React Native/Expo** (tu stack actual), sin necesidad de procesar audio nativo en el teléfono
- Comunicación desktop ↔ móvil vía red local: WebSocket o similar (baja carga, no es audio crudo, solo eventos de estado: niveles, espectro, sugerencias de IA, valores de parámetros)
- Esto simplifica muchísimo el desarrollo comparado con procesar audio en el celular, y resuelve de forma elegante la limitación de que los celulares no tienen entradas de micrófono profesionales
- Casos de uso reales: el sonidista camina por la sala escuchando la mezcla mientras ajusta desde el celular; el cantante ve su propio espectro/sugerencias sin tocar la laptop del operador
- (Queda abierta como posibilidad futura, no para el roadmap actual, una versión ligera que sí procese audio localmente en el celular para streaming/práctica — pero no es el foco ahora)

---

## 3. Roadmap por fases

### Fase 0 — Fundación (2-3 semanas)
- Repo monorepo: `/core` (Rust DSP+IA), `/desktop` (Tauri), `/mobile` (Expo/React Native, app de monitoreo remoto)
- Pipeline de audio básico funcionando: captura → passthrough → salida, midiendo latencia real
- Definir el formato de comunicación entre el core Rust y la UI (eventos de nivel de audio, estado de la cadena, etc.), incluyendo el canal WebSocket que luego usará el móvil

### Fase 1 — Motor DSP clásico (3-4 semanas)
- Noise Gate configurable (threshold, attack, release)
- Compresor (ratio, threshold, attack/release, makeup gain)
- EQ paramétrico (mínimo 3 bandas)
- Medidor de nivel en tiempo real en la UI
- **Entregable:** ya es una herramienta usable, aunque sin IA todavía

### Fase 2 — IA: supresión de ruido y feedback (4-6 semanas)
- Integrar RNNoise (o equivalente) vía ONNX para ruido de fondo
- Detección de feedback: combinar análisis espectral (FFT) con un modelo entrenado en patrones de acople
- A/B testing: comparar antes/después con métricas objetivas (SNR) y pruebas con usuarios reales

### Fase 3 — IA: mejora de claridad vocal (5-7 semanas)
- Modelo de speech/vocal enhancement, cuantizado (INT8) para tiempo real
- Cuidado: entrenar o afinar el modelo específicamente en voz **cantada**, no hablada — son espectros muy distintos
- Modo "estudio" opcional (post-procesamiento no en vivo) con modelo más pesado para quien no necesita tiempo real

### Fase 4 — IA: corrección de tono en tiempo real (6-8 semanas)
- Detección de pitch de baja latencia (YIN o similar)
- Resíntesis (PSOLA clásico, o red neuronal ligera)
- Esta es la fase técnicamente más difícil — considera dejarla para después de tener usuarios reales probando las fases anteriores

### Fase 5 — Visualizador de espectro en tiempo real (2-3 semanas, puede ir en paralelo con Fase 2)
- Cálculo de FFT en el core Rust (ventana Hann, con solapamiento) sobre el buffer de audio
- Envío de bins de magnitud a la UI vía eventos
- Render en Canvas/WebGL: espectro en barras o curva, escala logarítmica en frecuencia y dB en amplitud
- Opcional: espectrograma tipo "waterfall" (frecuencia vs. tiempo), útil también para ver venir el feedback antes de que sea audible

### Fase 6 — IA consejera de configuración (4-5 semanas, depende de tener Fases 2-3 avanzadas)
- Extracción periódica de métricas resumen del audio (energía por banda, sibilancia, SNR, clipping, comportamiento del gate) desde el core Rust
- Capa de interpretación con LLM (Groq/Llama, mismo patrón que usas en KAIZEN Protect): recibe las métricas y devuelve recomendaciones en lenguaje humano ("baja 2-3dB cerca de 3.2kHz", "sube el attack del gate")
- UI: tarjeta de "Sugerencias de IA" con opción de aplicar el ajuste automáticamente o solo ver el detalle

### Fase 7 — App móvil de monitoreo/control remoto (3-4 semanas)
- App Expo/React Native que se conecta al desktop por red local (WebSocket)
- Vistas: medidores de nivel, espectro en vivo, sugerencias de IA, control remoto de parámetros/presets
- No procesa audio — solo visualiza y controla el motor que corre en el desktop

### Fase 8 — Interoperabilidad con otras herramientas (2-3 semanas, opcional/no bloqueante)
- Meta: que el usuario pueda usar el software junto con ecualizadores físicos u otras herramientas externas sin conflictos, no que dependa de ellas
- Soporte de **enrutamiento de audio flexible**: dejar que el usuario elija qué dispositivo de entrada/salida usar (para poder intercalar un EQ físico externo entre el micrófono y la entrada del software, o entre la salida del software y el amplificador)
- Compatibilidad con **ASIO/CoreAudio/ALSA multi-dispositivo**, para no competir por el control exclusivo de la tarjeta de audio
- Evaluar soporte de **plugins VST/AU** más adelante, para que el software pueda cargarse dentro de otro host de audio (o cargar plugins de terceros dentro de él) — esto se puede dejar como mejora futura, no es parte del MVP

### Fase 9 — Pulido de interfaz y UX multiplataforma (paralelo, continuo)
- Ver sección 4

### Fase 10 — Beta cerrada y ajustes con usuarios reales
- Sonidistas, cantantes de iglesia, streamers — grupos de prueba distintos, porque cada uno usa la herramienta diferente

---

## 4. Interfaz clara y fácil de usar en cualquier dispositivo

Principios de diseño para que sea intuitiva tanto en pantalla grande (laptop) como pequeña (celular):

- **Vista simple por defecto, avanzada opcional:** al abrir, el usuario ve 3-4 controles grandes (Nivel de ruido, Feedback, Claridad, Tono) con un slider simple cada uno. Un botón "Modo avanzado" despliega los parámetros técnicos (attack/release, Hz exactos, etc.) para quien sabe de audio
- **Presets por caso de uso:** "Voz para karaoke", "Predicador/Iglesia", "Streaming", "Concierto en vivo" — así el usuario no técnico no tiene que entender DSP
- **Medidor visual en tiempo real** siempre visible (entrada vs. salida) para que el usuario vea que la herramienta está trabajando
- **Responsive real, no solo "que quepa":** en celular, los controles avanzados se acomodan en pestañas o acordeón; en desktop pueden verse todos a la vez en una sola pantalla
- **Indicador de latencia visible:** en vivo, la gente necesita saber si hay retraso audible — muéstralo en milisegundos
- **Modo oscuro** (estándar esperado en apps de audio, se usan en escenarios con poca luz)
- **Accesibilidad:** tamaños de texto/touch targets adecuados para uso en escenario (a veces con luces bajas y prisa)

---

## 5. Cosas que probablemente se te pasaron (y que debemos definir)

- **Latencia objetivo:** ¿cuál es el máximo aceptable? En vivo, arriba de ~15-20ms ya se empieza a notar/molestar. Esto condiciona qué tan pesados pueden ser los modelos de IA
- **Descubrimiento en red local:** para que la app móvil de monitoreo encuentre el desktop automáticamente en la misma red (mDNS/Bonjour o similar), sin que el usuario tenga que escribir IPs manualmente
- **Seguridad de la conexión desktop-móvil:** aunque sea red local, conviene un emparejamiento simple (código/QR) para que no cualquier dispositivo en la misma WiFi pueda controlar el software en un evento en vivo
- **Monetización:** ¿es de pago único, suscripción, freemium con presets básicos gratis y avanzados de pago? Esto afecta si necesitas backend de licencias/pagos (ya tienes experiencia con PayPal/ePayco de KAIZEN Protect, podrías reusar esa capa)
- **Perfiles de usuario/nube:** ¿los presets personalizados se guardan solo local o se sincronizan entre dispositivos? Si es lo segundo, necesitas backend (podrías reusar Supabase, que ya conoces)
- **Certificación de audio en producción:** para uso profesional en vivo, algunos ingenieros de sonido van a preguntar por compatibilidad ASIO/soporte de interfaces específicas — vale la pena probar con 2-3 interfaces populares (Focusrite, Behringer) desde temprano
- **Pruebas de estrés reales:** un ensayo de banda o concierto real es muy distinto a pruebas en tu escritorio — el feedback real depende de la sala, parlantes, distancia del micrófono
- **Legal/patentes:** la corrección de tono en tiempo real (estilo Auto-Tune) tuvo patentes históricas de Antares; conviene revisar que tu implementación (algoritmo propio) no choque con patentes vigentes en tu mercado
- **Nombre y marca:** vale la pena definirlo pronto para reservar dominio/redes, igual que hiciste con KAIZEN Protect

---

## 6. Próximo paso concreto

Si te parece bien este plan, el siguiente paso práctico es arrancar la **Fase 0**: crear el repo, configurar Tauri v2 con el proyecto Rust del core, y lograr que el audio pase de la entrada a la salida sin procesar (passthrough) midiendo la latencia real de tu setup. Esto valida que la arquitectura elegida cumple el objetivo de latencia antes de invertir tiempo en DSP e IA.

**Nota sobre el enrutamiento de audio (interoperabilidad):** al construir el motor de captura/salida en la Fase 0, es buen momento para diseñarlo desde el inicio permitiendo que el usuario elija libremente sus dispositivos de entrada y salida — así queda naturalmente preparado desde el día uno para trabajar junto con ecualizadores físicos u otro hardware externo, sin tener que rediseñar esa parte después.
