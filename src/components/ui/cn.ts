import { clsx, type ClassValue } from "clsx";

/** Small class-composition helper; keeps variants readable. */
export function cn(...values: ClassValue[]): string {
  return clsx(values);
}