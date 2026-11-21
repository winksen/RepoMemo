/** A cairn: three stones, the largest at the bottom, stacked to mark the way. Drawn like the other icons (24 px grid, 2 px round strokes). */
export function CairnIcon({ size = 16, className }: { size?: number | string; className?: string }) {
  return (
    <svg aria-hidden="true" className={className} fill="none" height={size} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" viewBox="0 0 24 24" width={size} xmlns="http://www.w3.org/2000/svg">
      <ellipse cx="12" cy="18.5" rx="8.5" ry="3.2" />
      <ellipse cx="12" cy="12" rx="5.8" ry="2.9" />
      <ellipse cx="12.4" cy="6.2" rx="3.2" ry="2.2" />
    </svg>
  );
}
