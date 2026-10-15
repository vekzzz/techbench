/**
 * The Techbench mark: a grid of small tiles, one per Tool, the top-right one in the single
 * accent tone. Draws in the current theme's colours.
 */
export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden="true">
      <rect width="32" height="32" rx="7" className="fill-foreground" />
      <rect x="6" y="6" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="13.5" y="6" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="21" y="6" width="5" height="5" rx="1.2" className="fill-danger" />
      <rect x="6" y="13.5" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="13.5" y="13.5" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="21" y="13.5" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="6" y="21" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="13.5" y="21" width="5" height="5" rx="1.2" className="fill-background" />
      <rect x="21" y="21" width="5" height="5" rx="1.2" className="fill-background" />
    </svg>
  );
}
