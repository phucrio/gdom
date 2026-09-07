import console from 'node:console';
import process from 'node:process';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { createServer } from 'vite';
import { readFile, writeFile, unlink } from 'node:fs/promises';

// Run with node; NODE_PATH can point to an existing Playwright installation.
const { chromium } = createRequire(import.meta.url)('playwright');
const server = await createServer({ server: { host: '127.0.0.1', port: 15327 } });
const configuration = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'));
const imagePolicy = configuration.app.security.csp.split(';').find(directive => directive.trim().startsWith('img-src'));
assert.ok(imagePolicy);
let browser;
let fixtureCreated = false;
try {
  await server.listen();
  browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  await page.route('**/owner-avatar.svg', route => route.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28"><rect width="28" height="28" fill="#6366f1"/><circle cx="14" cy="10" r="5" fill="white"/><path d="M4 28a10 10 0 0 1 20 0" fill="white"/></svg>'}));
  await page.route('**/missing-avatar.png', route => route.fulfill({status:404,body:''}));
  const errors = [];
  page.on('pageerror', error => { errors.push(error.message); console.error(error.message); });
  await writeFile('.debug-shortcut.html', `<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="${imagePolicy}"></head><body><div id="root"></div><script type="module">
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
        globalThis.openedItems.push(args.input);
      });
      const account = { id:'source', email:'source@example.test', googlePermissionId:'source', authStatus:'CONNECTED', displayName:'Test account' };
      const base = { mimeType:'application/vnd.google-apps.shortcut', isFolder:false, size:0, modifiedTime:null,
        owners:[{permissionId:'source',emailAddress:'source@example.test'}], isOwner:true,
        canTransferOwnership:false, webViewLink:null };
      const items = [
        {...base,id:'shortcut-course',name:'Courses',owners:[{permissionId:'source',emailAddress:'source@example.test',avatarUrl:'https://lh3.googleusercontent.com/owner-avatar.svg'}],folderId:'target-course',folderResourceKey:'course-key',resourceKey:'shortcut-key',shortcutTargetId:'target-course'},
        {...base,id:'shortcut-study',name:'Huyền học',folderId:'target-study',folderResourceKey:'study-key',shortcutTargetId:'target-study'},
        {...base,id:'folder',name:'Ordinary folder',mimeType:'application/vnd.google-apps.folder',isFolder:true,folderId:'folder',folderResourceKey:'native-key',shortcutTargetId:null},
        {...base,id:'file-shortcut',name:'Document shortcut',isOwner:false,owners:[{permissionId:'other',emailAddress:'alexandra.long.owner@example.test',avatarUrl:'/missing-avatar.png'}],folderId:null,shortcutTargetId:'document'},
        {...base,id:'blocked-shortcut',name:'Unavailable folder',folderId:'blocked',shortcutTargetId:'blocked'},
      ];
      globalThis.requests = [];
      const backend = {...createTauriBackend(), listDriveFiles: async input => {
        globalThis.requests.push(input);
        if(input.folderId === 'blocked') throw new Error('Access denied to shared folder');
        if(input.pageToken) return {items:[{...base,id:'more',name:'More document',folderId:null,shortcutTargetId:null}],nextPageToken:null};
        return {items: input.folderId ? [{...base,id:'child',name:'Nested document',folderId:null,shortcutTargetId:null}, {...base,id:'self',name:'This folder',folderId:input.folderId,shortcutTargetId:input.folderId}, {...base,id:'ancestor',name:'Back to root',folderId:'root',shortcutTargetId:'root'}, {...base,id:'nested',name:'Nested folder',isFolder:true,folderId:'nested',folderResourceKey:'nested-key',shortcutTargetId:null}] : items,nextPageToken:input.folderId==='target-course'?'more':null};
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
  const ownRow = page.getByRole('row').filter({has:page.getByRole('button',{name:'Courses',exact:true})});
  await page.waitForFunction(() => globalThis.document.querySelector('.owner-cell img')?.naturalWidth > 0, undefined, {timeout:5000});
  assert.equal(await ownRow.locator('.owner-name').innerText(), 'Me');
  const sharedOwner = page.getByRole('row').filter({hasText:'Document shortcut'}).locator('.owner-cell');
  await sharedOwner.locator('img').waitFor({state:'detached'});
  assert.equal(await sharedOwner.locator('.avatar-circle').innerText(), 'AL');
  assert.equal(await sharedOwner.locator('.owner-name').innerText(), 'alexandra.long.owner@example.test');
  if (process.env['GDOM_QA_SCREENSHOT']) await page.screenshot({ path: process.env['GDOM_QA_SCREENSHOT'], fullPage: true });
  async function verifyOpened(folderId) {
    await page.getByText('Nested document', { exact:true }).waitFor();
    const request = await page.evaluate(() => globalThis.requests.at(-1));
    assert.equal(request.folderId, folderId);
    assert.equal(request.accountId, 'source');
    assert.equal(request.folderResourceKey, { 'target-course':'course-key','target-study':'study-key',folder:'native-key' }[folderId]);
    await page.getByRole('button',{name:'Refresh files',exact:true}).click();
    await page.getByText('Nested document',{exact:true}).waitFor();
    const refreshed = await page.evaluate(()=>globalThis.requests.at(-1));
    assert.equal(refreshed.folderResourceKey, request.folderResourceKey);
    if(folderId === 'target-course') {
      await page.getByRole('button',{name:'Load more files',exact:true}).click();
      await page.getByText('More document',{exact:true}).waitFor();
      const paged = await page.evaluate(()=>globalThis.requests.at(-1));
      assert.equal(paged.pageToken,'more');
      assert.equal(paged.folderResourceKey,'course-key');
      await page.getByRole('button',{name:'Nested folder',exact:true}).click();
      await page.waitForFunction(()=>globalThis.requests.at(-1).folderId==='nested');
      await page.getByRole('button',{name:'Courses',exact:true}).click();
      await page.getByText('Nested document',{exact:true}).waitFor();
      const returned = await page.evaluate(()=>globalThis.requests.at(-1));
      assert.equal(returned.folderId,'target-course');
      assert.equal(returned.folderResourceKey,'course-key');
    }
    await page.getByRole('button', { name:'This folder', exact:true }).click();
    assert.equal(await page.locator('.breadcrumb-item').count(), 2);
    await page.getByRole('button', {name:'Back to root',exact:true}).click();
    await page.getByText('Document shortcut', {exact:true}).waitFor();
    assert.equal((await page.evaluate(()=>globalThis.requests.at(-1))).folderResourceKey,null);
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
  assert.deepEqual(await page.evaluate(() => globalThis.openedItems), [{accountId:'source',fileId:'file-shortcut',resourceKey:null}]);
  await page.getByRole('button', {name:'Action menu for Courses',exact:true}).click();
  await page.getByRole('menuitem', {name:'Open in Google Drive',exact:true}).click();
  await page.waitForFunction(() => globalThis.openedItems.length === 2);
  assert.deepEqual(await page.evaluate(() => globalThis.openedItems), [{accountId:'source',fileId:'file-shortcut',resourceKey:null},{accountId:'source',fileId:'shortcut-course',resourceKey:'shortcut-key'}]);
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
