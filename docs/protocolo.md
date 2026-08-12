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

### `dsp` — estado de la cadena DSP (emitido al iniciar y en cada cambio)

```json
{
  "type": "dsp",
  "preset": "vozLimpia",
  "globalBypass": false,
  "links": [
    { "name": "highpass",       "enabled": true, "bypass": false, "eqBands": null },
    { "name": "boomsuppressor", "enabled": true, "bypass": false, "eqBands": null },
    {
      "name": "eq",
      "enabled": true,
      "bypass": false,
      "eqBands": [
        { "kind": "lowShelf", "freqHz": 200,  "gainDb": -2,   "q": 0.8 },
        { "kind": "peaking",  "freqHz": 3000, "gainDb": 2,    "q": 1.5 },
        { "kind": "highShelf","freqHz": 8000, "gainDb": 1.5,  "q": 0.8 }
      ]
    },
    { "name": "deesser",        "enabled": true, "bypass": false, "eqBands": null },
    { "name": "compressor",     "enabled": true, "bypass": false, "eqBands": null },
    { "name": "limiter",        "enabled": true, "bypass": false, "eqBands": null }
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

La UI los envía por Tauri `invoke` (no por WebSocket en esta fase).

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

### `stop`

```json
{ "type": "stop" }
```

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
| `apply_preset` | `{ preset: PresetId }` | `DspState` |
| `set_global_bypass` | `{ bypass: boolean }` | `DspState` |
| `set_link_bypass` | `{ name: string, bypass: boolean }` | `DspState` |
| `set_eq_band` | `{ bandIndex: number, gainDb: number }` | `DspState` |
| `get_analysis` | — | `AnalysisSample \| null` |
| `get_session_summary` | — | `SessionSummary \| null` |
| `apply_suggestion` | `{ suggestionId: number }` | — |
| `get_pairing_info` | — | `{ code, port, lanAddress }` |

- `bufferSize` en `start_engine` es opcional (`null` → heurística automática).
- Los nombres de módulo de la cadena incluyen: `gain`, `highpass`, `notch`,
  `boomsuppressor`, `eq`, `compressor`, `deesser`, `saturator`, `delay`,
  `reverb`, `limiter`.

- `apply_preset`/`set_global_bypass`/`set_link_bypass` devuelven el nuevo
  `DspState` y además emiten el evento `dsp`.
- `set_eq_band` ajusta en vivo la ganancia de la banda `bandIndex` (0-based) del
  ecualizador del preset activo; emite un evento `dsp` con las bandas nuevas.
  Devuelve error si el preset no tiene EQ o el índice está fuera de rango.
- Estos cuatro comandos exigen el motor **en marcha**; si está detenido devuelven
  error (`EngineNotRunning`). `get_dsp_state` no exige motor en marcha.
- `get_analysis` devuelve la última muestra de análisis (incluida la de la
  última sesión) y `get_session_summary` el resumen acumulado de la sesión
  actual; ambos devuelven `null` si el motor nunca arrancó.
- `apply_suggestion` busca la sugerencia por `id` en la última muestra y, si su
  acción es aplicar un preset, reconfigura la cadena en vivo (equivale a
  `apply_preset` con confirmación).

Los argumentos en JS usan camelCase (Tauri v2 los convierte desde snake_case).

## WebSocket (móvil → escritorio)

- URL: `ws://<ip>:4356/?token=<código>`.
- Autenticación: el código de emparejamiento va en el query string. Sin token
  válido, el servidor responde `401` y cierra la conexión.
- Tráfico: solo **eventos** (server → client). El cliente no envía mensajes.
- Seguridad: cifrado local `ws://` (no `wss://`); no exponer el WebSocket fuera
  de la red local y rotar el código al detectar intentos fallidos repetidos.
