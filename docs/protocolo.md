# Protocolo de VoxLFA

Contrato de datos entre el motor (`core`), la cabina de escritorio y el móvil.

## Reglas

- El contrato vive en **un solo lugar**: `core/src/protocol/` (Rust, serde).
- Espejos TypeScript: `desktop/src/lib/types.ts` y `mobile/src/lib/protocol.ts`.
- **Nunca** cambies un nombre de campo sin actualizar los tres lados.
- Enumerados con `tag = "type"` y `rename_all = "camelCase"` (y
  `rename_all_fields = "camelCase"` en las variantes struct). Los eventos
  estructurales se **aplanan**: el discriminante y los campos viajan en el
  mismo objeto JSON.

## Eventos (motor → UI / móvil)

Emitidos por el motor como evento Tauri `engine-event` en el escritorio y como
mensajes JSON por el WebSocket para el móvil.

### `status` — cambio de estado

```json
{
  "type": "status",
  "state": "running",
  "sampleRate": 48000,
  "bufferSize": 512,
  "latencyMs": 8.2,
  "inputDevice": "Micrófono (USB Audio)",
  "outputDevice": "Altavoces (USB Audio)"
}
```

- `state`: `"stopped" | "starting" | "running" | "stopping" | "error"`.
- `latencyMs`: latencia medida captura→salida.

### `level` — muestra de niveles en tiempo real (máx. cada 50 ms)

```json
{
  "type": "level",
  "inputRmsDb": -18.3,
  "inputPeakDb": -11.2,
  "outputRmsDb": -6.4,
  "outputPeakDb": -2.1,
  "latencyMs": 8.2,
  "capturedAtMs": 1723400000123
}
```

- Niveles en **dBFS** (silencio ≈ `-120`).
- `inputRmsDb`/`inputPeakDb`: señal **antes** de la cadena DSP.
- `outputRmsDb`/`outputPeakDb`: señal **después** de la cadena DSP.
- `capturedAtMs` es tiempo monotónico.

### `spectrum` — espectro de la entrada en vivo (FFT, máx. cada 50 ms)

```json
{
  "type": "spectrum",
  "binsDb": [-62.4, -58.1, -50.2, -42.7, -31.0, -24.5, -19.2, -14.8, ...],
  "sampleRate": 48000,
  "capturedAtMs": 1723400000123
}
```

- `binsDb`: nivel pico de cada banda logarítmica en **dBFS** (suavizado
  ataque/release). Longitud fija **32** (constante `SPECTRUM_BIN_COUNT` en los
  tres lados: `core/src/protocol/event.rs`, `desktop/src/lib/types.ts`,
  `mobile/src/lib/protocol.ts`).
- La FFT (ventana Hann, 2048 puntos, 50 % de solapamiento) se calcula sobre la
  **entrada** (señal pre-DSP) y se reduce a 32 bandas entre ~20 Hz y el Nyquist
  de `sampleRate` (a 48 kHz, ~23 Hz de resolución por bin).
- `sampleRate` define los bordes de banda: el consumidor reconstruye la escala
  logarítmica como `edge_i = 20 · (min(nyquist, 20 kHz) / 20)^(i/32)`.
- Las barras con nivel ≥ -6 dBFS se dibujan en naranja (zona caliente).

### `dsp` — estado de la cadena DSP (emitido al iniciar y en cada cambio)

```json
{
  "type": "dsp",
  "preset": "vozLimpia",
  "globalBypass": false,
  "links": [
    { "name": "highpass",  "enabled": true, "bypass": false, "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null },
    {
      "name": "noisegate",
      "enabled": true,
      "bypass": false,
      "eqBands": null,
      "gateParams": {
        "thresholdDb": -50,
        "attackMs": 2,
        "releaseMs": 100,
        "holdMs": 120,
        "rangeDb": 40
      },
      "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null
    },
    { "name": "boomsuppressor", "enabled": true, "bypass": false, "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null },
    {
      "name": "eq",
      "enabled": true,
      "bypass": false,
      "eqBands": [
        { "kind": "lowShelf", "freqHz": 200,  "gainDb": -2,   "q": 0.8 },
        { "kind": "peaking",  "freqHz": 3000, "gainDb": 2,    "q": 1.5 },
        { "kind": "highShelf","freqHz": 8000, "gainDb": 1.5,  "q": 0.8 }
      ],
      "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null
    },
    { "name": "deesser",    "enabled": true, "bypass": false, "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null },
    { "name": "compressor", "enabled": true, "bypass": false, "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null },
    {
      "name": "delay",
      "enabled": true,
      "bypass": false,
      "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null,
      "delayParams": {
        "mode": "slapback",
        "timeMs": 65,
        "feedback": 0,
        "mix": 0.08,
        "preDelayMs": 0,
        "lowCutHz": 200,
        "highCutHz": 8000,
        "tempoBpm": 120,
        "syncEnabled": false,
        "duckAmount": 0.3
      },
      "reverbParams": null
    },
    {
      "name": "reverb",
      "enabled": true,
      "bypass": false,
      "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null,
      "reverbParams": {
        "mode": "plate",
        "roomSize": 0.5,
        "damping": 0.5,
        "wet": 0.08,
        "preDelayMs": 15,
        "highCutHz": 12000,
        "lowCutHz": 100
      }
    },
    { "name": "limiter",    "enabled": true, "bypass": false, "eqBands": null, "gateParams": null, "denoiseParams": null, "feedbackParams": null, "pitchCorrectionParams": null, "delayParams": null, "reverbParams": null }
  ]
}
```

- `preset`: `"dry" | "vozLimpia" | "radio" | "warm"`.
- `links` mantiene el **orden de la cadena**: primero el elemento más cercano a
  la entrada.
- `enabled`: el módulo existe en el preset; `bypass`: está puenteado
  individualmente. Si `globalBypass` es `true`, la entrada va directa a la
  salida sin pasar por ningún módulo.
- `eqBands`: solo lo lleva el módulo `eq`; es la configuración **actual** del
  ecualizador (bandas, frecuencias, ganancias y Q), que cambia con el comando
  `set_eq_band`. Los demás módulos lo envían a `null`.
- `gateParams`: solo lo lleva el módulo `noisegate`; es la configuración
  **actual** de la puerta de ruido (umbral, ataque, liberación, *hold* y rango),
  que cambia con el comando `set_noise_gate`. Los demás módulos lo envían a
  `null`. El móvil lo muestra en solo lectura (no lo ajusta).
- `denoiseParams`: solo lo lleva el módulo `denoise` (mezcla wet/dry).
- `feedbackParams`: solo lo lleva el módulo `feedbacksuppress` (umbral y Q).
- `pitchCorrectionParams`: solo lo lleva el módulo `pitchcorrection` (strength,
  mix, scale, root).
- `delayParams`: solo lo lleva el módulo `delay`; incluye `mode` (digital/analog/
  tape/slapback), `timeMs`, `feedback`, `mix`, `preDelayMs`, `lowCutHz`,
  `highCutHz`, `tempoBpm`, `syncEnabled`, `duckAmount`. Se ajusta con
  `set_delay`.
- `reverbParams`: solo lo lleva el módulo `reverb`; incluye `mode` (plate/hall/
  room), `roomSize`, `damping`, `wet`, `preDelayMs`, `highCutHz`, `lowCutHz`.
  Se ajusta con `set_reverb`.
- `latencyMs` (ver evento `status`) ya **incluye** la latencia propia de la
  cadena (p. ej. el limiter suma su lookahead).

### `analysis` — muestra de análisis vocal (máx. cada ~500 ms)

```json
{
  "type": "analysis",
  "metrics": {
    "rmsDb": -18.3,
    "peakDb": -11.2,
    "dynamicRangeDb": 12.4,
    "crestDb": 7.1,
    "brightness": 0.46,
    "resonanceScore": 0.38,
    "fatigueScore": 0.22,
    "windowMs": 2000
  },
  "suggestions": [
    {
      "id": 0,
      "kind": "resonance",
      "severity": 0.64,
      "message": "Se acumula energía en la zona baja-media (boominess). …",
      "action": { "type": "applyPreset", "preset": "vozLimpia" }
    }
  ],
  "capturedAtMs": 1723400001234
}
```

- `metrics` se calcula sobre una **ventana deslizante** de `windowMs` ms usando
  bandas de biquads (sin FFT): `brightness`, `resonanceScore` y `fatigueScore`
  son índices 0–1; los niveles van en dBFS.
- `suggestions`: reglas heurísticas disparadas (la UI muestra el mensaje en
  español y ofrece "Aplicar" cuando `action.type` es `applyPreset`). El `id` es
  estable por regla y lo usa el comando `apply_suggestion`.
- `kind`: `"timbre" | "dynamics" | "fatigue" | "resonance"`.
- `action`: `{ "type": "none" }` o `{ "type": "applyPreset", "preset": … }`.

El resumen de la sesión no viaja por evento: se consulta con el comando
`get_session_summary`.

### `devices` — listado de dispositivos

```json
{
  "type": "devices",
  "inputs":  [{ "name": "Micrófono (USB Audio)", "isDefault": true }],
  "outputs": [{ "name": "Altavoces (USB Audio)",  "isDefault": true }]
}
```

### `warning` — aviso no fatal (el motor sigue corriendo)

```json
{ "type": "warning", "message": "output underrun detected" }
```

> Los mensajes internos van en inglés (estándar de logs); la UI traduce si es
> necesario.

## Comandos (UI → motor)

La UI de escritorio los envía por Tauri `invoke`; la app móvil envía los mismos
comandos (excepto `start`) por el WebSocket como mensajes JSON.

### `start`

```json
{
  "type": "start",
  "inputDevice": null,
  "outputDevice": null,
  "bufferSize": null
}
```

- Dispositivos `null` = predeterminado del sistema.
- `bufferSize` (muestras/callback) es **opcional**: `null` deja que el core lo
  elija con una heurística por dispositivo (USB → 128, Bluetooth/HDMI → 1024,
  resto → 256).
- **Solo Tauri**: arrancar el motor exige el callback de eventos de la ventana;
  el WebSocket **rechaza** este comando con un `warning`.

### `stop`

```json
{ "type": "stop" }
```

### `setPreset`, `setGlobalBypass`, `setLinkBypass`, `setEqBand`

Mismos comandos que los Tauri del escritorio (ver tabla siguiente), con los
campos en camelCase:

```json
{ "type": "setPreset",       "preset": "warm" }
{ "type": "setGlobalBypass", "bypass": true }
{ "type": "setLinkBypass",   "link": "eq", "bypass": true }
{ "type": "setEqBand",       "bandIndex": 2, "gainDb": -4.5 }
```

- El móvil los envía por el WebSocket; el servidor los ejecuta contra el mismo
  gestor del motor que la cabina y el resultado llega como evento `dsp`.
- La ganancia del EQ se **acota** en el servidor a `[-18, 18]` dB (rango de la
  cabina) y `bandIndex` debe existir en el preset activo (si no, error).
- Si un comando falla (motor detenido, JSON inválido, `start` no permitido,
  mensaje > 1 KB), el servidor responde al móvil con un evento `warning`.

## Comandos Tauri del escritorio

| Comando | Argumentos | Resultado |
| --- | --- | --- |
| `list_devices` | — | `{ inputs, outputs }` |
| `start_engine` | `{ inputDevice, outputDevice, bufferSize }` | — |
| `stop_engine` | — | — |
| `get_engine_status` | — | `EngineStatus \| null` |
| `get_last_level` | — | `LevelSample \| null` |
| `get_presets` | — | `PresetInfo[]` |
| `get_dsp_state` | — | `DspState \| null` |
| `get_config` | — | `AppConfig` |
| `apply_preset` | `{ preset: PresetId }` | `DspState` |
| `set_global_bypass` | `{ bypass: boolean }` | `DspState` |
| `set_link_bypass` | `{ name: string, bypass: boolean }` | `DspState` |
| `set_eq_band` | `{ bandIndex: number, gainDb: number }` | `DspState` |
| `set_noise_gate` | `{ params: NoiseGateParams }` | `DspState` |
| `set_delay` | `{ params: DelayParams }` | `DspState` |
| `set_reverb` | `{ params: ReverbParams }` | `DspState` |
| `get_analysis` | — | `AnalysisSample \| null` |
| `get_session_summary` | — | `SessionSummary \| null` |
| `apply_suggestion` | `{ suggestionId: number }` | — |
| `get_pairing_info` | — | `{ code, port, lanAddress }` |

- `bufferSize` en `start_engine` es opcional (`null` → heurística automática).
- Los nombres de módulo de la cadena incluyen: `gain`, `highpass`, `noisegate`,
  `notch`, `boomsuppressor`, `eq`, `compressor`, `deesser`, `saturator`,
  `delay`, `reverb`, `limiter`.

## Configuración persistida (solo escritorio)

`AppConfig` (comando `get_config`) es el esquema del `config.json` del usuario
(`$XDG_CONFIG_HOME/voxlfa/config.json`). **No** es parte del contrato con el
móvil: solo lo consume la cabina para precargar los selectores.

```json
{
  "defaultInput": "Interfaz Scarlett 2i2",
  "defaultOutput": "Monitor 01",
  "bufferSize": 128,
  "profiles": [
    {
      "deviceKey": "Interfaz Scarlett 2i2",
      "preset": "warm",
      "eqBands": [
        { "kind": "lowShelf", "freqHz": 120, "gainDb": 3, "q": 0.8 }
      ],
      "gateParams": {
        "thresholdDb": -48,
        "attackMs": 3,
        "releaseMs": 120,
        "holdMs": 120,
        "rangeDb": 40
      },
      "delayParams": {
        "mode": "digital",
        "timeMs": 80,
        "feedback": 0.25,
        "mix": 0.1,
        "preDelayMs": 0,
        "lowCutHz": 200,
        "highCutHz": 12000,
        "tempoBpm": 120,
        "syncEnabled": false,
        "duckAmount": 0.4
      },
      "reverbParams": {
        "mode": "plate",
        "roomSize": 0.5,
        "damping": 0.5,
        "wet": 0.12,
        "preDelayMs": 20,
        "highCutHz": 12000,
        "lowCutHz": 100
      },
      "globalBypass": false,
      "linkBypass": { "reverb": true }
    }
  ]
}
```

- `profiles` guarda un perfil por **dispositivo de entrada** (`deviceKey` es el
  nombre del dispositivo, o `"default"` si se usó el predeterminado del
  sistema).
- Al arrancar el motor con un dispositivo con perfil, se aplican su preset,
  sus `eqBands`, sus `gateParams`, sus `delayParams`, sus `reverbParams` y sus
  bypasses.
  `apply_preset`/`set_*_bypass` persisten al instante; el ajuste fino del EQ
  (`set_eq_band`), de la puerta de ruido (`set_noise_gate`), del delay
  (`set_delay`) y del reverb (`set_reverb`) se vuelcan al detener el motor.
- Archivo tolerante a fallos: si falta o está corrupto se parte de la
  configuración vacía.

- `apply_preset`/`set_global_bypass`/`set_link_bypass` devuelven el nuevo
  `DspState` y además emiten el evento `dsp`.
- `set_eq_band` ajusta en vivo la ganancia de la banda `bandIndex` (0-based) del
  ecualizador del preset activo; emite un evento `dsp` con las bandas nuevas.
  Devuelve error si el preset no tiene EQ o el índice está fuera de rango.
- `set_noise_gate` ajusta en vivo los parámetros de la puerta de ruido del
  preset activo (solo Tauri: el móvil es de solo lectura); emite un evento `dsp`
  con los `gateParams` nuevos. Devuelve error si el preset no tiene puerta de
  ruido. Los valores se acotan en el motor a las ventanas de la cabina.
- Estos cuatro comandos exigen el motor **en marcha**; si está detenido devuelven
  error (`EngineNotRunning`). `get_dsp_state` no exige motor en marcha.
- `get_analysis` devuelve la última muestra de análisis (incluida la de la
  última sesión) y `get_session_summary` el resumen acumulado de la sesión
  actual; ambos devuelven `null` si el motor nunca arrancó.
- `apply_suggestion` busca la sugerencia por `id` en la última muestra y, si su
  acción es aplicar un preset, reconfigura la cadena en vivo (equivale a
  `apply_preset` con confirmación).

Los argumentos en JS usan camelCase (Tauri v2 los convierte desde snake_case).

## WebSocket (móvil ↔ escritorio)

- URL: `ws://<ip>:4356/?token=<código>`.
- Autenticación: el código de emparejamiento va en el query string. Sin token
  válido, el servidor responde `401` y cierra la conexión. Tras
  `MAX_FAILED_ATTEMPTS` (3) handshakes fallidos consecutivos el código **rota**
  automáticamente y la cabina recibe el nuevo por el evento `pairing-event`
  (la app móvil debe pedir el código actualizado o volver a escanear el QR).
- Descubrimiento: el escritorio se anuncia por **mDNS** como
  `_voxlfa._tcp.local.` (TXT solo con metadatos: `name`, `ver`; nunca el
  token). La app móvil conecta rellenando IP/puerto/código a mano o
  **escanenado el QR** que muestra la cabina, que codifica exactamente esta
  URL (`ws://<ip>:<puerto>/?token=<código>`).
- Tráfico:
  - **Server → client**: eventos del motor (`status`, `level`, `dsp`, …).
  - **Client → server**: comandos `stop`, `setPreset`, `setGlobalBypass`,
    `setLinkBypass`, `setEqBand`, `setNoiseGate`, `setDelay` y `setReverb`
    (JSON con `tag = "type"`). `start` se rechaza.
- Comandos malformados o fallidos se responden con un evento `warning` dirigido
  al cliente que los envió.
- Límites de entrada (seguridad): mensajes ≤ 1 KB; ganancia del EQ acotada a
  `[-18, 18]` dB; el resto de campos los valida el motor (índices, nombres de
  módulo, preset).
- Seguridad: el token equivale a **mando remoto** (puede detener el motor y
  reconfigurar la cadena). Cifrado local `ws://` (no `wss://`); no exponer el
  WebSocket fuera de la red local y rotar el código al detectar intentos
  fallidos repetidos.
