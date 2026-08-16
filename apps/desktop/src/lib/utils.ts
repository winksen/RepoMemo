import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** Standard shadcn utility; retained even though RepoMemo styles are CSS-owned. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
