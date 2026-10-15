# Design system

Warm monochrome, numbers first. Tokens live in `src/index.css`; components use the Tailwind names, never raw colours.

- **Type**: Geist for UI, Geist Mono for addresses, ports, figures and Results (tabular numbers, no ligatures). Headings are semibold with tight tracking; body line-height 1.6.
- **Colour**: one warm gray family. Colour only carries meaning, always as muted text on a washed-out tint:
  - `success` / `success-soft` — healthy (Low increase under load)
  - `warning` / `warning-soft` — caution (Moderate, metered connection, star ratings)
  - `danger` / `danger-soft` — failure (High, No internet, Rate limited, Interrupted)
  - `info` / `info-soft` — neutral notes
- **Charts**: `chart-1` is the measured quantity (speed), `chart-2` the warning line (latency under load), `muted-foreground` dashed for a reference value.
- **Surfaces**: 1px hairline `border`, 6px radius (`--radius: 0.375rem`), no shadows, no gradients.
- **Motion**: buttons scale to 0.98 when pressed; colour transitions 150 ms. Nothing else moves.
- **Logo**: `public/logo.svg` (favicon) and `src/components/logo.tsx` (theme-aware): a 3x3 grid of tiles, one per Tool, the top-right tile in the danger tone as the single accent.
