import assert from 'node:assert/strict';
import process from 'node:process';
import console from 'node:console';
import { createRequire } from 'node:module';
import { writeFile, unlink } from 'node:fs/promises';
import { createServer } from 'vite';
const { chromium } = createRequire(import.meta.url)('playwright');
const server = await createServer({server:{host:'127.0.0.1',port:15328}});
let browser;
let fixtureCreated = false;
try {
  await server.listen();
  browser = await chromium.launch({channel:'chrome',headless:true});
  const page = await browser.newPage({viewport:{width:1100,height:900}});
  await writeFile('.debug-account.html', `<!doctype html><html><body><div id="root"></div><script type="module">
    import React, {useState} from 'react';
    import {createRoot} from 'react-dom/client';
    import {ConnectDialog} from '/src/accounts/ConnectDialog.tsx';
    import {LandingScreen} from '/src/auth/LandingScreen.tsx';
    import {createTauriBackend} from '/src/ipc/tauri.ts';
    import {mockIPC} from '@tauri-apps/api/mocks';
    import '/src/App.css';
    globalThis.attempts=[]; globalThis.cancellations=[]; globalThis.connected=0;
    let current=null; let finish=null;
    mockIPC(async (command,args)=>{
      if(command==='get_oauth_config') return {isConfigured:true,canSignIn:true,usingCustomOverride:false,clientId:null};
      if(command==='begin_account_connection') {
        if(globalThis.delayRegistration) await new Promise(resolve=>globalThis.releaseRegistration=resolve);
        if(current) throw new Error('Another account connection is already in progress');
        current=args.attemptId; globalThis.attempts.push(current); return;
      }
      if(command==='connect_account') {
        if(globalThis.failSignIn) {current=null; throw new Error('Google sign-in failed. Please retry.');}
        return new Promise(resolve=>{finish=resolve;globalThis.completeCurrent=()=>{current=null;resolve({id:'late-success'});};});
      }
      if(command==='cancel_account_connection') {
        if(globalThis.failCancellation) throw new Error('Could not cancel sign-in. Try again.');
        globalThis.cancellations.push(args.attemptId);
        if(current===args.attemptId) {current=null; if(globalThis.delayCompletion) globalThis.completeOld=finish; else finish?.({id:'late-success'}); finish=null;}
        return;
      }
      throw new Error('Unexpected command '+command);
    });
    const backend=createTauriBackend();
    function Harness() {
      const [show,setShow]=useState(false);
      const [landing,setLanding]=useState(false);
      globalThis.showLanding=()=>setLanding(true);
      globalThis.forceUnmount=()=>setShow(false);
      return landing ? React.createElement(LandingScreen,{backend,onAnnounce:()=>{},onConnected:()=>globalThis.connected++,onOpenLegal:()=>{}})
        : React.createElement(React.Fragment,null,
          React.createElement('button',{onClick:()=>setShow(true)},'Add account'),
          show && React.createElement(ConnectDialog,{backend,onClose:()=>setShow(false),onConnected:()=>globalThis.connected++,onAnnounce:()=>{}}));
    }
    createRoot(document.getElementById('root')).render(React.createElement(Harness));
  </script></body></html>`,{flag:'wx'});
  fixtureCreated=true;
  await page.goto('http://127.0.0.1:15328/.debug-account.html');
  for(const closeAction of ['Cancel','Close','Escape']) {
    await page.getByRole('button',{name:'Add account',exact:true}).click();
    await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
    await page.getByRole('button',{name:'Waiting for browser…',exact:true}).waitFor();
    assert.equal(await page.getByRole('button',{name:'Cancel',exact:true}).isEnabled(),true,'Pending sign-in must remain cancellable');
    if(process.env['GDOM_AUTH_SCREENSHOT']) await page.screenshot({path:process.env['GDOM_AUTH_SCREENSHOT'],fullPage:true});
    if(closeAction==='Escape') await page.keyboard.press('Escape');
    else await page.getByRole('button',{name:closeAction,exact:true}).click();
    await page.getByRole('dialog').waitFor({state:'hidden'});
  }
  assert.equal(await page.evaluate(()=>globalThis.cancellations.length),3);
  assert.equal(await page.evaluate(()=>globalThis.connected),0,'Cancelled late success must not reconnect or close a new dialog');
  await page.evaluate(()=>{globalThis.delayRegistration=true;});
  await page.getByRole('button',{name:'Add account',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.waitForFunction(()=>Boolean(globalThis.releaseRegistration));
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('button',{name:'Cancelling…',exact:true}).waitFor();
  await page.evaluate(()=>{globalThis.releaseRegistration();globalThis.delayRegistration=false;});
  await page.getByRole('dialog').waitFor({state:'hidden'});
  assert.equal(await page.evaluate(()=>globalThis.cancellations.length),4);
  await page.evaluate(()=>{globalThis.failSignIn=true;});
  await page.getByRole('button',{name:'Add account',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Google sign-in failed'}).waitFor();
  assert.equal(await page.getByRole('button',{name:'Sign in with Google',exact:true}).isEnabled(),true);
  await page.evaluate(()=>{globalThis.failSignIn=false;});
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('dialog').waitFor({state:'hidden'});
  await page.getByRole('button',{name:'Add account',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.evaluate(()=>{globalThis.failCancellation=true;});
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Could not cancel sign-in'}).waitFor();
  await page.evaluate(()=>{globalThis.failCancellation=false;});
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('dialog').waitFor({state:'hidden'});
  const cancellationCount = await page.evaluate(()=>globalThis.cancellations.length);
  await page.getByRole('button',{name:'Add account',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.evaluate(()=>globalThis.forceUnmount());
  await page.waitForFunction(count=>globalThis.cancellations.length > count,cancellationCount);
  await page.evaluate(()=>globalThis.showLanding());
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.evaluate(()=>{globalThis.delayCompletion=true;});
  await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).click();
  await page.evaluate(()=>{globalThis.completeOld({id:'old'});globalThis.delayCompletion=false;});
  assert.equal(await page.getByRole('button',{name:'Cancel sign-in',exact:true}).isEnabled(),true);
  assert.equal(await page.getByRole('button',{name:'Opening system browser…',exact:true}).isDisabled(),true);
  await page.evaluate(()=>{globalThis.failCancellation=true;});
  await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Could not cancel sign-in'}).waitFor();
  await page.evaluate(()=>globalThis.completeCurrent());
  assert.equal(await page.getByRole('button',{name:'Cancel sign-in',exact:true}).isEnabled(),true);
  await page.evaluate(()=>{globalThis.failCancellation=false;});
  await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();
  await page.getByRole('button',{name:'Sign in with Google',exact:true}).waitFor();
  assert.equal(await page.getByRole('button',{name:'Sign in with Google',exact:true}).isEnabled(),true);
  assert.equal(await page.evaluate(()=>globalThis.connected),0);
  console.log('PASS: Cancel/Close/Escape release attempt, immediate retry, delayed registration, late success suppression, sign-in failure retry, landing cancel');
} finally {
  await browser?.close();
  await server.close();
  if(fixtureCreated) await unlink('.debug-account.html');
}
