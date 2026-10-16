import test from 'node:test';import assert from 'node:assert/strict';
import {Connection,normalizeServer,storageScope} from '../web/connection.mjs';
import {Backend} from '../web/client.mjs';
const initial=()=>({notebook_id:'one',revision:0,notes:[{id:'a',body:'old'}]});
test('server URL validation rejects credentials, paths and remote plaintext',()=>{
 for(const bad of ['http://notes.example.org','https://u:p@notes.example.org','https://notes.example.org/path','https://notes.example.org/?token=x','file:///tmp/notes'])assert.throws(()=>normalizeServer(bad));
 assert.equal(normalizeServer('https://notes.example.org/'),'https://notes.example.org');assert.equal(normalizeServer('http://127.0.0.1:4173'),'http://127.0.0.1:4173');
});
test('transport uses an in-memory bearer and never redirects or embeds credentials in URL',async()=>{
 const calls=[];const connection=new Connection('https://notes.example.org',async(url,options)=>{calls.push({url,options});return {ok:true,json:async()=>url.endsWith('/session')?{token:'ephemeral'}:{}};});
 await connection.login('synthetic-key');await connection.request('/api/notebook');assert.equal(calls[1].options.headers.Authorization,'Bearer ephemeral');assert.equal(calls[1].options.headers['X-Lattice-Client'],'1');assert.equal(calls[1].url,'https://notes.example.org/api/notebook');assert.equal(calls[1].options.redirect,'error');assert.equal(calls[1].options.cache,'no-store');await connection.logout();assert.equal(connection.token,null);
});
test('draft storage separates server and durable notebook identity',()=>{assert.notEqual(storageScope('https://a.test','one'),storageScope('https://b.test','one'));assert.notEqual(storageScope('https://a.test','one'),storageScope('https://a.test','two'));assert.throws(()=>storageScope('https://a.test',''));});
test('refresh updates clean clients but cannot discard an edit arriving during download',async()=>{
 let resolve;const gate=new Promise(r=>resolve=r);let loads=0;
 const backend=new Backend(async(path)=>{if(path==='/api/version')return {notebook_id:'one',revision:1};if(++loads===1)return initial();await gate;return {...initial(),revision:1,notes:[{id:'a',body:'remote'}]};});
 await backend.load();const refreshing=backend.refresh();await new Promise(r=>setImmediate(r));backend.edit('a','mine');resolve();await refreshing;assert.equal(backend.body('a'),'mine');assert.equal(backend.state.revision,0);
});
test('refresh and write carry notebook identity to protect database replacements',async()=>{
 const writes=[];const backend=new Backend(async(path,payload)=>{if(payload){writes.push(payload);return {notebook:{...initial(),revision:1}};}if(path==='/api/version')return {notebook_id:'replacement',revision:1};return initial();});
 await backend.load();backend.edit('a','mine');await backend.flush();assert.equal(writes[0].notebook_id,'one');await assert.rejects(backend.refresh());assert.equal(backend.blocked,true);
});
test('clean client receives remote edits',async()=>{let loads=0;const backend=new Backend(async path=>path==='/api/version'?{notebook_id:'one',revision:1}:++loads===1?initial():{...initial(),revision:1,notes:[{id:'a',body:'remote'}]});await backend.load();await backend.refresh();assert.equal(backend.body('a'),'remote');});
