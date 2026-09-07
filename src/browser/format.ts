export function formatFileSize(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) {
    return "—";
  }
  if (bytes === 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  if (i === 0) {
    return `${bytes} B`;
  }
  const value = bytes / Math.pow(k, i);
  return `${value.toFixed(1)} ${units[i] ?? ""}`;
}

export function formatDate(isoString: string | null | undefined): { display: string; full: string } {
  if (!isoString) {
    return { display: "—", full: "—" };
  }
  try {
    const d = new Date(isoString);
    if (isNaN(d.getTime())) {
      return { display: "—", full: isoString };
    }
    const full = d.toLocaleString(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    });
    const now = new Date();
    const isToday =
      d.getDate() === now.getDate() &&
      d.getMonth() === now.getMonth() &&
      d.getFullYear() === now.getFullYear();

    const display = isToday
      ? d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
      : d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });

    return { display, full };
  } catch {
    return { display: "—", full: isoString };
  }
}

import type { FileIconKind } from "./FileTypeIcon.tsx";

const EXTENSIONS: Record<string, FileIconKind> = {
  pdf: "pdf", zip: "archive", rar: "archive", "7z": "archive", tar: "archive", gz: "archive", bz2: "archive", xz: "archive", epub: "ebook", mobi: "ebook", doc: "document", docx: "document", odt: "document", txt: "document", md: "markdown", markdown: "markdown", xls: "spreadsheet", xlsx: "spreadsheet", ods: "spreadsheet", csv: "spreadsheet", ppt: "presentation", pptx: "presentation", odp: "presentation", png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", svg: "image", heic: "image", mp3: "audio", wav: "audio", flac: "audio", m4a: "audio", mp4: "video", webm: "video", mov: "video", mkv: "video", rs: "code", ts: "code", tsx: "code", js: "code", jsx: "code", py: "code", go: "code", java: "code", json: "code", yaml: "code", yml: "code", toml: "code", css: "code", html: "code",
};

export function getFileIconKind(name: string, mimeType: string, isFolder: boolean, isShortcut = false): FileIconKind {
  if (isShortcut || mimeType === "application/vnd.google-apps.shortcut") return "shortcut";
  if (isFolder || mimeType === "application/vnd.google-apps.folder") return "folder";
  if (mimeType === "application/vnd.google-apps.document") return "google-doc";
  if (mimeType === "application/vnd.google-apps.spreadsheet") return "google-sheet";
  if (mimeType === "application/vnd.google-apps.presentation") return "google-slide";
  if (mimeType === "application/epub+zip" || mimeType.includes("mobipocket")) return "ebook";
  if (mimeType === "text/markdown") return "markdown";
  if (mimeType.includes("pdf")) return "pdf";
  if (mimeType.startsWith("image/")) return "image";
  if (mimeType.startsWith("video/")) return "video";
  if (mimeType.startsWith("audio/")) return "audio";
  if (mimeType.includes("spreadsheet") || mimeType.includes("excel")) return "spreadsheet";
  if (mimeType.includes("presentation") || mimeType.includes("powerpoint")) return "presentation";
  if (mimeType.includes("document") || mimeType.includes("word")) return "document";
  if (mimeType.includes("zip") || mimeType.includes("compressed") || mimeType.includes("rar")) return "archive";
  const parts = name.toLowerCase().split(".");
  const extension = parts[parts.length - 1] ?? "";
  return EXTENSIONS[extension] ?? "file";
}
