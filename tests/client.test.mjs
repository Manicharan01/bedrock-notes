import test from 'node:test';import assert from 'node:assert/strict';import {Backend,recoverDrafts,markdownFilename} from '../web/client.mjs';
const initial=()=>({revision:0,notes:[{id:'a',title:'Alpha',body:'old'}]});
const wait=()=>{let resolve;const promise=new Promise(r=>resolve=r);return {promise,resolve};};

test('Markdown export names work across native save dialogs',()=>{
  assert.equal(markdownFilename('Projects/Today: why?'),'Today- why-.md');
  assert.equal(markdownFilename('Archive\\CON.txt'),'_CON.txt.md');
  assert.equal(markdownFilename(' . '),'Untitled.md');
  assert.equal(markdownFilename('Ideas / తెలుగు ✨'),'తెలుగు ✨.md');
  assert.equal(markdownFilename('Line\nbreak.'),'Line-break.md');
});

test('a delayed save cannot discard a newer edit',async()=>{
  const pending=wait(),calls=[];const backend=new Backend(async(path,payload)=>{if(!payload)return initial();calls.push(payload);if(calls.length===1)await pending.promise;return {notebook:{revision:calls.length,notes:[{id:'a',title:'Alpha',body:payload.body}]}};});
  await backend.load();backend.edit('a','first');const saving=backend.flush();await Promise.resolve();backend.edit('a','newest');pending.resolve();await saving;
  assert.deepEqual(calls.map(c=>c.body),['first','newest']);assert.deepEqual(calls.map(c=>c.revision),[0,1]);assert.equal(backend.state.notes[0].body,'newest');assert.equal(backend.drafts.size,0);
});
test('revision conflicts preserve drafts and block further writes',async()=>{
  let writes=0;const backend=new Backend(async(path,payload)=>{if(!payload)return initial();writes++;throw Object.assign(new Error('conflict'),{code:'revision_conflict'});});await backend.load();backend.edit('a','unsaved');await assert.rejects(backend.flush());assert.equal(backend.drafts.get('a').body,'unsaved');assert.equal(backend.blocked,true);await assert.rejects(backend.flush());assert.equal(writes,1);
});
test('transport failure retains edits and allows a deliberate retry',async()=>{
  let fails=true;const backend=new Backend(async(path,payload)=>{if(!payload)return initial();if(fails)throw new Error('offline');return {notebook:{revision:1,notes:[{id:'a',title:'Alpha',body:payload.body}]}};});await backend.load();backend.edit('a','keep');await assert.rejects(backend.flush());assert.equal(backend.drafts.size,1);fails=false;await backend.flush();assert.equal(backend.state.notes[0].body,'keep');
});
test('rename waits for pending saves and uses the next revision',async()=>{
  const commands=[];const backend=new Backend(async(path,payload)=>{if(!payload)return initial();commands.push(payload);return {notebook:{revision:commands.length,notes:[{id:'a',title:payload.title||'Alpha',body:payload.body||'draft'}]},selected_id:'a'};});await backend.load();backend.edit('a','draft');await backend.mutate({kind:'rename',id:'a',title:'Beta'});assert.deepEqual(commands.map(c=>c.kind),['update','rename']);assert.deepEqual(commands.map(c=>c.revision),[0,1]);assert.equal(backend.state.notes[0].title,'Beta');
});
test('a newer draft base tracks the save already acknowledged by the server',async()=>{
  const gate=wait();let writes=0;let savedBase;const backend=new Backend(async(path,payload)=>{if(!payload)return initial();writes++;if(writes===1)await gate.promise;else{savedBase=backend.drafts.get('a').baseBody;throw new Error('offline');}return {notebook:{revision:1,notes:[{id:'a',title:'Alpha',body:payload.body}]}};});await backend.load();backend.edit('a','first');const saving=backend.flush();await Promise.resolve();backend.edit('a','newer');gate.resolve();await assert.rejects(saving);assert.equal(savedBase,'first');assert.equal(backend.drafts.get('a').body,'newer');
});

test('recovery restores a draft when the server still has its base content',async()=>{
  const backend=new Backend(async()=>initial());await backend.load();recoverDrafts(backend,JSON.stringify([['a',{body:'recovered',baseBody:'old'}]]));assert.equal(backend.body('a'),'recovered');assert.equal(backend.blocked,false);
});
test('recovery drops acknowledged drafts but preserves conflicting and orphaned drafts',async()=>{
  const backend=new Backend(async()=>initial());await backend.load();recoverDrafts(backend,JSON.stringify([['a',{body:'conflicting',baseBody:'outdated'}],['deleted',{body:'orphan',baseBody:'removed'}]]));assert.equal(backend.blocked,true);assert.equal(backend.drafts.size,2);
  const other=new Backend(async()=>initial());await other.load();recoverDrafts(other,JSON.stringify([['a',{body:'old',baseBody:'anything'}]]));assert.equal(other.drafts.size,0);
});
test('malformed recovery data is rejected before applying any draft',async()=>{
  const backend=new Backend(async()=>initial());await backend.load();assert.throws(()=>recoverDrafts(backend,JSON.stringify([['a',{body:'valid',baseBody:'old'}],['broken',null]])));assert.equal(backend.drafts.size,0);
});
