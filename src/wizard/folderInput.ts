export type FolderParseSuccess = {
  ok: true;
  folderId: string;
  source: "url" | "id";
};

export type FolderParseFailure = {
  ok: false;
  reason: "empty" | "invalid";
};

export type FolderParseResult = FolderParseSuccess | FolderParseFailure;

const FOLDER_ID_PATTERN = /^[A-Za-z0-9_-]{10,}$/;

export function parseFolderInput(raw: string): FolderParseResult {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "empty" };
  }

  const fromUrl = extractFolderIdFromUrl(trimmed);
  if (fromUrl !== null) {
    if (!FOLDER_ID_PATTERN.test(fromUrl)) {
      return { ok: false, reason: "invalid" };
    }
    return { ok: true, folderId: fromUrl, source: "url" };
  }

  if (looksLikeUrl(trimmed)) {
    return { ok: false, reason: "invalid" };
  }

  if (FOLDER_ID_PATTERN.test(trimmed)) {
    return { ok: true, folderId: trimmed, source: "id" };
  }

  return { ok: false, reason: "invalid" };
}

export function folderParseErrorMessage(reason: FolderParseFailure["reason"]): string {
  switch (reason) {
    case "empty":
      return "Enter a Drive folder URL or folder ID.";
    case "invalid":
      return "That is not a valid Google Drive folder URL or ID.";
  }
}

function looksLikeUrl(value: string): boolean {
  return /^https?:\/\//i.test(value) || value.includes("drive.google.com");
}

function extractFolderIdFromUrl(value: string): string | null {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }

  const host = url.hostname.toLowerCase();
  if (host !== "drive.google.com" && host !== "docs.google.com") {
    return null;
  }

  const folderMatch = url.pathname.match(/\/(?:folders|folder\/d)\/([A-Za-z0-9_-]+)/);
  if (folderMatch?.[1]) {
    return folderMatch[1];
  }

  if (/\/file\/d\//.test(url.pathname)) {
    return null;
  }

  const queryId = url.searchParams.get("id");
  if (queryId !== null && FOLDER_ID_PATTERN.test(queryId)) {
    return queryId;
  }

  return null;
}
