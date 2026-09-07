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

export function getFileIcon(mimeType: string, isFolder: boolean): string {
  if (isFolder) {
    return "📁";
  }
  if (mimeType.includes("pdf")) return "📄";
  if (mimeType.includes("image")) return "🖼️";
  if (mimeType.includes("video")) return "🎥";
  if (mimeType.includes("audio")) return "🎵";
  if (mimeType.includes("spreadsheet") || mimeType.includes("excel")) return "📊";
  if (mimeType.includes("presentation") || mimeType.includes("powerpoint")) return "📽️";
  if (mimeType.includes("document") || mimeType.includes("word")) return "📝";
  if (mimeType.includes("zip") || mimeType.includes("compressed")) return "📦";
  return "📄";
}
