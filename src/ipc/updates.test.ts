import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { createTauriBackend } from "./tauri.ts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => { vi.clearAllMocks(); });

it("routes updater requests through backend commands with explicit confirmation", async () => {
  const backend = createTauriBackend();
  await backend.getUpdateStatus();
  await backend.checkForUpdates();
  await backend.downloadUpdate({ confirmed: true });
  await backend.installUpdate({ confirmed: true });
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["get_update_status", undefined],
    ["check_for_updates", undefined],
    ["download_update", { input: { confirmed: true } }],
    ["install_update", { input: { confirmed: true } }],
  ]);
});
