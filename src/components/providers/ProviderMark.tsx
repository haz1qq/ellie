import type { CSSProperties } from "react";
import openaiMark from "../../assets/providers/openai.svg";
import deepseekMark from "../../assets/providers/deepseek.svg";

const PROVIDER_MARKS: Record<string, string> = {
  "openai-codex": openaiMark,
  "openai-api": openaiMark,
  deepseek: deepseekMark,
};

export interface ProviderMarkProps {
  providerId: string;
  /** Provider display name used for the initial-letter fallback. */
  displayName: string;
}

/**
 * Monochrome provider identity mark. Bundled brand SVGs are theme-adapted with
 * a CSS mask (never recolored); providers without a bundled mark fall back to
 * the existing initial-letter tile glyph. No remote assets are loaded.
 */
export function ProviderMark({ providerId, displayName }: ProviderMarkProps) {
  const mark = PROVIDER_MARKS[providerId];
  if (!mark) {
    return <span aria-hidden="true">{displayName.slice(0, 1)}</span>;
  }
  return (
    <span
      className="provider-mark"
      aria-hidden="true"
      style={{ "--provider-mark": `url("${mark}")` } as CSSProperties}
    />
  );
}