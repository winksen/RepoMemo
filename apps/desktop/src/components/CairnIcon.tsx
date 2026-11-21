import { Mountains, type IconWeight } from "@phosphor-icons/react";

/** Cairn's icon: mountains, the country a cairn marks the way across. A Phosphor icon, so it takes a weight (`fill` where it is the heading). */
export function CairnIcon({ size = 16, weight = "regular", className }: { size?: number | string; weight?: IconWeight; className?: string }) {
  return <Mountains aria-hidden="true" className={className} size={size} weight={weight} />;
}
