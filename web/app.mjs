import {Backend,recoverDrafts,markdownFilename} from './client.mjs';
import {Connection,normalizeServer,storageScope} from './connection.mjs';
const $=s=>document.querySelector(s), $$=s=>[...document.querySelectorAll(s)];
const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
let UI_KEY, DRAFT_KEY, connection;
let activeId=null,openTabs=[],mode='read',tag='',graph=false,index={backlinks:new Map()},saveTimer,toastTimer,searchSerial=0,lastError=null,connectionError=false,booting=true;
const banner=document.createElement('div');banner.id='backend-banner';banner.hidden=true;$('#note-view').before(banner);
const backend=new Backend(request,()=>{if(booting)return;lastError=null;saveDrafts();render();},error=>{lastError=error;saveDrafts();updateStatus();toast(error.message);});

async function request(path,payload) {
  try{return await connection.request(path,payload);}catch(error){if(error.status===401&&!booting){$('#connect-hint').textContent='Your session expired. Drafts are kept. Enter the access key to reconnect.';showConnection();}throw error;}
}
function showConnection(){if(!$('#connection-dialog').open)$('#connection-dialog').showModal();}
async function connect(){
  const native=!!window.__TAURI_INTERNALS__;
  let saved,choose=false;try{saved=localStorage.getItem('lattice.server');choose=sessionStorage.getItem('lattice.choose')==='1';sessionStorage.removeItem('lattice.choose');}catch{}
  const defaultServer=native?'':location.origin;
  $('#server-url').value=saved||defaultServer;
  $('#connection-dialog').addEventListener('cancel',e=>e.preventDefault());
  async function tryServer(base,key){
    const next=new Connection(base);
    if(!booting&&next.base!==connection.base)throw new Error('Use Switch server in Settings to change notebooks safely.');
    const info=await next.request('/api/info');
    if(info.authRequired){try{await next.request('/api/session');}catch(error){if(error.status!==401)throw error;await next.login(key);}}
    connection=next;
    try{localStorage.setItem('lattice.server',next.base);}catch{}
    $('#server-key').value='';$('#connection-dialog').close();
    return next;
  }
  $('#connection-form').onsubmit=async e=>{
    e.preventDefault();$('#connect-submit').disabled=true;
    try{await tryServer(normalizeServer($('#server-url').value.trim()),$('#server-key').value);$('#connection-dialog').dispatchEvent(new Event('connected'));if(!booting){if(backend.drafts.size&&!backend.blocked)caught(backend.flush());else caught(refresh());}}
    catch(error){$('#connect-hint').textContent=error.message;}
    finally{$('#connect-submit').disabled=false;}
  };
  if(!choose&&(saved||defaultServer)){try{return await tryServer(saved||defaultServer,'');}catch(error){$('#connect-hint').textContent=error.status===401?'Enter your server access key.':error.message;}}
  showConnection();
  return new Promise(resolve=>$('#connection-dialog').addEventListener('connected',()=>resolve(connection),{once:true}));
}
function notes(){return backend.state?.notes||[];}
function active(){return notes().find(n=>n.id===activeId);}
function meta(id=activeId){return backend.state?.metadata[id]||{links:[],tags:[],headings:[],html:''};}
function body(id=activeId){return backend.body(id);}
function toast(message){$('#toast').textContent=message;$('#toast').classList.add('show');clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').classList.remove('show'),5000);}
function caught(promise){promise.catch(error=>toast(error.message||'Operation failed.'));}
function saveUI(){try{UI_KEY&&localStorage.setItem(UI_KEY,JSON.stringify({activeId,openTabs,mode}));}catch{/* UI preferences are optional. */}}
function saveDrafts(){if(!DRAFT_KEY)return;try{if(backend.drafts.size)localStorage.setItem(DRAFT_KEY,JSON.stringify([...backend.drafts]));else localStorage.removeItem(DRAFT_KEY);}catch{if(backend.drafts.size)toast('Browser recovery storage is unavailable. Keep this tab open until Saved to SQLite appears.');}}
function updateStatus(){
  const dirty=backend.drafts.size>0;
  $('#save-status').innerHTML=backend.blocked?'Conflict · drafts kept':connectionError?'Server unavailable · drafts kept':lastError?'Save failed · draft kept':dirty?'Saving to SQLite…':'<i></i> Saved to SQLite';
  banner.hidden=!backend.blocked;
  if(backend.blocked){banner.innerHTML='<span>The notebook changed. Your local drafts are preserved.</span><button data-action="backup">Download drafts</button><button data-action="reload">Reload latest</button>';}
}
function selectNote(id,anchor='') {
  if(!notes().some(n=>n.id===id))return;
  activeId=id;graph=false;if(!openTabs.includes(id))openTabs.push(id);
  setSidebar(false);saveUI();render();
  if(anchor){const h=meta(id).headings.find(h=>h.text.toLowerCase()===anchor.toLowerCase()||h.id===anchor);if(h)requestAnimationFrame(()=>document.getElementById(h.id)?.scrollIntoView({behavior:'smooth',block:'start'}));}
}
function setSidebar(open){document.body.classList.toggle('sidebar-open',open);const toggle=$('[data-action="toggle-sidebar"]');toggle.setAttribute('aria-expanded',String(open));}
function setInspector(open){document.body.classList.toggle('show-inspector',open);const toggle=$('[data-action="toggle-inspector"][aria-expanded]');toggle?.setAttribute('aria-expanded',String(open));toggle?.setAttribute('aria-label',open?'Hide connections':'Show connections');}
function setMode(value){mode=value;saveUI();render();}
function renderTabs(){$('#tabs').innerHTML=openTabs.map(id=>{const n=notes().find(n=>n.id===id);return n?'<div class="tab '+(id===activeId&&!graph?'current':'')+'"><button data-open="'+esc(id)+'">'+esc(n.title)+'</button><button class="close" data-close="'+esc(id)+'" aria-label="Close '+esc(n.title)+' tab">×</button></div>':'';}).join('');}
async function renderList(){
  const sequence=++searchSerial,revision=backend.state.revision,q=$('#search').value;
  let found=notes().slice().sort((a,b)=>b.updated-a.updated||a.id.localeCompare(b.id));
  if(q||tag){try{const result=await request('/api/search?'+new URLSearchParams({q,tag}));if(sequence!==searchSerial||revision!==backend.state.revision)return;if(result.revision!==revision){toast('Notebook changed in another session. Reload to see current search results.');return;}found=result.ids.map(id=>notes().find(n=>n.id===id)).filter(Boolean);}catch(e){if(sequence===searchSerial)toast(e.message);return;}}
  if(sequence!==searchSerial)return;
  $('#note-count').textContent=notes().length;
  $('#list-label').textContent=q?'SEARCH RESULTS':tag?'#'+tag.toUpperCase():'ALL NOTES';
  $('#note-list').innerHTML=found.length?found.map((n,i)=>'<button class="note-item '+(n.id===activeId?'active':'')+'" data-open="'+esc(n.id)+'"><span class="note-number">'+String(i+1).padStart(2,'0')+'</span><span class="item-title">'+esc(n.title)+'</span><span class="note-excerpt">'+esc(n.body.replace(/[#[\]*_`>]/g,'').replace(/\s+/g,' ').trim().slice(0,64)||'A fresh page, ready for a thought.')+'</span></button>').join(''):'<div class="empty">No notes here yet. Start with one thought.</div>';
  const tags=[...new Set(notes().flatMap(n=>meta(n.id).tags))].sort();
  $('#tag-list').innerHTML=tags.map(t=>'<button class="tag-chip '+(tag===t?'active':'')+'" data-tag="'+esc(t)+'">#'+esc(t)+'</button>').join('');$('#clear-tag').hidden=!tag;
}
function renderPreview(){const n=active();$('#preview').innerHTML=n?meta().html:'<p>Create a new note to begin.</p>';const text=n?body():'';$('#word-count').textContent=(text.trim()?text.trim().split(/\s+/).length:0)+' words';$('#char-count').textContent=text.length+' characters';$('#note-tags').textContent=n?meta().tags.map(t=>'#'+t).join('  '):'';}
function renderConnections(){
  const n=active(),incoming=n?(backend.state.backlinks[n.id]||[]):[],links=n?meta().links:[];
  const outgoing=[...new Map(links.map(l=>[l.target+'#'+l.anchor,l])).values()];
  $('#backlink-count').textContent=incoming.length;$('#outgoing-count').textContent=outgoing.length;
  $('#backlinks').innerHTML=incoming.length?incoming.map(id=>{const other=notes().find(n=>n.id===id);return '<button class="connection" data-open="'+esc(id)+'"><span>↗ '+esc(other.title)+'</span><small>'+esc(other.body.split('\n').find(l=>l.includes('[['))||other.body.slice(0,90))+'</small></button>';}).join(''):'<div class="empty">Link here from another note to create a backlink.</div>';
  $('#outgoing').innerHTML=outgoing.map(l=>'<button class="outgoing-link" data-target="'+esc(l.target)+'" data-link-id="'+esc(l.note_id||'')+'" data-anchor="'+esc(l.anchor)+'">↗ '+esc(l.label||l.target)+(l.note_id?'':' ＋')+'</button>').join('')||'<div class="empty">Use [[Note title]] to connect a note.</div>';
  $('#outline').innerHTML=(n?meta().headings:[]).map(h=>'<button class="outline-item '+(h.level>1?'sub':'')+'" data-heading="'+esc(h.id)+'" data-offset="'+h.index+'">'+esc(h.text)+'</button>').join('');
  drawGraph($('#mini-graph'),true);
}
function render(){
  if(!backend.state)return;
  openTabs=openTabs.filter(id=>notes().some(n=>n.id===id));
  if(!active())activeId=notes()[0]?.id||null;if(activeId&&!openTabs.includes(activeId))openTabs.push(activeId);
  index.backlinks=new Map(Object.entries(backend.state.backlinks));
  renderTabs();caught(renderList());$('#graph-view').hidden=!graph;$('#note-view').hidden=graph;
  const n=active();$('#note-code').textContent='BR / '+String(n?notes().indexOf(n)+1:0).padStart(2,'0');$('#breadcrumb').innerHTML='<span>FIELD NOTES /</span> '+esc(graph?'Connection atlas':n?.title||'No notes');
  $('#title').disabled=!n||backend.commandBusy||backend.blocked;$('#editor').disabled=!n||backend.commandBusy||backend.blocked;
  if(document.activeElement!==$('#title'))$('#title').value=n?.title||'Your next idea starts here';
  const value=n?body():'';if($('#editor').value!==value)$('#editor').value=value;
  $('#modified').textContent=n?'Edited '+new Date(n.updated).toLocaleDateString(undefined,{month:'short',day:'numeric'}):'';
  $('#writing-area').className='writing-area mode-'+mode;$$('[data-mode]').forEach(b=>{b.classList.toggle('active',b.dataset.mode===mode);b.setAttribute('aria-pressed',String(b.dataset.mode===mode));});
  document.title=(n?n.title+' · ':'')+'Bedrock';
  renderPreview();renderConnections();if(graph)drawGraph($('#full-graph'),false);updateStatus();
  $$('.rail-top button').forEach(b=>b.classList.toggle('active',b.dataset.action===(graph?'graph':'notes')));
}
function graphData(_notes,_index,current,local){const edges=backend.state.edges;const connected=new Set([current]);for(const e of edges)if(e.source===current||e.target===current){connected.add(e.source);connected.add(e.target);}return {nodes:notes().filter(n=>!local||connected.has(n.id)),edges:edges.filter(e=>!local||(connected.has(e.source)&&connected.has(e.target)))};}
/* Graph layout is a presentation concern; the Rust API supplies authoritative edges. */
function drawGraph(container,local){
  const data=graphData(notes(),index,activeId,local),width=local?230:800,height=local?176:500,points=new Map(),count=data.nodes.length;
  if(count>120){container.innerHTML='<div class="empty">Graph display is limited to 120 notes. Use the local graph or search.</div>';return;}
  data.nodes.forEach((n,i)=>{const a=i*Math.PI*2/Math.max(1,count)-Math.PI/2;points.set(n.id,{x:local&&n.id===activeId?width/2:width/2+Math.cos(a)*width*.34,y:local&&n.id===activeId?height/2:height/2+Math.sin(a)*height*.32});});
  const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg');svg.setAttribute('viewBox',`0 0 ${width} ${height}`);svg.setAttribute('class','graph-svg');svg.setAttribute('aria-label',local?'Connections to the current note':'Graph of note links');
  const lines=data.edges.map(edge=>{const line=document.createElementNS(ns,'line');svg.append(line);return {line,edge};});const groups=[];
  for(const note of data.nodes){const g=document.createElementNS(ns,'g'),circle=document.createElementNS(ns,'rect'),label=document.createElementNS(ns,'text');g.setAttribute('role','button');g.setAttribute('tabindex','0');g.setAttribute('aria-label','Open '+note.title);const size=local?(note.id===activeId?15:10):Math.min(26,16+(index.backlinks.get(note.id)?.length||0));circle.classList.add('node-block');circle.setAttribute('width',size);circle.setAttribute('height',size);circle.setAttribute('x',-size/2);circle.setAttribute('y',-size/2);circle.setAttribute('rx','2');if(note.id===activeId)circle.classList.add('current-node');label.setAttribute('class','node-label');label.setAttribute('text-anchor','middle');label.setAttribute('y',local?18:27);label.textContent=note.title.length>(local?17:28)?note.title.slice(0,local?15:26)+'…':note.title;if(!local&&!$('#graph-labels').checked)label.style.display='none';g.append(circle,label);svg.append(g);groups.push({g,note});let drag=null,moved=false;
    g.addEventListener('pointerdown',e=>{drag={x:e.clientX,y:e.clientY};moved=false;g.setPointerCapture(e.pointerId);});g.addEventListener('pointermove',e=>{if(!drag)return;if(Math.abs(e.clientX-drag.x)+Math.abs(e.clientY-drag.y)>4)moved=true;if(!moved)return;const p=new DOMPoint(e.clientX,e.clientY).matrixTransform(svg.getScreenCTM().inverse());points.set(note.id,{x:Math.max(18,Math.min(width-18,p.x)),y:Math.max(18,Math.min(height-18,p.y))});update();});g.addEventListener('pointerup',()=>{drag=null;});g.addEventListener('pointercancel',()=>{drag=null;});g.addEventListener('click',()=>{if(!moved)selectNote(note.id);});g.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();selectNote(note.id);}});
  }
  function update(){for(const {g,note}of groups){const p=points.get(note.id);g.setAttribute('transform',`translate(${p.x},${p.y})`);}for(const {line,edge}of lines){const a=points.get(edge.source),b=points.get(edge.target);line.setAttribute('x1',a.x);line.setAttribute('y1',a.y);line.setAttribute('x2',b.x);line.setAttribute('y2',b.y);}}
  update();container.replaceChildren(svg);if(!local)$('#graph-count').textContent=data.nodes.length+' notes · '+data.edges.length+' connections';
}
function dialog(content,submit='Continue'){return new Promise(resolve=>{if($('#dialog').open){resolve(null);return;}$('#dialog-content').innerHTML=content;$('#dialog-submit').hidden=!submit;$('#dialog-submit').textContent=submit||'';$('#dialog-cancel').textContent=submit?'Cancel':'Close';$('#dialog').showModal();const done=value=>{$('#dialog').close();$('#dialog-form').onsubmit=null;$('#dialog-cancel').onclick=null;$('#dialog').oncancel=null;resolve(value);};$('#dialog-form').onsubmit=e=>{e.preventDefault();done($('#dialog-value')?.value??true);};$('#dialog-cancel').onclick=()=>done(null);$('#dialog').oncancel=e=>{e.preventDefault();done(null);};$('#dialog-value')?.focus();});}
async function createNote(title,initialBody=''){
  if(!title)title=await dialog('<h2>Start on solid ground.</h2><p>Give this thought a name.</p><input id="dialog-value" type="text" aria-label="New note title" maxlength="180" required>','Create note');if(!title)return;
  const id=await backend.mutate({kind:'create',title,body:initialBody});selectNote(id);setMode('edit');$('#editor').focus();
}
async function follow(target,id,anchor=''){if(id){selectNote(id,anchor);return;}if(await dialog('<h2>Make a connection</h2><p>Create <strong>'+esc(target)+'</strong>?</p>','Create note'))await createNote(target);}
async function download(name,data,type='text/plain'){
  if(window.__TAURI__){
    const path=await window.__TAURI__.dialog.save({defaultPath:name});
    if(!path)return false;
    await window.__TAURI__.fs.writeTextFile(path,data);
  }else{
    const url=URL.createObjectURL(new Blob([data],{type})),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),10000);
  }
  return true;
}
async function backup(){const all=notes().map(n=>({...n,body:body(n.id)}));for(const [id,draft]of backend.drafts)if(!all.some(n=>n.id===id))all.push({id,title:'Recovered draft '+id.slice(0,8),body:draft.body,updated:Date.now()});if(await download('Bedrock-backup-'+new Date().toISOString().slice(0,10)+'.json',JSON.stringify({format:'lattice-backup-v1',notes:all},null,2),'application/json'))toast('Saved a backup, including any unsaved drafts.');}
async function exportNote(){const n=active();if(n)await download(markdownFilename(n.title),body(),'text/markdown;charset=utf-8');}
async function chooseImport(backupFile=false){
  if(!window.__TAURI__){if(backupFile){const input=document.createElement('input');input.type='file';input.accept='.json';input.onchange=()=>{if(input.files[0])caught(restore(input.files[0]).catch(e=>{toast(e.message);throw e;}));};input.click();}else $('#import-input').click();return;}
  const picked=await window.__TAURI__.dialog.open({multiple:!backupFile,filters:[{name:backupFile?'Notebook backup':'Markdown',extensions:backupFile?['json']:['md','markdown']}]});
  if(!picked)return;
  const paths=Array.isArray(picked)?picked:[picked],files=[];
  for(const path of paths){const text=await window.__TAURI__.fs.readTextFile(path);files.push({name:typeof path==='string'?decodeURIComponent(path.split('/').at(-1).split('\\').at(-1)):'Note.md',size:new TextEncoder().encode(text).length,text:async()=>text});}
  // Android content URIs need the display name from the file picker. Parse titles from imported content when no filename is available.
  for(const file of files)if(!/\.(md|markdown|json)$/i.test(file.name))file.name=((await file.text()).match(/^#\s+(.+)$/m)?.[1]?.replace(/[<>:"\\|?*#\[\]]/g,'').slice(0,160)||'Imported note '+(files.indexOf(file)+1))+'.md';
  if(backupFile)await restore(files[0]);else await importFiles(files);
}
async function importEntries(entries){const id=await backend.mutate({kind:'import',notes:entries.map(n=>({title:n.title,body:n.body}))});if(id)selectNote(id);toast('Imported notes and updated links to any renamed imported notes.');}
async function importFiles(files){const entries=[];for(const file of files){if(!/\.(md|markdown)$/i.test(file.name)||file.size>2_000_000)throw new Error('Import requires Markdown files no larger than 2 MB each.');entries.push({title:file.name.replace(/\.(md|markdown)$/i,''),body:await file.text()});}if(entries.length)await importEntries(entries);}
async function restore(file){if(file.size>50_000_000)throw new Error('Backup file exceeds the request limit.');const parsed=JSON.parse(await file.text());if(parsed.format!=='lattice-backup-v1'||!Array.isArray(parsed.notes))throw new Error('Unsupported backup file');await importEntries(parsed.notes);}
async function shortcuts(){await dialog('<h2>Keyboard shortcuts</h2><dl class="shortcut-list"><dt><kbd>Ctrl / ⌘ K</kbd></dt><dd>Find a note</dd><dt><kbd>Ctrl / ⌘ N</kbd></dt><dd>Create a note</dd><dt><kbd>Ctrl / ⌘ S</kbd></dt><dd>Save pending changes</dd><dt><kbd>Escape</kbd></dt><dd>Close a panel</dd><dt><kbd>?</kbd></dt><dd>Show this help outside the editor</dd></dl>',null);}
async function settings(){
  const pending=dialog('<h2>Your self-hosted notebook.</h2><p>Confirmed notes are stored in SQLite on your server. This device keeps UI settings and unsaved drafts. A connection is required to load and save notes.</p><p>Connected to: '+esc(connection.base)+'</p><p>Imports merge notes; colliding names get a suffix and links between imported notes follow those names.</p><div class="storage-actions"><button type="button" data-storage="shortcuts">Keyboard shortcuts</button><button type="button" data-storage="server">Switch server / sign out</button><button type="button" data-storage="import">↥ Import Markdown files</button><button type="button" data-storage="backup">↧ Download notebook backup</button><button type="button" data-storage="restore">↥ Restore a backup</button><button type="button" data-storage="old">↥ Import the old browser notebook</button><button type="button" data-storage="markdown">↧ Export current note (.md)</button><button type="button" data-storage="connections">⌘ Show / hide connections</button></div>',null);
  $$('[data-storage]').forEach(b=>b.onclick=()=>{const action=b.dataset.storage;if(action==='shortcuts'){$('#dialog-cancel').click();caught(shortcuts());return;}if(action==='server'){caught((async()=>{saveDrafts();await connection.logout().catch(()=>{});try{localStorage.removeItem('lattice.server');sessionStorage.setItem('lattice.choose','1');}catch{}location.reload();})());return;}if(action==='import')caught(chooseImport());if(action==='backup')caught(backup());if(action==='markdown')caught(exportNote());if(action==='connections')setInspector(!document.body.classList.contains('show-inspector'));if(action==='restore')caught(chooseImport(true));if(action==='old')caught((async()=>{const saved=localStorage.getItem('lattice.v1');if(!saved)throw new Error('No old notebook on this origin. Restore a JSON backup exported from the original app.');const old=JSON.parse(saved);if(!Array.isArray(old.notes))throw new Error('Old notebook is invalid');await importEntries(old.notes);})().catch(e=>{toast(e.message);throw e;}));});await pending;
}
async function noteActions(){const n=active();if(!n)return;const pending=dialog('<h2>'+esc(n.title)+'</h2><div class="storage-actions"><button type="button" data-note-action="rename">✎ Rename note and update resolved links</button><button type="button" data-note-action="export">↧ Export Markdown</button><button type="button" data-note-action="delete">× Delete note</button></div>',null);$$('[data-note-action]').forEach(b=>b.onclick=()=>{const action=b.dataset.noteAction;if(action==='export'){caught(exportNote());return;}$('#dialog-cancel').click();if(action==='rename'){$('#title').focus();$('#title').select();}if(action==='delete')caught((async()=>{if(await dialog('<h2>Delete this note?</h2><p>'+esc(n.title)+' will be removed from the database. Export it first if you need a copy.</p>','Delete note'))await backend.mutate({kind:'delete',id:n.id});})());});await pending;}
async function reloadLatest(){if(backend.drafts.size&&!(await dialog('<h2>Reload saved notes?</h2><p>Your unsaved drafts will be discarded. Download a backup first if you need them.</p>','Discard drafts and reload')))return;backend.drafts.clear();saveDrafts();location.reload();}
const actions={home:()=>{if(notes()[0])selectNote(notes()[0].id);},notes:()=>{graph=false;render();},search:()=>{setSidebar(true);$('#search').focus();},new:()=>createNote(),graph:()=>{graph=true;setSidebar(false);setInspector(false);render();},daily:async()=>{const d=new Date(),date=[d.getFullYear(),String(d.getMonth()+1).padStart(2,'0'),String(d.getDate()).padStart(2,'0')].join('-'),found=notes().find(n=>n.title===date);if(found)selectNote(found.id);else await createNote(date,'# '+date+'\n\n## What I noticed\n\n## One next step\n\n- [ ] \n\n#daily');},settings,more:noteActions,backup,reload:reloadLatest,'back-to-note':()=>{graph=false;render();},'toggle-sidebar':()=>setSidebar(!document.body.classList.contains('sidebar-open')),'toggle-inspector':()=>setInspector(!document.body.classList.contains('show-inspector'))};
document.addEventListener('click',e=>{
  if(document.body.classList.contains('sidebar-open')&&!e.target.closest('.sidebar,[data-action="toggle-sidebar"]'))setSidebar(false);
  if(!backend.state)return;const b=e.target.closest('button');
  if(b?.dataset.action){const outcome=actions[b.dataset.action]?.();if(outcome?.then)caught(outcome);}
  if(b?.dataset.mode)setMode(b.dataset.mode);if(b?.dataset.open)selectNote(b.dataset.open);
  if(b?.dataset.close){openTabs=openTabs.filter(id=>id!==b.dataset.close);if(activeId===b.dataset.close)activeId=openTabs.at(-1)||notes()[0]?.id;saveUI();render();}
  if(b?.dataset.tag){tag=tag===b.dataset.tag?'':b.dataset.tag;caught(renderList());}
  if(b?.dataset.target!==undefined)caught(follow(b.dataset.target,b.dataset.linkId,b.dataset.anchor));
  if(b?.dataset.heading){if(mode==='edit'){$('#editor').focus();$('#editor').setSelectionRange(Number(b.dataset.offset),Number(b.dataset.offset));}else document.getElementById(b.dataset.heading)?.scrollIntoView({behavior:'smooth',block:'start'});}
  const link=e.target.closest('.wiki-link');if(link){e.preventDefault();caught(follow(link.dataset.note,link.dataset.id,link.dataset.anchor));}
});
$('#editor').addEventListener('input',()=>{
  if(!active())return;backend.edit(activeId,$('#editor').value);saveDrafts();updateStatus();
  clearTimeout(saveTimer);saveTimer=setTimeout(()=>caught(backend.flush()),250);showSuggestions();
});
function showSuggestions(){let popup=$('#suggestions');popup?.remove();const editor=$('#editor'),match=editor.value.slice(0,editor.selectionStart).match(/\[\[([^\]\n]{0,60})$/);if(!match)return;const matches=notes().filter(n=>n.title.toLowerCase().includes(match[1].toLowerCase())).slice(0,6);if(!matches.length)return;popup=document.createElement('div');popup.id='suggestions';popup.className='suggestions';for(const n of matches){const b=document.createElement('button');b.textContent='↗ '+n.title;b.onmousedown=e=>e.preventDefault();b.onclick=()=>{const end=editor.selectionStart;editor.setRangeText('[['+n.title+']]',end-match[0].length,end,'end');editor.focus();popup.remove();editor.dispatchEvent(new Event('input'));};popup.append(b);}$('.editor-pane').append(popup);}
$('#editor').addEventListener('blur',()=>setTimeout(()=>$('#suggestions')?.remove(),150));
$('#editor').addEventListener('keydown',e=>{if(e.key==='Tab'){e.preventDefault();e.target.setRangeText('  ',e.target.selectionStart,e.target.selectionEnd,'end');e.target.dispatchEvent(new Event('input'));}if(e.key==='Escape')$('#suggestions')?.remove();});
$('#title').addEventListener('blur',()=>{const n=active();if(!n)return;const title=$('#title').value;if(title===n.title)return;const id=n.id;caught(backend.mutate({kind:'rename',id,title}).then(()=>{if(activeId===id)$('#title').value=active().title;toast('Renamed note and updated resolved links.');}).catch(e=>{if(activeId===id)$('#title').value=active()?.title||'';throw e;}));});
$('#title').addEventListener('keydown',e=>{if(e.key==='Enter'){e.preventDefault();$('#title').blur();}if(e.key==='Escape'){$('#title').value=active()?.title||'';$('#title').blur();}});
$('#preview').addEventListener('change',e=>{if(!e.target.matches('input[data-line]')||!active())return;const lines=body().split('\n'),line=Number(e.target.dataset.line);if(!lines[line])return;lines[line]=lines[line].replace(/\[([ xX])\]/,e.target.checked?'[x]':'[ ]');backend.edit(activeId,lines.join('\n'));saveDrafts();$('#editor').value=body();caught(backend.flush());});
$('#search').addEventListener('input',()=>{if(backend.state)caught(renderList());});$('#clear-tag').onclick=()=>{tag='';caught(renderList());};$('#graph-labels').onchange=()=>drawGraph($('#full-graph'),false);
$('#import-input').onchange=e=>{caught(importFiles([...e.target.files]).catch(error=>{toast(error.message);throw error;}));e.target.value='';};
document.addEventListener('keydown',e=>{if($('#dialog').open||$('#connection-dialog').open||!backend.state)return;if(e.key==='Escape'){const inspector=document.body.classList.contains('show-inspector');const sidebar=document.body.classList.contains('sidebar-open');if(inspector){setInspector(false);$('[data-action="toggle-inspector"][aria-expanded]').focus();}else if(sidebar){setSidebar(false);$('[data-action="toggle-sidebar"]').focus();}return;}if(e.key==='?'&&!e.target.closest('input,textarea,[contenteditable="true"]')){e.preventDefault();caught(shortcuts());return;}if((e.ctrlKey||e.metaKey)&&['k','n','s'].includes(e.key.toLowerCase())){e.preventDefault();const key=e.key.toLowerCase();if(key==='k')actions.search();if(key==='n')caught(createNote());if(key==='s')caught(backend.flush().then(()=>toast('Saved to SQLite.')));}});
window.addEventListener('beforeunload',e=>{if(backend.drafts.size){saveDrafts();e.preventDefault();e.returnValue='';}});

async function start(){
  $('#storage-label').textContent='Private · Self-hosted';$('.sidebar-footer small').textContent='Stored on your server.';
  $('#save-status').textContent='Connecting to Rust…';$('#editor').disabled=true;$('#title').disabled=true;
  await connect();
  await backend.load();
  const scope=storageScope(connection.base,backend.state.notebook_id); UI_KEY=scope+'.ui';DRAFT_KEY=scope+'.drafts';
  try{const ui=JSON.parse(localStorage.getItem(UI_KEY)||'null');if(ui){activeId=ui.activeId;openTabs=Array.isArray(ui.openTabs)?ui.openTabs:[];mode=['edit','split','read'].includes(ui.mode)?ui.mode:'read';}}catch{/* Invalid UI preferences do not affect stored notes. */}
  try{recoverDrafts(backend,localStorage.getItem(DRAFT_KEY));}catch{toast('Draft recovery data is invalid. It was not used.');}
  booting=false;render();if(backend.drafts.size&&!backend.blocked)caught(backend.flush());
}
async function refresh(){if(booting||document.hidden||$('#connection-dialog').open)return;try{await backend.refresh();connectionError=false;updateStatus();}catch(error){if(error.status!==401){connectionError=true;updateStatus();}}}
window.addEventListener('focus',()=>caught(refresh()));setInterval(()=>caught(refresh()),15000);
if('serviceWorker' in navigator&&!window.__TAURI_INTERNALS__)navigator.serviceWorker.register('./sw.js').catch(()=>{});
start().catch(error=>{$('#save-status').textContent='Backend unavailable';banner.hidden=false;banner.textContent='Could not load your notebook. '+error.message+' ';const retry=document.createElement('button');retry.textContent='Try again';retry.onclick=()=>location.reload();banner.append(retry);toast('Your saved notes remain on the server.');});
