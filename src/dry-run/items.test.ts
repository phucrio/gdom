import { describe, expect, it } from "vitest";

import { GOOGLE_FOLDER_MIME, GOOGLE_SHORTCUT_MIME, itemKindLabel, itemStateLabel } from "./items.ts";

describe("dry-run item labels", () => {
  it("labels folders shortcuts and skip states without using color", () => {
    expect(itemKindLabel(GOOGLE_FOLDER_MIME)).toBe("Folder");
    expect(itemKindLabel(GOOGLE_SHORTCUT_MIME)).toBe("Shortcut");
    expect(itemKindLabel("text/plain")).toBe("File");
    expect(itemStateLabel("ELIGIBLE")).toBe("Eligible");
    expect(itemStateLabel("SKIPPED_ALREADY_OWNED_BY_TARGET")).toBe("Already target-owned");
    expect(itemStateLabel("SKIPPED_SHARED_DRIVE")).toBe("Shared Drive");
    expect(itemStateLabel("SKIPPED_TRASHED")).toBe("Trashed or missing");
  });
});
