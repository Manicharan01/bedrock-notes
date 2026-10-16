export function normalizeServer(value) {
  const url=new URL(value);
  if(!['http:','https:'].includes(url.protocol)||url.username||url.password||url.pathname!=='/'||url.search||url.hash)throw new Error('Enter a server origin such as https://notes.example.org');
  if(url.protocol!=='https:'&&!['localhost','127.0.0.1','[::1]'].includes(url.hostname))throw new Error('Remote servers require HTTPS.');
  return url.origin;
}
// Keep the established wire header so restyled clients also work with 0.3 servers.
export class Connection {
  constructor(base,fetcher=globalThis.fetch.bind(globalThis)){this.base=normalizeServer(base);this.fetcher=fetcher;this.token=null;}
  async request(path,payload,method=payload?'POST':'GET') {
    if(!path.startsWith('/api/'))throw new Error('Invalid API path');
    const response=await this.fetcher(this.base+path,{method,credentials:'include',headers:{'X-Lattice-Client':'1',...(this.token?{Authorization:'Bearer '+this.token}:{}),...(payload?{'Content-Type':'application/json'}:{})},body:payload?JSON.stringify(payload):undefined,cache:'no-store',redirect:'error'});
    if(!response.ok){let error;try{error=await response.json();}catch{error={message:'Request failed ('+response.status+'). Your draft was kept.'};}throw Object.assign(new Error(error.message||'Request failed'),{code:error.code,status:response.status});}
    return response.json();
  }
  async login(key){const result=await this.request('/api/session',{key});this.token=result.token;}
  async logout(){try{await this.request('/api/session',undefined,'DELETE');}finally{this.token=null;}}
}
export function storageScope(server,notebook){if(!notebook)throw new Error('Server did not supply a notebook identity');return 'lattice.v3.'+encodeURIComponent(server)+'.'+encodeURIComponent(notebook);}
