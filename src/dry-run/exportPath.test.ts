import { describe, expect, it } from "vitest";

import { DRY_RUN_EXPORT_PATH_REQUIRED } from "./copy.ts";
import { destinationIsCsv, exportDestinationError, suggestedDryRunPath } from "./exportPath.ts";

describe("dry-run export path", () => {
  it("requires a destination and treats csv as the item metadata format", () => {
    expect(exportDestinationError("")).toBe(DRY_RUN_EXPORT_PATH_REQUIRED);
    expect(exportDestinationError("  ")).toBe(DRY_RUN_EXPORT_PATH_REQUIRED);
    expect(exportDestinationError("C:/tmp/gdom-dry-run.txt")).toBeNull();
    expect(destinationIsCsv("report.CSV")).toBe(true);
    expect(destinationIsCsv("report.txt")).toBe(false);
    expect(suggestedDryRunPath("job 99")).toBe("gdom-dry-run-job99.txt");
  });
});
