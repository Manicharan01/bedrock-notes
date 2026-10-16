// Titles may be valid notes while containing characters a native save dialog rejects.
export function markdownFilename(title) {
  let name=String(title).split(/[\\/]/).at(-1).replace(/[<>:"|?*\u0000-\u001f\u007f]/g,'-').replace(/[. ]+$/g,'').trim();
  if(!name)name='Untitled';
  if(/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name))name='_'+name;
  return name+'.md';
}

// Serializes writes and keeps unsaved edits separate from server-confirmed snapshots.
export class Backend {
  constructor(request, onChange = () => {}, onError = () => {}) {
    this.request = request; this.onChange = onChange; this.onError = onError;
    this.state = null; this.drafts = new Map(); this.serial = 0;
    this.tail = Promise.resolve(); this.blocked = false; this.commandBusy = false;
  }
  async load() { this.state = await this.request('/api/notebook'); this.onChange(); }
  refresh() {
    return this.enqueue(async () => {
      if(this.drafts.size||this.blocked||this.commandBusy)return;
      const version=await this.request('/api/version');
      if(version.notebook_id!==this.state.notebook_id){this.blocked=true;throw new Error('The server notebook was replaced. Export drafts and reload.');}
      if(version.revision!==this.state.revision){
        const latest=await this.request('/api/notebook');
        if(this.drafts.size)return;
        if(latest.notebook_id!==this.state.notebook_id){this.blocked=true;throw new Error('The server notebook was replaced. Reload before writing.');}
        this.state=latest;this.onChange();
      }
    });
  }
  edit(id, body) {
    const previous = this.drafts.get(id);
    const saved = this.state.notes.find(n => n.id === id);
    if (!saved) throw new Error('Note no longer exists');
    this.drafts.set(id, {body, baseBody: previous?.baseBody ?? saved.body, serial: ++this.serial});
  }
  body(id) { return this.drafts.get(id)?.body ?? this.state.notes.find(n=>n.id===id)?.body ?? ''; }
  enqueue(action) {
    const job = this.tail.then(action);
    this.tail = job.catch(() => {});
    return job.catch(error => { if(error.code === 'revision_conflict') this.blocked = true; this.onError(error); throw error; });
  }
  flush() { return this.enqueue(() => this.flushInternal()); }
  async flushInternal() {
    if(this.blocked) throw Object.assign(new Error('Notebook changed. Export drafts and reload.'), {code:'revision_conflict'});
    while(this.drafts.size) {
      const [id, draft] = this.drafts.entries().next().value;
      const response = await this.request('/api/commands', {notebook_id:this.state.notebook_id,revision:this.state.revision,kind:'update',id,body:draft.body});
      this.state = response.notebook;
      const current = this.drafts.get(id);
      if(current?.serial === draft.serial) this.drafts.delete(id);
      else if(current) current.baseBody = this.state.notes.find(n=>n.id===id)?.body ?? current.baseBody;
      this.onChange();
    }
  }
  mutate(command) {
    return this.enqueue(async () => {
      await this.flushInternal();
      this.commandBusy = true; this.onChange();
      try {
        const response = await this.request('/api/commands', {notebook_id:this.state.notebook_id,revision:this.state.revision,...command});
        this.state = response.notebook; this.onChange(); return response.selected_id;
      } finally { this.commandBusy = false; this.onChange(); }
    });
  }
}

export function recoverDrafts(backend, serialized) {
  const entries=JSON.parse(serialized||'[]');
  if(!Array.isArray(entries))throw new Error('Invalid recovery data');
  const ids=new Set();
  for(const entry of entries){
    if(!Array.isArray(entry)||entry.length!==2||typeof entry[0]!=='string'||typeof entry[1]?.body!=='string'||typeof entry[1]?.baseBody!=='string'||ids.has(entry[0]))throw new Error('Invalid recovery data');
    ids.add(entry[0]);
  }
  for(const [id,draft]of entries){
    const saved=backend.state.notes.find(n=>n.id===id);
    if(saved?.body===draft.body)continue;
    if(saved&&saved.body===draft.baseBody)backend.edit(id,draft.body);
    else{backend.drafts.set(id,{...draft,serial:++backend.serial});backend.blocked=true;}
  }
}
