import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ProviderMark } from "./ProviderMark";

describe("ProviderMark", () => {
  it("renders a bundled monochrome mark for OpenAI and DeepSeek", () => {
    for (const [providerId, displayName] of [
      ["openai-codex", "OpenAI / Codex"],
      ["openai-api", "OpenAI API"],
      ["deepseek", "DeepSeek"],
    ] as const) {
      const { container } = render(
        <ProviderMark providerId={providerId} displayName={displayName} />,
      );
      const mark = container.querySelector(".provider-mark");
      expect(mark).not.toBeNull();
      expect(mark?.getAttribute("aria-hidden")).toBe("true");
      const mask = mark?.getAttribute("style") ?? "";
      expect(mask).toMatch(/--provider-mark: url\("data:image\/svg|\.svg"\)/);
      expect(mark?.textContent).toBe("");
    }
  });

  it("falls back to the initial letter for providers without a bundled mark", () => {
    const { container } = render(
      <ProviderMark providerId="anthropic-claude" displayName="Anthropic / Claude" />,
    );
    const symbol = container.querySelector("span");
    expect(symbol).not.toBeNull();
    expect(symbol?.textContent).toBe("A");
    expect(container.querySelector(".provider-mark")).toBeNull();
  });
});