import { describe, expect, it } from "vitest";
import { formatDate, formatFileSize, getFileIconKind } from "./format.ts";

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

  it("classifies local icons from MIME type, extension, and shortcut state", () => {
    expect(getFileIconKind("Folder", "application/vnd.google-apps.folder", true)).toBe("folder");
    expect(getFileIconKind("Report.pdf", "application/pdf", false)).toBe("pdf");
    expect(getFileIconKind("archive.TAR.GZ", "application/octet-stream", false)).toBe("archive");
    expect(getFileIconKind("guide.EPUB", "application/octet-stream", false)).toBe("ebook");
    expect(getFileIconKind("guide", "application/epub+zip", false)).toBe("ebook");
    expect(getFileIconKind("data.pdf", "text/csv", false)).toBe("spreadsheet");
    expect(getFileIconKind("README", "text/plain", false)).toBe("document");
    expect(getFileIconKind("config", "application/json", false)).toBe("code");
    expect(getFileIconKind("archive.pdf", "application/gzip", false)).toBe("archive");
    expect(getFileIconKind("notes.md", "text/plain", false)).toBe("markdown");
    expect(getFileIconKind("notes", "text/markdown", false)).toBe("markdown");
    expect(getFileIconKind("Document", "application/vnd.google-apps.document", false)).toBe("google-doc");
    expect(getFileIconKind("presentation.pptx", "application/octet-stream", false)).toBe("presentation");
    expect(getFileIconKind("Shortcut", "application/vnd.google-apps.shortcut", false)).toBe("shortcut");
    expect(getFileIconKind("unknown.blob", "application/octet-stream", false)).toBe("file");
  });
});
