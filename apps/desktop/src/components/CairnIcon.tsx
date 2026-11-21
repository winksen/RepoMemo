import { RocketLaunch, type IconWeight } from "@phosphor-icons/react";

/** Cairn's icon: a rocket taking off. A Phosphor icon, so it takes a weight (`fill` where it is the heading). */
export function CairnIcon({ size = 16, weight = "regular", className }: { size?: number | string; weight?: IconWeight; className?: string }) {
  return <RocketLaunch aria-hidden="true" className={className} size={size} weight={weight} />;
}
