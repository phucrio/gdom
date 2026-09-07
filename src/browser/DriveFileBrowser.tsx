import { useCallback, useEffect, useRef, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import type { AccountDto, DriveFileItemDto, JobDto } from "../ipc/types.ts";
import { ContextMenu, type ContextMenuItemAction } from "./ContextMenu.tsx";
import { FileTypeIcon } from "./FileTypeIcon.tsx";
import { formatDate, formatFileSize, getFileIconKind } from "./format.ts";
import { RenameDialog, TrashConfirmDialog } from "./ItemActionDialogs.tsx";
import { ItemDetailsDialog } from "./ItemDetailsDialog.tsx";
import { OwnerCell } from "./OwnerCell.tsx";
import { OwnerPicker } from "./OwnerPicker.tsx";

export type BreadcrumbItem = {
  resourceKey: string | null;
  id: string;
  name: string;
};

type DriveFileBrowserProps = {
  account: AccountDto;
  accounts: AccountDto[];
  backend: BackendPort;
  onAnnounce: (message: string) => void;
  onMigrationStarted: (job: JobDto) => void;
  onAddAccount: () => void;
};

export function DriveFileBrowser({
  account,
  accounts,
  backend,
  onAnnounce,
  onMigrationStarted,
  onAddAccount,
}: DriveFileBrowserProps) {
  const [breadcrumbs, setBreadcrumbs] = useState<BreadcrumbItem[]>([
    { id: "root", name: "My Drive", resourceKey: null },
  ]);
  const currentFolder = breadcrumbs[breadcrumbs.length - 1] ?? { id: "root", name: "My Drive", resourceKey: null };

  const [items, setItems] = useState<DriveFileItemDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [nextPageToken, setNextPageToken] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);

  const [sortField, setSortField] = useState<"name" | "modifiedTime">("name");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("asc");

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // Context Menu State
  const [menuAnchor, setMenuAnchor] = useState<{ x: number; y: number } | null>(null);
  const [menuItems, setMenuItems] = useState<DriveFileItemDto[]>([]);

  // Dialog States
  const [detailsTarget, setDetailsTarget] = useState<DriveFileItemDto | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<DriveFileItemDto | null>(null);
  const [trashTargets, setTrashTargets] = useState<DriveFileItemDto[]>([]);
  const [ownerPickerOpen, setOwnerPickerOpen] = useState(false);

  // Request race condition prevention
  const activeReqIdRef = useRef(0);

  const loadFolder = useCallback(
    async (folder: BreadcrumbItem, pageToken: string | null = null, append = false) => {
      const reqId = ++activeReqIdRef.current;
      if (!append) {
        setLoading(true);
        setError(null);
      } else {
        setLoadingMore(true);
      }

      try {
        const orderParam =
          sortField === "name"
            ? sortOrder === "asc"
              ? "folder,name"
              : "folder,name desc"
            : sortOrder === "asc"
              ? "folder,modifiedTime"
              : "folder,modifiedTime desc";

        const res = await backend.listDriveFiles({
          accountId: account.id,
          folderId: folder.id === "root" ? null : folder.id,
          folderResourceKey: folder.resourceKey,
          pageToken,
          pageSize: 50,
          orderBy: orderParam,
        });

        if (activeReqIdRef.current !== reqId) return;

        setItems((prev) => (append ? [...prev, ...res.items] : res.items));
        setNextPageToken(res.nextPageToken);
      } catch (err: unknown) {
        if (activeReqIdRef.current !== reqId) return;
        const msg = err instanceof Error ? err.message : "Failed to load files.";
        setError(msg);
      } finally {
        if (activeReqIdRef.current === reqId) {
          setLoading(false);
          setLoadingMore(false);
        }
      }
    },
    [account.id, backend, sortField, sortOrder],
  );

  useEffect(() => {
    setSelectedIds(new Set());
    setMenuAnchor(null);
    setMenuItems([]);
    setDetailsTarget(null);
    setOpenError(null);
    setRenameTarget(null);
    setTrashTargets([]);
    setOwnerPickerOpen(false);
    void loadFolder(currentFolder);
  }, [currentFolder, loadFolder]);

  function handleSort(field: "name" | "modifiedTime") {
    if (sortField === field) {
      setSortOrder((prev) => (prev === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortOrder("asc");
    }
  }

  function handleRowClick(event: React.MouseEvent, item: DriveFileItemDto) {
    if (event.ctrlKey || event.metaKey) {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        if (next.has(item.id)) {
          next.delete(item.id);
        } else {
          next.add(item.id);
        }
        return next;
      });
    } else if (event.shiftKey && items.length > 0) {
      const lastSelectedId = Array.from(selectedIds).pop();
      const lastIndex = items.findIndex((i) => i.id === lastSelectedId);
      const currentIndex = items.findIndex((i) => i.id === item.id);
      if (lastIndex !== -1 && currentIndex !== -1) {
        const start = Math.min(lastIndex, currentIndex);
        const end = Math.max(lastIndex, currentIndex);
        const next = new Set(selectedIds);
        for (let i = start; i <= end; i++) {
          const it = items[i];
          if (it) next.add(it.id);
        }
        setSelectedIds(next);
      } else {
        setSelectedIds(new Set([item.id]));
      }
    } else {
      setSelectedIds(new Set([item.id]));
    }
  }

  async function openInGoogleDrive(item: DriveFileItemDto) {
    setOpenError(null);
    try {
      await backend.openDriveItem({accountId: account.id, fileId: item.id, resourceKey: item.resourceKey ?? null});
    } catch (error: unknown) {
      setOpenError(error instanceof Error ? error.message : "Failed to open Google Drive.");
    }
  }

  function handleRowDoubleClick(item: DriveFileItemDto) {
    const folderId = item.folderId;
    if (folderId) {
      setSelectedIds(new Set());
      setMenuAnchor(null);
      setMenuItems([]);
      setBreadcrumbs((previous) => {
        const ancestorIndex = previous.findIndex(crumb => crumb.id === folderId);
        return ancestorIndex >= 0
          ? previous.slice(0, ancestorIndex + 1).map((crumb, index) => index === ancestorIndex
            ? { ...crumb, resourceKey: item.folderResourceKey ?? crumb.resourceKey } : crumb)
          : [...previous, { id: folderId, name: item.name, resourceKey: item.folderResourceKey ?? null }];
      });
      onAnnounce(`Opened folder ${item.name}`);
    } else {
      void openInGoogleDrive(item);
    }
  }

  function handleContextMenu(event: React.MouseEvent, item: DriveFileItemDto) {
    event.preventDefault();
    const isSelected = selectedIds.has(item.id);
    let targets: DriveFileItemDto[];
    if (isSelected && selectedIds.size > 1) {
      targets = items.filter((i) => selectedIds.has(i.id));
    } else {
      setSelectedIds(new Set([item.id]));
      targets = [item];
    }
    setMenuItems(targets);
    setMenuAnchor({ x: event.clientX, y: event.clientY });
  }

  function handleActionClick(event: React.MouseEvent, item: DriveFileItemDto) {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const isSelected = selectedIds.has(item.id);
    let targets: DriveFileItemDto[];
    if (isSelected && selectedIds.size > 1) {
      targets = items.filter((i) => selectedIds.has(i.id));
    } else {
      setSelectedIds(new Set([item.id]));
      targets = [item];
    }
    setMenuItems(targets);
    setMenuAnchor({ x: rect.left, y: rect.bottom + 4 });
  }

  function handleMenuAction(action: ContextMenuItemAction, targetItems: DriveFileItemDto[]) {
    const first = targetItems[0];
    switch (action) {
      case "open":
        if (first) handleRowDoubleClick(first);
        break;
      case "openGoogleDrive":
        if (first) void openInGoogleDrive(first);
        break;
      case "viewDetails":
        if (first) setDetailsTarget(first);
        break;
      case "rename":
        if (first) {
          setRenameTarget(first);
        }
        break;
      case "copyLink":
        if (first?.webViewLink) {
          void navigator.clipboard.writeText(first.webViewLink);
          onAnnounce("Link copied to clipboard.");
        }
        break;
      case "transferOwnership":
        setOwnerPickerOpen(true);
        break;
      case "moveToTrash":
        setTrashTargets(targetItems);
        break;
    }
  }

  async function handleRenameConfirm(newName: string) {
    if (!renameTarget) return;
    await backend.renameDriveItem({
      accountId: account.id,
      fileId: renameTarget.id,
      newName,
    });
    onAnnounce(`Renamed to "${newName}".`);
    void loadFolder(currentFolder);
  }

  async function handleTrashConfirm() {
    for (const item of trashTargets) {
      await backend.trashDriveItem({
        accountId: account.id,
        fileId: item.id,
      });
    }
    onAnnounce(`Moved ${trashTargets.length} item(s) to trash.`);
    void loadFolder(currentFolder);
  }

  const selectedList = items.filter((i) => selectedIds.has(i.id));

  return (
    <div className="drive-browser" role="region" aria-label="Drive file explorer">
      {/* Top Header & Breadcrumb */}
      <div className="browser-toolbar">
        <nav className="breadcrumbs" aria-label="Breadcrumb folder trail">
          <ol className="breadcrumb-list">
            {breadcrumbs.map((crumb, idx) => {
              const isLast = idx === breadcrumbs.length - 1;
              return (
                <li key={crumb.id} className="breadcrumb-item">
                  {isLast ? (
                    <span aria-current="page" className="breadcrumb-current">
                      {crumb.name}
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="breadcrumb-link"
                      onClick={() => {
                        setBreadcrumbs((prev) => prev.slice(0, idx + 1));
                      }}
                    >
                      {crumb.name}
                    </button>
                  )}
                  {!isLast && <span className="breadcrumb-sep" aria-hidden="true">/</span>}
                </li>
              );
            })}
          </ol>
        </nav>

        <div className="toolbar-actions">
          {selectedIds.size > 0 && (
            <span className="selection-count">
              {selectedIds.size} selected
            </span>
          )}
          <button
            type="button"
            className="secondary-button icon-button"
            title="Refresh files"
            aria-label="Refresh files"
            onClick={() => void loadFolder(currentFolder)}
            disabled={loading}
          >
            ↻ Refresh
          </button>
        </div>
      </div>

      {/* Main Table Area */}
      <div className="browser-table-wrapper">
        <table className="drive-table" aria-label="Google Drive files">
          <thead>
            <tr>
              <th scope="col" className="col-name">
                <button
                  type="button"
                  className="table-sort-button"
                  onClick={() => handleSort("name")}
                >
                  Name {sortField === "name" && (sortOrder === "asc" ? "▲" : "▼")}
                </button>
              </th>
              <th scope="col" className="col-owner">
                Owner
              </th>
              <th scope="col" className="col-modified">
                <button
                  type="button"
                  className="table-sort-button"
                  onClick={() => handleSort("modifiedTime")}
                >
                  Modified Date {sortField === "modifiedTime" && (sortOrder === "asc" ? "▲" : "▼")}
                </button>
              </th>
              <th scope="col" className="col-size">
                Size
              </th>
              <th scope="col" className="col-action">
                Action
              </th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <tr key={`skeleton-${i}`} className="table-row skeleton-row">
                  <td className="col-name"><div className="skeleton-box" /></td>
                  <td className="col-owner"><div className="skeleton-box" /></td>
                  <td className="col-modified"><div className="skeleton-box" /></td>
                  <td className="col-size"><div className="skeleton-box" /></td>
                  <td className="col-action"><div className="skeleton-box" /></td>
                </tr>
              ))
            ) : error ? (
              <tr>
                <td colSpan={5} className="table-empty error-cell" role="alert">
                  <p className="error-title">Failed to load files</p>
                  <p className="error-desc">{error}</p>
                  <button
                    type="button"
                    className="secondary-button"
                    onClick={() => void loadFolder(currentFolder)}
                  >
                    Retry
                  </button>
                </td>
              </tr>
            ) : items.length === 0 ? (
              <tr>
                <td colSpan={5} className="table-empty">
                  <p className="empty-title">This folder is empty</p>
                </td>
              </tr>
            ) : (
              items.map((item) => {
                const isSelected = selectedIds.has(item.id);
                const iconKind = getFileIconKind(item.name, item.mimeType, item.isFolder, item.shortcutTargetId !== null);
                const { display: modDate, full: fullDate } = formatDate(item.modifiedTime);
                const sizeStr = item.folderId ? "—" : formatFileSize(item.size);


                return (
                  <tr
                    key={item.id}
                    className={`table-row ${isSelected ? "selected" : ""}`}
                    onClick={(e) => handleRowClick(e, item)}
                    onDoubleClick={() => handleRowDoubleClick(item)}
                    onContextMenu={(e) => handleContextMenu(e, item)}
                    tabIndex={0}
                    aria-selected={isSelected}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        handleRowDoubleClick(item);
                      }
                    }}
                  >
                    <td
                      className="col-name"
                      onClick={(e) => {
                        if (item.folderId && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
                          e.stopPropagation();
                          handleRowDoubleClick(item);
                        }
                      }}
                    >
                      <div className="name-cell">
                        <span className="item-icon"><FileTypeIcon kind={iconKind} /></span>
                        {item.folderId !== null ? (
                          <button
                            type="button"
                            className="folder-link-button"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleRowDoubleClick(item);
                            }}
                          >
                            {item.name}
                          </button>
                        ) : (
                          <span className="item-title" title={item.name}>{item.name}</span>
                        )}
                      </div>
                    </td>
                    <td className="col-owner">
                      <OwnerCell item={item} account={account} accounts={accounts} />
                    </td>
                    <td className="col-modified" title={fullDate}>
                      {modDate}
                    </td>
                    <td className="col-size">
                      {sizeStr}
                    </td>
                    <td className="col-action">
                      <button
                        type="button"
                        className="action-menu-trigger"
                        aria-label={`Action menu for ${item.name}`}
                        aria-haspopup="true"
                        onClick={(e) => handleActionClick(e, item)}
                      >
                        ⋮
                      </button>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>

        {nextPageToken && !loading && (
          <div className="load-more-row">
            <button
              type="button"
              className="secondary-button"
              onClick={() => void loadFolder(currentFolder, nextPageToken, true)}
              disabled={loadingMore}
            >
              {loadingMore ? "Loading more…" : "Load more files"}
            </button>
          </div>
        )}
      </div>

      {openError && <p className="error" role="alert">{openError}</p>}
      {detailsTarget && <ItemDetailsDialog item={detailsTarget} onClose={() => setDetailsTarget(null)} />}

      {/* Context Menu Component */}
      <ContextMenu
        anchorPosition={menuAnchor}
        targetItems={menuItems}
        onClose={() => setMenuAnchor(null)}
        onAction={handleMenuAction}
      />

      {/* Rename Dialog */}
      {renameTarget && (
        <RenameDialog
          isOpen={renameTarget !== null}
          currentName={renameTarget.name}
          onClose={() => setRenameTarget(null)}
          onConfirm={handleRenameConfirm}
        />
      )}

      {/* Trash Confirm Dialog */}
      {trashTargets.length > 0 && (
        <TrashConfirmDialog
          isOpen={trashTargets.length > 0}
          itemsCount={trashTargets.length}
          itemName={trashTargets[0]?.name ?? ""}
          onClose={() => setTrashTargets([])}
          onConfirm={handleTrashConfirm}
        />
      )}

      {/* Owner Picker Popover / Dialog */}
      {ownerPickerOpen && selectedList.length > 0 && (
        <OwnerPicker
          isOpen={ownerPickerOpen}
          sourceAccount={account}
          accounts={accounts}
          selectedItems={selectedList}
          backend={backend}
          onClose={() => setOwnerPickerOpen(false)}
          onMigrationStarted={onMigrationStarted}
          onAddAccount={onAddAccount}
          onAnnounce={onAnnounce}
        />
      )}
    </div>
  );
}
