// Selector de dispositivo de audio con opción "Predeterminado".
//
// Presenta los dispositivos detectados; si el backend informa el predeterminado
// lo marca para que el usuario lo identifique de un vistazo.

import type { AudioDeviceInfo } from "../lib/types";
import { shortDeviceName } from "../lib/format";

interface DeviceSelectorProps {
  label: string;
  devices: AudioDeviceInfo[];
  value: string | null;
  onChange: (name: string | null) => void;
  disabled?: boolean;
}

export function DeviceSelector({
  label,
  devices,
  value,
  onChange,
  disabled,
}: DeviceSelectorProps) {
  return (
    <label className="select">
      <span className="select__label">{label}</span>
      <select
        value={value ?? ""}
        disabled={disabled}
        onChange={(event) => {
          const name = event.target.value;
          onChange(name === "" ? null : name);
        }}
      >
        <option value="">Predeterminado del sistema</option>
        {devices.map((device) => (
          <option key={device.name} value={device.name}>
            {shortDeviceName(device.name)}
            {device.isDefault ? " (sistema)" : ""}
          </option>
        ))}
      </select>
    </label>
  );
}
