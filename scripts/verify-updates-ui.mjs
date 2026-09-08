/* global window, document */
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import process from "node:process";
import console from "node:console";
import { createServer } from "vite";
import { chromium } from "playwright";
const output = await mkdtemp(join(tmpdir(), "gdom-updates-qa-"));
const server = await createServer({ server: { host: "127.0.0.1", port: 0 }, plugins: [{
  name: "updates-qa", configureServer(vite) {
    vite.middlewares.use("/__updates-qa", async (_request, response) => {
      response.setHeader("Content-Type", "text/html");
      response.end(await vite.transformIndexHtml("/__updates-qa", '<html lang="en"><head><title>Updates QA</title></head><body><div id="root"></div><script type="module" src="/scripts/progress-ui-fixture.tsx"></script></body></html>'));
    });
  },
}] });
await server.listen();
const browser = await chromium.launch({ headless: true, ...(process.env.GDOM_QA_BROWSER === "chromium" ? {} : { channel: process.env.GDOM_QA_BROWSER || "chrome" }) });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, reducedMotion: "reduce" });
const errors = [];
page.on("pageerror", error => errors.push(error.message));
const count = command => page.evaluate(name => window.progressQa.commands.filter(value => value === name).length, command);
async function capture(state) {
  for (const width of [375, 768, 1280]) {
    await page.setViewportSize({ width, height: 750 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true);
    await page.screenshot({ path: join(output, `${state}-${width}.png`), fullPage: true });
  }
}
try {
  await page.goto(`${server.resolvedUrls.local[0]}__updates-qa`);
  await page.getByRole("button", { name: "Check for updates", exact: true }).click();
  await page.getByText("Automatic updates are unavailable for this installation.").waitFor();
  await capture("unavailable");
  await page.keyboard.press("Escape");
  assert.equal(await page.getByRole("button", { name: "Check for updates", exact: true }).evaluate(element => element === document.activeElement), true);
  await page.evaluate(() => window.progressQa.updateScenario("idle"));
  await page.getByRole("button", { name: "Update available", exact: true }).waitFor();
  assert.equal(await count("checkForUpdates"), 1);
  assert.equal(await count("downloadUpdate"), 0);
  await page.evaluate(() => window.progressQa.emptyAccounts(true));
  await page.locator(".landing-shell").waitFor();
  assert.equal(await count("checkForUpdates"), 1);
  await page.getByRole("button", { name: "Update available", exact: true }).click();
  await page.getByText("<script>plain text</script>", { exact: false }).waitFor();
  assert.equal(await page.locator(".update-notes script").count(), 0);
  await capture("available-prelogin");
  await page.keyboard.press("Escape");
  assert.equal(await count("downloadUpdate"), 0);
  await page.getByRole("button", { name: "Update available", exact: true }).click();
  await page.getByRole("button", { name: "Confirm download", exact: true }).click();
  await page.getByRole("progressbar", { name: "Update download" }).waitFor();
  await capture("downloading");
  await page.evaluate(() => window.progressQa.unknownDownloadSize());
  await page.waitForFunction(() => !document.querySelector("progress")?.hasAttribute("value"));
  await capture("downloading-unknown");
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.progressQa.finishDownload());
  await page.getByRole("button", { name: "Update available", exact: true }).click();
  await page.getByText("The update is verified and ready to install. Installation will restart GDOM.").waitFor();
  assert.equal(await count("installUpdate"), 0);
  await capture("ready");
  await page.getByRole("button", { name: "Install and restart", exact: true }).click();
  await page.getByText(/Installation is deferred/).waitFor();
  await capture("deferred");
  await page.evaluate(() => window.progressQa.finishWork());
  assert.equal(await count("installUpdate"), 1);
  await page.getByRole("button", { name: "Install and restart", exact: true }).click();
  await page.getByText("Installing the update. GDOM will restart…").waitFor();
  assert.equal(await count("installUpdate"), 2);
  await capture("installing");
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.progressQa.failUpdateCheck(true); window.progressQa.updateScenario("idle"); });
  await page.getByRole("button", { name: "Check for updates", exact: true }).click();
  await page.getByRole("alert").filter({ hasText: "Update service is offline." }).waitFor();
  await capture("error");
  await page.evaluate(() => window.progressQa.failUpdateCheck(false));
  await page.getByRole("button", { name: "Retry check", exact: true }).click();
  await page.getByRole("button", { name: "Confirm download", exact: true }).waitFor();
  await page.keyboard.press("Escape");
  await page.evaluate(() => window.progressQa.registryFailure(true));
  await page.getByRole("button", { name: "Retry loading accounts", exact: true }).waitFor();
  await page.getByRole("button", { name: "Update available", exact: true }).click();
  await capture("registry-error");
  await page.keyboard.press("Escape");
  for (const phase of ["checking", "upToDate"]) {
    await page.evaluate(value => window.progressQa.updateScenario(value), phase);
    await page.getByRole("button", { name: "Check for updates", exact: true }).click();
    await page.getByRole("dialog").getByRole("status").filter({ hasText: phase === "checking" ? "Checking for updates" : "latest stable release" }).waitFor();
    await capture(phase);
    await page.keyboard.press("Escape");
  }
  assert.deepEqual(errors, []);
  console.log(`PASS updater confirmations, polling, deferred, prelogin, retry, focus, notes and responsive layouts: ${output}`);
} catch (failure) { await page.screenshot({ path: join(output, "failure.png"), fullPage: true }); console.log(await page.locator("body").innerText()); throw failure; } finally { await browser.close(); await server.close(); }
