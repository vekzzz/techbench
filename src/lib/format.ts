/** A round-trip or connect time as whole milliseconds, with anything under 1 ms shown as "<1 ms". */
export function milliseconds(value: number | null) {
  if (value === null) return "-";
  return value < 1 ? "<1 ms" : `${Math.floor(value)} ms`;
}

/** A byte count as "250.0 MB", or "1.25 GB" from a gigabyte up. */
export function dataSize(bytes: number) {
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(2)} GB` : `${(bytes / 1e6).toFixed(1)} MB`;
}
