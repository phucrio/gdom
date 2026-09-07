/* global window */
import assert from "node:assert/strict";
import console from "node:console";
import { createRequire } from "node:module";
import { mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "vite";

const { chromium } = createRequire(import.meta.url)("playwright");
const output = join(tmpdir(), "gdom-storage-qa");
await mkdir(output, { recursive: true });
const fixture = `<html lang="en"><head><title>Account storage QA</title></head><body><div id="root"></div><script type="module">
import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {AvatarMenu} from '/src/accounts/AvatarMenu.tsx';
import {createTauriBackend} from '/src/ipc/tauri.ts';
import {mockIPC} from '@tauri-apps/api/mocks';
import '/src/App.css';
const accounts = ['source', 'target'].map(id => ({id, email:id+'@gmail.com', displayName:id, label:null, authStatus:'CONNECTED'}));
window.storageMode='ready'; window.storageRequests=[];
mockIPC(async (command,args) => {
  if(command!=='get_account_storage') throw new Error('Unexpected command '+command);
  window.storageRequests.push(args.input.accountId);
  if(args.input.accountId==='target') return {usageBytes:2147483648,limitBytes:16106127360};
  if(window.storageMode==='slow') return new Promise(resolve=>{window.finishStorage=resolve;});
  if(window.storageMode==='error') throw new Error('Offline');
  if(window.storageMode==='unlimited') return {usageBytes:107374182400,limitBytes:null};
  if(window.storageMode==='full') return {usageBytes:19327352832,limitBytes:16106127360};
  return {usageBytes:107374182400,limitBytes:5497558138880};
});
const backend=createTauriBackend();
function Fixture(){const[active,setActive]=useState('source');return React.createElement('div',{style:{display:'flex',justifyContent:'flex-end',padding:24}},React.createElement(AvatarMenu,{activeAccount:accounts.find(account=>account.id===active),accounts,backend,onSelectAccount:setActive,onAddAccount:()=>{},onAnnounce:()=>{},onRefresh:()=>{}}));}
createRoot(document.getElementById('root')).render(React.createElement(Fixture));
</script></body></html>`;
const server = await createServer({ server: { host: "127.0.0.1", port: 0, strictPort: false }, plugins: [{
  name: "storage-fixture", configureServer(vite) {
    vite.middlewares.use("/__storage-qa", async (_request, response) => {
      response.setHeader("Content-Type", "text/html");
      response.end(await vite.transformIndexHtml("/__storage-qa", fixture));
    });
  },
}] });
let browser;
try {
  await server.listen();
  browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 750 } });
  await page.goto(`${server.resolvedUrls.local[0]}__storage-qa`);
  const openMenu = () => page.getByRole("button", { name: /Account menu for/ }).click();
  await openMenu();
  await page.getByText("100.0 GB used of 5.0 TB", { exact: true }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.storageRequests), ["source"]);
  for (const width of [1100, 375]) {
    await page.setViewportSize({ width, height: 520 });
    const bounds = await page.locator(".avatar-dropdown").boundingBox();
    assert.ok(bounds.x >= 0 && bounds.x + bounds.width <= width && bounds.y + bounds.height <= 520);
    await page.screenshot({ path: join(output, `storage-${width}.png`) });
  }
  for (const mode of ["error", "unlimited", "full"]) {
    await page.keyboard.press("Escape");
    await page.evaluate(value => { window.storageMode = value; }, mode);
    await openMenu();
    if (mode === "error") {
      await page.getByText("Storage usage unavailable.", { exact: true }).waitFor();
      await page.screenshot({ path: join(output, "storage-error.png") });
      await page.evaluate(() => { window.storageMode = "ready"; });
      await page.getByRole("button", { name: "Retry storage" }).click();
      await page.getByText("100.0 GB used of 5.0 TB", { exact: true }).waitFor();
    } else {
      await page.getByText(mode === "full" ? "120% used · Storage full" : "100.0 GB used · No storage limit", { exact: true }).waitFor();
      assert.equal(await page.locator("meter").count(), mode === "full" ? 1 : 0);
      await page.screenshot({ path: join(output, `storage-${mode}.png`) });
    }
  }
  await page.keyboard.press("Escape");
  await page.evaluate(() => { window.storageMode = "slow"; });
  await openMenu();
  await page.getByText("Loading storage…", { exact: true }).waitFor();
  await page.screenshot({ path: join(output, "storage-loading.png") });
  await page.getByRole("menuitem", { name: /target target@gmail.com/ }).click();
  await openMenu();
  await page.getByText("2.0 GB used of 15.0 GB", { exact: true }).waitFor();
  await page.evaluate(() => window.finishStorage({ usageBytes: 0, limitBytes: 1024 }));
  await page.getByText("2.0 GB used of 15.0 GB", { exact: true }).waitFor();
  assert.equal(await page.getByText("0 B used of 1.0 KB", { exact: true }).count(), 0);
  await page.screenshot({ path: join(output, "storage-switched.png") });
  console.log(`PASS: active-account routing, refresh on open, loading, error/retry, unlimited, full, narrow layout, and stale-response isolation. Captures: ${output}`);
} finally {
  await browser?.close();
  await server.close();
}
