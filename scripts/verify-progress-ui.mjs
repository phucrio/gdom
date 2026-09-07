/* global window, Event, document, requestAnimationFrame */
import process from "node:process";
import console from "node:console";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createServer } from "vite";

const { chromium } = createRequire(import.meta.url)("playwright");
const output = process.env.GDOM_QA_OUTPUT ?? join(tmpdir(), "gdom-progress-qa");
await mkdir(output, { recursive: true });
const server = await createServer({ server: { host: "127.0.0.1", port: 0 }, plugins: [{
  name: "progress-qa-fixture", configureServer(vite) {
    vite.middlewares.use("/__progress-qa", async (_request, response) => {
      response.setHeader("Content-Type", "text/html");
      response.end(await vite.transformIndexHtml("/__progress-qa", '<html lang="en"><head><title>Progress QA</title></head><body><div id="root"></div><script type="module" src="/scripts/progress-ui-fixture.tsx"></script></body></html>'));
    });
  },
}] });
await server.listen();
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const assertText = async (text) => { await page.getByText(text, { exact: true }).waitFor(); };
try {
await page.goto(`${server.resolvedUrls.local[0]}__progress-qa`);
assert.notEqual(await page.locator(".global-transfer-panel").evaluate((element) => element.ownerDocument.defaultView?.getComputedStyle(element).position), "fixed", "Progress must reserve layout space instead of covering workspace content");
await assertText("0 / 2 processed · 0 succeeded · 0 failed · 0 skipped");
await page.setViewportSize({ width: 720, height: 520 });
const shortWindowPanel = await page.locator(".global-transfer-panel").boundingBox();
assert.ok(shortWindowPanel && shortWindowPanel.y >= 0 && shortWindowPanel.y + shortWindowPanel.height <= 520, "Progress must remain reachable in a short window");
const shortWindowAction = await page.getByRole("button", { name: "Action menu for release-notes.md" }).boundingBox();
assert.ok(shortWindowPanel && shortWindowAction && shortWindowAction.y + shortWindowAction.height <= shortWindowPanel.y, "Workspace actions must remain reachable above progress");
await page.setViewportSize({ width: 1280, height: 900 });
await page.getByRole("button", { name: "Action menu for release-notes.md" }).click();
await page.getByRole("menuitem", { name: "Transfer ownership…" }).click();
const startTransfer = page.getByRole("button", { name: "Start transfer", exact: true });
assert.equal(await startTransfer.isDisabled(), true);
await page.getByRole("radio", { name: "Select target (target@gmail.com)" }).check();
assert.equal(await page.evaluate(() => window.progressQa.commands.some((command) => command.startsWith("startTransferOperation:"))), false);
await page.evaluate(() => window.progressQa.failStartTransfer(true));
await startTransfer.click();
await page.getByRole("alert").filter({ hasText: "Transfer start unavailable" }).waitFor();
assert.equal(await page.getByRole("radio", { name: "Select target (target@gmail.com)" }).isChecked(), true);
assert.equal(await startTransfer.isDisabled(), false);
await page.evaluate(() => window.progressQa.failStartTransfer(false));
await startTransfer.click();
await page.waitForFunction(() => window.progressQa.commands.includes("startTransferOperation:source:target:drive-item-1:false"));
await page.getByRole("button", { name: "Expand migration details" }).click();
  await page.getByText("Report.pdf", { exact: true }).waitFor();
  await page.getByRole("feed").evaluate((element) => element.dispatchEvent(new Event("scroll", { bubbles: true })));
  await page.getByText("Folder", { exact: true }).waitFor();
  // Given a loaded panel with a failing refresh held in flight.
  await page.evaluate(() => { window.progressQa.failJob(true); window.progressQa.holdJob(); window.progressQa.setStatus("RUNNING"); });
  await page.waitForFunction(() => window.progressQa.jobHeld);
  // When completion arrives before that refresh rejects, drain its queued refresh.
  await page.evaluate(() => { window.progressQa.complete(); window.progressQa.failJob(false); window.progressQa.releaseJob(); });
  // Then the completed snapshot appears without another event or a manual retry.
  await page.getByText("2 / 2 processed · 1 succeeded · 1 failed · 0 skipped", { exact: true }).waitFor({ timeout: 3000 });
  await assertText("2 / 2 processed · 1 succeeded · 1 failed · 0 skipped");
  await assertText("verified");
  await assertText("permanent failed");
  await page.waitForFunction(() => {
    const fill = document.querySelector(".progress-bar-fill");
    const track = document.querySelector(".progress-bar-bg");
    return fill && track && Math.abs(fill.getBoundingClientRect().width - track.getBoundingClientRect().width) < 1;
  });
  await page.screenshot({ path: join(output, "completed-desktop.png"), fullPage: true });
  // Given an already loaded snapshot, an ordinary refresh failure stays recoverable.
  await page.evaluate(() => { window.progressQa.failJob(true); window.progressQa.setStatus("RUNNING"); });
  await page.getByRole("alert").filter({ hasText: "Progress unavailable" }).waitFor();
  const requestsAfterFailure = await page.evaluate(() => window.progressQa.jobRequests);
  for (let frame = 0; frame < 3; frame += 1) await page.evaluate(() => new Promise(requestAnimationFrame));
  assert.equal(await page.evaluate(() => window.progressQa.jobRequests), requestsAfterFailure, "Failure alone must not retry indefinitely");
  const retry = page.getByRole("button", { name: "Retry", exact: true });
  await retry.waitFor({ timeout: 3000 });
  for (const width of [375, 640, 768, 900, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await retry.focus();
    await page.screenshot({ path: join(output, `progress-error-${width}.png`), fullPage: true });
    const bounds = await retry.boundingBox();
    assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= width);
  }
  // When the user retries after recovery, the alert clears and the new state loads.
  await page.evaluate(() => window.progressQa.failJob(false));
  await retry.click();
  await page.getByRole("alert").filter({ hasText: "Progress unavailable" }).waitFor({ state: "hidden" });
  await page.getByRole("button", { name: "Pause", exact: true }).waitFor();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByText("Cancel this migration? Transfers already completed will not be reversed.", { exact: true }).waitFor();
  assert.equal(await page.evaluate(() => window.progressQa.commands.includes("cancelMigration")), false);
  await page.evaluate(() => window.progressQa.setStatus("COMPLETED"));
  await page.getByText("Cancel this migration? Transfers already completed will not be reversed.", { exact: true }).waitFor({ state: "hidden" });
  assert.equal(await page.getByRole("button", { name: "Cancel migration", exact: true }).count(), 0);
  await page.evaluate(() => window.progressQa.setStatus("RUNNING"));
  await page.getByRole("button", { name: "Pause", exact: true }).waitFor();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByText("Cancel this migration? Transfers already completed will not be reversed.", { exact: true }).waitFor();
  await page.getByRole("button", { name: "Keep running", exact: true }).click();
  await page.evaluate(() => window.progressQa.setStatus("PAUSED", "scan"));
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await page.waitForFunction(() => window.progressQa.commands.includes("startScan"));
  assert.equal(await page.getByRole("button", { name: "Cancel", exact: true }).count(), 0);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.waitForFunction(() => window.progressQa.commands.includes("pauseScan"));
  for (const status of ["QUEUED", "AUTH_REQUIRED", "SOURCE_RATE_LIMITED", "WAITING_FOR_QUOTA"]) {
    await page.evaluate((value) => window.progressQa.setStatus(value), status);
    await page.getByRole("button", { name: "Resume", exact: true }).click();
    await page.getByRole("button", { name: "Pause", exact: true }).waitFor();
  }
  await page.evaluate(() => window.progressQa.setStatus("CANARY_REVIEW"));
  const approve = page.getByRole("button", { name: "Approve remaining transfers", exact: true });
  await approve.waitFor();
  assert.equal(await approve.isEnabled(), true);
  assert.equal(await page.locator("#canary-confirmation-email").count(), 0);
  assert.equal(await page.evaluate(() => window.progressQa.commands.includes("continueMigration")), false, "Canary review must wait for explicit approval");
  await page.screenshot({ path: join(output, "canary-review-desktop.png"), fullPage: true });
  for (const width of [375, 768]) {
    await page.setViewportSize({ width, height: 900 });
    await page.screenshot({ path: join(output, `canary-review-${width}.png`), fullPage: true });
    const panel = await page.locator(".global-transfer-panel").boundingBox();
    assert.ok(panel && panel.x >= 0 && panel.x + panel.width <= width);
    const cancel = await page.getByRole("button", { name: "Cancel", exact: true }).boundingBox();
    assert.ok(cancel && cancel.x + cancel.width <= width);
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await approve.click();
  await page.waitForFunction(() => window.progressQa.commands.includes("continueMigration"));
  await page.getByRole("link", { name: "Jobs", exact: true }).click();
  const filter = page.getByLabel("Filter by source or target account");
  await filter.selectOption("source");
  assert.equal(await page.locator(".job-card").count(), 1);
  await filter.selectOption("");
  assert.equal(await page.locator(".job-card").count(), 2);
  await page.evaluate(() => { window.progressQa.holdJob(); window.progressQa.setStatus("RUNNING"); });
  await page.getByRole("button", { name: "Track progress", exact: true }).nth(1).click();
  await page.locator(".global-transfer-panel").getByText("another@gmail.com", { exact: true }).waitFor();
  const requestsBeforeDisposalRelease = await page.evaluate(() => window.progressQa.jobRequests);
  await page.evaluate(() => window.progressQa.releaseJob());
  for (let frame = 0; frame < 3; frame += 1) await page.evaluate(() => new Promise(requestAnimationFrame));
  assert.equal(await page.evaluate(() => window.progressQa.jobRequests), requestsBeforeDisposalRelease, "Disposed snapshot must not drain queued requests");
  await page.locator(".global-transfer-panel").getByText("another@gmail.com", { exact: true }).waitFor();
  await page.evaluate(() => window.progressQa.disconnect());
  await page.getByRole("button", { name: "Account menu for source", exact: false }).click();
  await page.getByRole("menuitem", { name: "Reconnect account", exact: true }).click();
  await page.waitForFunction(() => window.progressQa.commands.includes("reauthenticateAccount"));
  await page.evaluate(() => window.progressQa.registryFailure(true));
  await assertText("Registry unavailable");
  await page.getByRole("button", { name: "Retry loading accounts" }).waitFor();
  assert.equal(await page.getByText("Sign in with Google", { exact: true }).count(), 0);
  await page.screenshot({ path: join(output, "registry-error.png"), fullPage: true });
  assert.deepEqual(errors, []);
  console.log(`PASS: recipient confirmation, short-window progress, queued completion after rejection, loaded-error manual Retry, no failure retry loop, disposal, progress event refresh, pagination, counters, cancel confirmation, scan pause/resume, halted resume, canary approval, account filter, reconnect and registry errors. Screenshots: ${output}`);
} finally {
  await browser.close();
  await server.close();
}
