import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  contrastRatio,
  declarationValue,
  parseRootColorTokens,
  parseRuleBlock,
  resolveCssColor,
} from "./contrast.ts";

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../App.css"), "utf8");

describe("contrastRatio", () => {
  it("measures an under-contrast field border below 3:1 and a passing pair at or above 3:1", () => {
    expect(contrastRatio("#3a4254", "#0a0c11")).toBeLessThan(3);
    expect(contrastRatio("#8a93a8", "#0a0c11")).toBeGreaterThanOrEqual(3);
  });
});

describe("shipped field boundary contrast", () => {
  it("keeps input and select borders at least 3:1 against fill, page, and raised panel", () => {
    const tokens = parseRootColorTokens(css);
    const fieldRule = parseRuleBlock(
      css,
      `input,
select`,
    );
    const border = resolveCssColor(declarationValue(fieldRule, "border"), tokens);
    const fill = resolveCssColor(declarationValue(fieldRule, "background"), tokens);
    const page = tokens["ink"];
    const panel = tokens["ink-raised"];

    if (page === undefined || panel === undefined) {
      throw new Error("missing --ink or --ink-raised");
    }

    expect(contrastRatio(border, fill)).toBeGreaterThanOrEqual(3);
    expect(contrastRatio(border, page)).toBeGreaterThanOrEqual(3);
    expect(contrastRatio(border, panel)).toBeGreaterThanOrEqual(3);
  });

  it("keeps ghost-style control borders at least 3:1 against page and raised panel", () => {
    const tokens = parseRootColorTokens(css);
    const controlRule = parseRuleBlock(
      css,
      `.nav-link,
.ghost-button,
.primary-button,
.danger-button,
.account-actions button,
.migration-controls button,
.step-tab,
.root-list button`,
    );
    const border = resolveCssColor(declarationValue(controlRule, "border"), tokens);
    const page = tokens["ink"];
    const panel = tokens["ink-raised"];

    if (page === undefined || panel === undefined) {
      throw new Error("missing --ink or --ink-raised");
    }

    expect(contrastRatio(border, page)).toBeGreaterThanOrEqual(3);
    expect(contrastRatio(border, panel)).toBeGreaterThanOrEqual(3);
  });
});
