// Selector de tamaño de buffer (latencia vs estabilidad).
//
// A menor buffer, menor latencia pero mayor riesgo de *underruns*; el valor
// "Auto" delega la elección al motor (heurística según el tipo de dispositivo).

interface BufferSelectorProps {
  /** Tamaño seleccionado, o `null` para "Auto". */
  value: number | null;
  onChange: (bufferSize: number | null) => void;
  disabled?: boolean;
}

const BUFFER_OPTIONS = [64, 128, 256, 512, 1024];

export function BufferSelector({ value, onChange, disabled }: BufferSelectorProps) {
  return (
    <label className="select">
      <span className="select__label">Buffer (latencia)</span>
      <select
        value={value === null ? "auto" : String(value)}
        disabled={disabled}
        onChange={(event) => {
          const raw = event.target.value;
          onChange(raw === "auto" ? null : Number(raw));
        }}
      >
        <option value="auto">Auto (heurística)</option>
        {BUFFER_OPTIONS.map((size) => (
          <option key={size} value={String(size)}>
            {size} smp
            {size <= 128 ? " · baja latencia" : size >= 1024 ? " · estable" : ""}
          </option>
        ))}
      </select>
    </label>
  );
}
