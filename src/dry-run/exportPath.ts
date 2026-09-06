import { DRY_RUN_EXPORT_PATH_REQUIRED, defaultDryRunFileName } from "./copy.ts";

export function destinationIsCsv(path: string): boolean {
  return path.trim().toLowerCase().endsWith(".csv");
}

export function exportDestinationError(path: string): string | null {
  if (path.trim().length === 0) {
    return DRY_RUN_EXPORT_PATH_REQUIRED;
  }
  return null;
}

export function suggestedDryRunPath(jobId: string): string {
  return defaultDryRunFileName(jobId, "txt");
}
