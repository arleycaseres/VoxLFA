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
    { "name": "Pasa altos",  "enabled": true, "bypass": false },
    { "name": "Ecualizador", "enabled": true, "bypass": false },
    { "name": "De-esser",    "enabled": true, "bypass": false },
    { "name": "Compresor",   "enabled": true, "bypass": false },
    { "name": "Limiter",     "enabled": true, "bypass": false }
  ]
}
```

- `preset`: `"dry" | "vozLimpia" | "radio" | "warm"`.
- `links` mantiene el **orden de la cadena**: primero el elemento más cercano a
  la entrada.
- `enabled`: el módulo existe en el preset; `bypass`: está puenteado
  individualmente. Si `globalBypass` es `true`, la entrada va directa a la
  salida sin pasar por ningún módulo.
- `latencyMs` (ver evento `status`) ya **incluye** la latencia propia de la
  cadena (p. ej. el limiter suma su lookahead).

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
{ "type": "start", "inputDevice": null, "outputDevice": null }
```

Dispositivos `null` = predeterminado del sistema.

### `stop`

```json
{ "type": "stop" }
```

## Comandos Tauri del escritorio

| Comando | Argumentos | Resultado |
| --- | --- | --- |
| `list_devices` | — | `{ inputs, outputs }` |
| `start_engine` | `{ inputDevice, outputDevice }` | — |
| `stop_engine` | — | — |
| `get_engine_status` | — | `EngineStatus \| null` |
| `get_last_level` | — | `LevelSample \| null` |
| `get_presets` | — | `PresetInfo[]` |
| `get_dsp_state` | — | `DspState \| null` |
| `apply_preset` | `{ preset: PresetId }` | `DspState` |
| `set_global_bypass` | `{ bypass: boolean }` | `DspState` |
| `set_link_bypass` | `{ name: string, bypass: boolean }` | `DspState` |
| `get_pairing_info` | — | `{ code, port, lanAddress }` |

- `apply_preset`/`set_global_bypass`/`set_link_bypass` devuelven el nuevo
  `DspState` y además emiten el evento `dsp`.
- Estos tres comandos exigen el motor **en marcha**; si está detenido devuelven
  error (`EngineNotRunning`). `get_dsp_state` no exige motor en marcha.

Los argumentos en JS usan camelCase (Tauri v2 los convierte desde snake_case).

## WebSocket (móvil → escritorio)

- URL: `ws://<ip>:4356/?token=<código>`.
- Autenticación: el código de emparejamiento va en el query string. Sin token
  válido, el servidor responde `401` y cierra la conexión.
- Tráfico: solo **eventos** (server → client). El cliente no envía mensajes.
- Seguridad: cifrado local `ws://` (no `wss://`); no exponer el WebSocket fuera
  de la red local y rotar el código al detectar intentos fallidos repetidos.
