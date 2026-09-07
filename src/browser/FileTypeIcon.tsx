import type { ReactNode } from "react";

export type FileIconKind = "archive" | "audio" | "code" | "document" | "ebook" | "file" | "folder" | "google-doc" | "google-sheet" | "google-slide" | "image" | "markdown" | "pdf" | "presentation" | "shortcut" | "spreadsheet" | "video";

type FileTypeIconProps = { kind: FileIconKind; title?: string };

function iconGlyph(kind: FileIconKind): ReactNode {
  switch (kind) {
    case "folder": return <path d="M3 6.5h6l2 2h10v10.5H3z" />;
    case "shortcut": return <><path d="M4 3h11l5 5v13H4z" /><path d="M10 14h7m0 0-3-3m3 3-3 3" /></>;
    case "pdf": return <><path d="M5 2h9l5 5v15H5z" /><path d="M14 2v6h5M8 16h8M8 19h6" /></>;
    case "archive": return <><path d="M4 5h16v15H4z" /><path d="M8 3h8v4H8zM12 9v7m0 2v1" /></>;
    case "image": return <><rect x="3" y="4" width="18" height="16" rx="1" /><circle cx="8" cy="9" r="1.5" /><path d="m4 18 5-5 3 3 3-4 5 6" /></>;
    case "audio": return <path d="M14 4v11.5a3 3 0 1 1-2-2.82V6l7-2v9.5a3 3 0 1 1-2-2.82V2z" />;
    case "video": return <><rect x="3" y="5" width="13" height="14" rx="1" /><path d="m16 10 5-3v10l-5-3z" /></>;
    case "spreadsheet": return <><path d="M5 2h9l5 5v15H5z" /><path d="M9 11h6M9 15h6M9 19h6M9 8v12M13 8v12" /></>;
    case "presentation": return <><path d="M4 3h16v12H4z" /><path d="M9 21h6m-3-6v6M8 8h8" /></>;
    case "markdown": return <><path d="M4 3h16v18H4z" /><path d="M7 16V9l3 4 3-4v7m3-5 2 2 2-2v5" /></>;
    case "ebook": return <path d="M4 4h7a3 3 0 0 1 3 3v12a3 3 0 0 0-3-3H4zM20 4h-7a3 3 0 0 0-3 3v12a3 3 0 0 1 3-3h7z" />;
    case "code": return <path d="m9 18-6-6 6-6m6 0 6 6-6 6M14 4l-4 16" />;
    case "google-doc": return <><path d="M5 2h9l5 5v15H5z" /><path d="M14 2v6h5M8 12h8M8 15h8M8 18h6" /></>;
    case "google-sheet": return <><path d="M5 2h9l5 5v15H5z" /><path d="M14 2v6h5M8 11h7M8 15h7M8 19h7M11.5 9v11" /></>;
    case "google-slide": return <><path d="M5 2h9l5 5v15H5z" /><rect x="8" y="11" width="8" height="5" rx=".5" /></>;
    case "document": return <><path d="M5 2h9l5 5v15H5z" /><path d="M14 2v6h5M8 12h8M8 16h8M8 20h5" /></>;
    case "file": return <path d="M5 2h9l5 5v15H5zM14 2v6h5" />;
  }
}

export function FileTypeIcon({ kind, title }: FileTypeIconProps) {
  return <svg className={`file-type-icon file-type-icon-${kind}`} viewBox="0 0 24 24" role={title ? "img" : undefined} aria-hidden={title ? undefined : true} aria-label={title}>{iconGlyph(kind)}</svg>;
}
