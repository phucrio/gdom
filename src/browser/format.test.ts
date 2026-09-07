import { describe, expect, it } from "vitest";
import { formatDate, formatFileSize, getFileIcon } from "./format.ts";

describe("format utilities", () => {
  it("formats file size with readable units and handles null/undefined/0", () => {
    expect(formatFileSize(null)).toBe("—");
    expect(formatFileSize(undefined)).toBe("—");
    expect(formatFileSize(0)).toBe("0 B");
    expect(formatFileSize(512)).toBe("512 B");
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(1024 * 1024 * 5)).toBe("5.0 MB");
    expect(formatFileSize(1024 * 1024 * 1024 * 2.5)).toBe("2.5 GB");
  });

  it("formats ISO dates with human readable summary and full detail", () => {
    expect(formatDate(null).display).toBe("—");
    expect(formatDate(undefined).display).toBe("—");
    expect(formatDate("invalid-date").display).toBe("—");

    const formatted = formatDate("2026-09-01T10:30:00Z");
    expect(formatted.display).not.toBe("—");
    expect(formatted.full).not.toBe("—");
  });

  it("returns distinctive icons based on mime type", () => {
    expect(getFileIcon("application/vnd.google-apps.folder", true)).toBe("📁");
    expect(getFileIcon("application/pdf", false)).toBe("📄");
    expect(getFileIcon("image/png", false)).toBe("🖼️");
    expect(getFileIcon("video/mp4", false)).toBe("🎥");
    expect(getFileIcon("audio/mpeg", false)).toBe("🎵");
  });
});
