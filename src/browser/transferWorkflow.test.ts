import { describe, expect, it } from "vitest";
import type { DriveFileItemDto } from "../ipc/types.ts";

function filterSelectedItems(
  items: DriveFileItemDto[],
  selectedIds: Set<string>,
): DriveFileItemDto[] {
  return items.filter((item) => selectedIds.has(item.id));
}

function canTransferOwnership(items: DriveFileItemDto[]): boolean {
  return items.length > 0 && items.every((i) => i.canTransferOwnership);
}

function getAvailableTargets(
  accounts: { id: string; authStatus: string }[],
  sourceAccountId: string,
) {
  return accounts.filter(
    (acc) => acc.id !== sourceAccountId && acc.authStatus !== "DISCONNECTED",
  );
}

describe("Transfer workflow validation", () => {
  const sampleItems: DriveFileItemDto[] = [
    {
      id: "folder-1",
      name: "Documents",
      mimeType: "application/vnd.google-apps.folder",
      isFolder: true,
      size: null,
      modifiedTime: "2026-09-01T10:00:00Z",
      owners: [{ permissionId: "perm-src", emailAddress: "src@gmail.com" }],
      webViewLink: "https://drive.google.com/folder-1",
      canTransferOwnership: true,
      isOwner: true,
      shortcutTargetId: null,
    },
    {
      id: "file-2",
      name: "Shared.pdf",
      mimeType: "application/pdf",
      isFolder: false,
      size: 1024,
      modifiedTime: "2026-09-02T10:00:00Z",
      owners: [{ permissionId: "perm-other", emailAddress: "other@gmail.com" }],
      webViewLink: null,
      canTransferOwnership: false,
      isOwner: false,
      shortcutTargetId: null,
    },
  ];

  it("filters selected items by ID set accurately", () => {
    const selected = filterSelectedItems(sampleItems, new Set(["folder-1"]));
    expect(selected.length).toBe(1);
    const first = selected[0];
    expect(first).toBeDefined();
    if (first) {
      expect(first.id).toBe("folder-1");
    }
  });

  it("permits transfer only when all items are owned by source account", () => {
    expect(canTransferOwnership([sampleItems[0]!])).toBe(true);
    expect(canTransferOwnership([sampleItems[1]!])).toBe(false);
    expect(canTransferOwnership(sampleItems)).toBe(false);
  });

  it("excludes source and disconnected accounts from target candidates", () => {
    const accounts = [
      { id: "src", authStatus: "CONNECTED" },
      { id: "tgt1", authStatus: "CONNECTED" },
      { id: "tgt2", authStatus: "DISCONNECTED" },
      { id: "tgt3", authStatus: "REAUTH_REQUIRED" },
    ];

    const candidates = getAvailableTargets(accounts, "src");
    expect(candidates.map((c) => c.id)).toEqual(["tgt1", "tgt3"]);
  });
});
