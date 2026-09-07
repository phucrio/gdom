import console from 'node:console';
import process from 'node:process';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { createServer } from 'vite';
import { writeFile, unlink } from 'node:fs/promises';

// Run with node; NODE_PATH can point to an existing Playwright installation.
const { chromium } = createRequire(import.meta.url)('playwright');
const server = await createServer({ server: { host: '127.0.0.1', port: 15327 } });
let browser;
let fixtureCreated = false;
try {
  await server.listen();
  browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  const errors = [];
  page.on('pageerror', error => { errors.push(error.message); console.error(error.message); });
  await writeFile('.debug-shortcut.html', `<!doctype html><html><body><div id="root"></div><script type="module">
      import React from 'react';
      import { createRoot } from 'react-dom/client';
      import { DriveFileBrowser } from '/src/browser/DriveFileBrowser.tsx';
      import { createTauriBackend } from '/src/ipc/tauri.ts';
      import '/src/App.css';
      import { mockIPC } from '@tauri-apps/api/mocks';
      globalThis.openedItems = [];
      mockIPC((command, args) => {
        if (command !== 'open_drive_item') throw new Error('Unexpected command: ' + command);
        if (globalThis.failOpen) throw new Error('Browser could not be opened');
        globalThis.openedItems.push(args.fileId);
      });
      const account = { id:'source', email:'source@example.test', googlePermissionId:'source', authStatus:'CONNECTED', displayName:'Test account' };
      const base = { mimeType:'application/vnd.google-apps.shortcut', isFolder:false, size:0, modifiedTime:null,
        owners:[{permissionId:'source',emailAddress:'source@example.test'}], isOwner:true,
        canTransferOwnership:false, webViewLink:null };
      const items = [
        {...base,id:'shortcut-course',name:'Courses',folderId:'target-course',shortcutTargetId:'target-course'},
        {...base,id:'shortcut-study',name:'Huyền học',folderId:'target-study',shortcutTargetId:'target-study'},
        {...base,id:'folder',name:'Ordinary folder',mimeType:'application/vnd.google-apps.folder',isFolder:true,folderId:'folder',shortcutTargetId:null},
        {...base,id:'file-shortcut',name:'Document shortcut',folderId:null,shortcutTargetId:'document'},
        {...base,id:'blocked-shortcut',name:'Unavailable folder',folderId:'blocked',shortcutTargetId:'blocked'},
      ];
      globalThis.requests = [];
      const backend = {...createTauriBackend(), listDriveFiles: async input => {
        globalThis.requests.push(input);
        if(input.folderId === 'blocked') throw new Error('Access denied to shared folder');
        return {items: input.folderId ? [{...base,id:'child',name:'Nested document',folderId:null,shortcutTargetId:null}, {...base,id:'self',name:'This folder',folderId:input.folderId,shortcutTargetId:input.folderId}, {...base,id:'ancestor',name:'Back to root',folderId:'root',shortcutTargetId:'root'}] : items,nextPageToken:null};
      }};
      createRoot(document.getElementById('root')).render(React.createElement(DriveFileBrowser, {
        account,accounts:[account],backend,onAnnounce:()=>{},onMigrationStarted:()=>{},onAddAccount:()=>{}
      }));
    </script></body></html>`, {flag:'wx'});
  fixtureCreated = true;
  const address = server.httpServer.address();
  assert.ok(address && typeof address !== 'string');
  await page.goto('http://127.0.0.1:' + address.port + '/.debug-shortcut.html');
  await page.getByText('Document shortcut', { exact: true }).waitFor();
  assert.equal(await page.getByRole('button', { name: 'Courses', exact: true }).count(), 1,
    'Folder shortcuts must render as navigable folders, not zero-byte files');
  if (process.env['GDOM_QA_SCREENSHOT']) await page.screenshot({ path: process.env['GDOM_QA_SCREENSHOT'], fullPage: true });
  async function verifyOpened(folderId) {
    await page.getByText('Nested document', { exact:true }).waitFor();
    const request = await page.evaluate(() => globalThis.requests.at(-1));
    assert.equal(request.folderId, folderId);
    assert.equal(request.accountId, 'source');
    await page.getByRole('button', { name:'This folder', exact:true }).click();
    assert.equal(await page.locator('.breadcrumb-item').count(), 2);
    await page.getByRole('button', {name:'Back to root',exact:true}).click();
    await page.getByText('Document shortcut', {exact:true}).waitFor();
  }
  await page.getByRole('button', {name:'Courses',exact:true}).click();
  await verifyOpened('target-course');
  await page.getByRole('row').filter({hasText:'Huyền học'}).press('Enter');
  await verifyOpened('target-study');
  await page.getByRole('row').filter({hasText:'Courses'}).locator('.col-owner').dblclick();
  await verifyOpened('target-course');
  await page.getByRole('button', {name:'Action menu for Huyền học',exact:true}).click();
  await page.getByRole('menuitem', {name:'Open folder',exact:true}).click();
  await verifyOpened('target-study');
  await page.getByRole('button', {name:'Ordinary folder',exact:true}).click();
  await verifyOpened('folder');
  assert.equal(await page.getByRole('button', {name:'Document shortcut',exact:true}).count(),0);
  await page.getByRole('button', {name:'Action menu for Document shortcut',exact:true}).click();
  await page.getByRole('menuitem', {name:'Open',exact:true}).click();
  await page.waitForFunction(() => globalThis.openedItems.length === 1);
  assert.deepEqual(await page.evaluate(() => globalThis.openedItems), ['file-shortcut']);
  await page.getByRole('button', {name:'Action menu for Courses',exact:true}).click();
  await page.getByRole('menuitem', {name:'Open in Google Drive',exact:true}).click();
  await page.waitForFunction(() => globalThis.openedItems.length === 2);
  assert.deepEqual(await page.evaluate(() => globalThis.openedItems), ['file-shortcut','shortcut-course']);
  await page.getByRole('button', {name:'Action menu for Huyền học',exact:true}).click();
  await page.getByRole('menuitem', {name:'View details',exact:true}).click();
  const dialog = page.getByRole('dialog', {name:'Item details'});
  await dialog.waitFor();
  assert.ok((await dialog.innerText()).includes('Folder shortcut'));
  assert.ok((await dialog.innerText()).includes('target-study'));
  if (process.env['GDOM_QA_SCREENSHOT']) await page.screenshot({path:process.env['GDOM_QA_SCREENSHOT'].replace('.png','-details.png'),fullPage:true});
  await page.setViewportSize({width:390,height:844});
  assert.equal(await dialog.evaluate(element => element.scrollWidth > element.clientWidth), false);
  if (process.env['GDOM_QA_SCREENSHOT']) await page.screenshot({path:process.env['GDOM_QA_SCREENSHOT'].replace('.png','-details-mobile.png'),fullPage:true});
  await page.setViewportSize({width:1100,height:760});
  await page.keyboard.press('Escape');
  await dialog.waitFor({state:'hidden'});
  await page.evaluate(() => { globalThis.failOpen = true; });
  await page.getByRole('button', {name:'Action menu for Document shortcut',exact:true}).click();
  await page.getByRole('menuitem', {name:'Open',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Browser could not be opened'}).waitFor();
  await page.getByRole('button', {name:'Unavailable folder',exact:true}).click();
  await page.getByText('Access denied to shared folder', {exact:true}).waitFor();
  assert.deepEqual(errors, []);
  console.log('PASS: shortcut click, Enter, double-click, context menu, breadcrumbs, account routing, ordinary folder, file shortcut, native open IPC, opener error, item details, access error');
} finally {
  await browser?.close();
  await server.close();
  if (fixtureCreated) await unlink('.debug-shortcut.html');
}
