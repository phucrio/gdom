/* global window, document */
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import process from "node:process";
import console from "node:console";
import { createServer } from "vite";
const { chromium } = createRequire(import.meta.url)("playwright");
const output = join(tmpdir(), "gdom-job-controls-qa");
await mkdir(output, {recursive:true});
const server = await createServer({server:{host:"127.0.0.1",port:0},plugins:[{name:"jobs-qa",configureServer(vite){
  vite.middlewares.use("/__jobs-qa",async (_request,response)=>{response.setHeader("Content-Type","text/html");response.end(await vite.transformIndexHtml("/__jobs-qa",'<html lang="en"><head><title>Jobs QA</title></head><body><div id="root"></div><script type="module" src="/scripts/progress-ui-fixture.tsx"></script></body></html>'));});
}}]});
await server.listen();
const browser = await chromium.launch({headless:true,...(process.env.GDOM_QA_BROWSER ? {channel:process.env.GDOM_QA_BROWSER} : {})});
const page = await browser.newPage({viewport:{width:1280,height:900}});
page.setDefaultTimeout(30000);page.setDefaultNavigationTimeout(90000); const errors=[];page.on("pageerror",error=>errors.push(error.message));
try {
 await page.goto(`${server.resolvedUrls.local[0]}__jobs-qa`);
 await page.getByRole("link",{name:"Jobs",exact:true}).click();
 await page.evaluate(()=>window.progressQa.queueScenario());
 const cards=page.locator('.job-group').filter({has:page.getByRole('heading',{name:'Queued',exact:true})}).locator('.job-card');
 await cards.first().getByText('Queued fixture 1',{exact:false}).waitFor();
 assert.equal(await cards.first().getByRole('button',{name:'Move up',exact:true}).isDisabled(),true);
 assert.equal(await cards.last().getByRole('button',{name:'Move down',exact:true}).isDisabled(),true);
 await page.evaluate(()=>window.progressQa.holdQueue());
 await cards.first().getByRole('button',{name:'Move down',exact:true}).focus();await page.keyboard.press('Enter');
 await page.getByRole('status').filter({hasText:'Updating queue...'}).waitFor();
 for(const action of await cards.getByRole('button',{name:/Move up|Move down|Remove from queue/}).all()) assert.equal(await action.isDisabled(),true);
 await page.evaluate(()=>window.progressQa.releaseQueue());
 await cards.first().getByText('Queued fixture 2',{exact:false}).waitFor();
 await page.evaluate(()=>window.progressQa.failQueue(true));
 await cards.first().getByRole('button',{name:'Remove from queue',exact:true}).click();
 await page.getByRole('alert').filter({hasText:'Queue update failed'}).waitFor();
 assert.equal(await cards.count(),3);
 await page.evaluate(()=>window.progressQa.failQueue(false));
 for(const width of [375,768,1280]) {
  await page.setViewportSize({width,height:900});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  await page.screenshot({path:join(output,`queue-${width}.png`),fullPage:true});
 }
 await page.getByLabel('Filter by source or target account').selectOption('source');
 assert.equal(await page.getByRole('button',{name:'Move up',exact:true}).count(),0);
 await cards.first().getByRole('button',{name:'Remove from queue',exact:true}).click();
 await page.waitForFunction(()=>window.progressQa.commands.includes('remove:queue-2'));
 await page.getByLabel('Filter by source or target account').selectOption('');
 const completed=page.locator('.job-group').filter({has:page.getByRole('heading',{name:'Completed',exact:true})});
 await completed.getByRole('button',{name:'View details'}).click();
 const destination=page.getByLabel('Destination path');
 await destination.fill('report.json');await page.getByRole('button',{name:'Export final report',exact:true}).click();
 await page.getByRole('alert').filter({hasText:'Choose a .txt or .csv'}).waitFor();
 await destination.fill('report.csv');await page.evaluate(()=>window.progressQa.failExport(true));
 await page.getByRole('button',{name:'Export final report',exact:true}).click();
 await page.getByRole('alert').filter({hasText:'Destination is not writable'}).waitFor();
 assert.equal(await destination.inputValue(),'report.csv');
 await page.evaluate(()=>window.progressQa.failExport(false));
 for(const extension of ['csv','txt']) {await destination.fill(`report.${extension}`);await destination.press('Enter');await page.getByRole('dialog').getByRole('status').filter({hasText:`Saved report.${extension}`}).waitFor();}
 for(const width of [375,768,1280]) {await page.setViewportSize({width,height:900});await page.screenshot({path:join(output,`report-${width}.png`),fullPage:true});}
 await page.keyboard.press('Escape');assert.equal(await page.getByRole('dialog').count(),0);
 assert.equal(await completed.getByRole('button',{name:'View details'}).evaluate(element=>element===document.activeElement),true);
 assert.deepEqual(errors,[]);console.log(`PASS jobs queue/report controls: ${output}`);
} finally {await browser.close();await server.close();}
