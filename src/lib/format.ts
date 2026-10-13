/** A round-trip or connect time as whole milliseconds, with anything under 1 ms shown as "<1 ms". */
export function milliseconds(value: number | null) {
  if (value === null) return "-";
  return value < 1 ? "<1 ms" : `${Math.floor(value)} ms`;
}
