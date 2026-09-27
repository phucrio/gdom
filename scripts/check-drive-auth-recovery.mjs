import assert from "node:assert/strict";
import console from "node:console";
import process from "node:process";
import { createRequire } from "node:module";
import { createServer } from "vite";

const { chromium } = createRequire(import.meta.url)("playwright");
const server = await createServer({
  server: { host: "127.0.0.1", port: 0 },
  plugins: [{
    name: "drive-auth-recovery-qa",
    configureServer(vite) {
      vite.middlewares.use("/__drive-auth-qa", async (_request, response) => {
        const html = `<!doctype html>
<html lang="en">
  <head><title>Drive auth recovery QA</title></head>
  <body>
    <div id="root"></div>
    <div id="drive-auth-announcement" class="live-region" aria-live="polite" aria-atomic="true" role="status"></div>
    <script type="module" src="/scripts/drive-auth-qa-fixture.tsx"></script>
  </body>
</html>`;
        response.statusCode = 200;
        response.setHeader("Content-Type", "text/html");
        response.end(await vite.transformIndexHtml("/__drive-auth-qa", html));
      });
    },
  }],
});
let browser;
try {
  await server.listen();
  const browserOptions = process.env.GDOM_QA_BROWSER
    ? { channel: process.env.GDOM_QA_BROWSER }
    : {};
  browser = await chromium.launch({ headless: true, ...browserOptions });
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  const pageErrors = [];
  page.on("pageerror", (error) => {
    const message = error.message;
    pageErrors.push(message);
  });

  const address = server.resolvedUrls?.local?.[0];
  assert.ok(address, "Vite did not publish a local QA URL");
  await page.goto(`${address}__drive-auth-qa`);
  await page.getByText("account requires re-authentication", { exact: true }).waitFor();

  const reauthenticate = page.getByRole("button", {
    name: "Reauthenticate and retry",
    exact: true,
  });
  await reauthenticate.waitFor();
  await reauthenticate.click();
  await page.waitForFunction(
    () =>
      document.getElementById("drive-auth-announcement")?.textContent ===
      "OAuth authorization was cancelled.",
  );
  assert.equal(
    await page.locator("#drive-auth-announcement").textContent(),
    "OAuth authorization was cancelled.",
  );
  assert.equal(await page.evaluate("window.driveAuthQa.reauthenticationCalls"), 1);
  assert.equal(await page.evaluate("window.driveAuthQa.listCalls"), 1);
  await reauthenticate.click();

  await page.getByText("report-after-reauth.txt", { exact: true }).waitFor();
  assert.equal(
    await page.locator("#drive-auth-announcement").textContent(),
    "Account owner@example.test reconnected. Reloading Drive files.",
  );
  assert.equal(await page.evaluate("window.driveAuthQa.reauthenticationCalls"), 2);
  assert.equal(await page.evaluate("window.driveAuthQa.listCalls"), 2);
  assert.equal(await page.evaluate("window.driveAuthQa.accountRefreshCalls"), 1);
  assert.deepEqual(
    await page.evaluate("window.driveAuthQa.commands"),
    ["reauthenticate:account-1", "reauthenticate:account-1"],
    "reauthentication must target the selected account",
  );
  assert.equal(pageErrors.length, 0, `Unexpected browser errors: ${pageErrors.join("; ")}`);

  console.log("PASS: auth failure offers system-browser reauthentication; cancellation is retryable; success reloads My Drive");
} finally {
  if (browser) {
    await browser.close();
  }
  await server.close();
}
