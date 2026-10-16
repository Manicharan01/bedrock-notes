const CACHE='bedrock-shell-v4-1';
const SHELL=['/','/style.css','/app.mjs','/client.mjs','/connection.mjs','/manifest.webmanifest','/icon.svg','/icons/192x192.png','/icons/512x512.png'];
self.addEventListener('install',event=>event.waitUntil(caches.open(CACHE).then(cache=>cache.addAll(SHELL))));
self.addEventListener('activate',event=>event.waitUntil(caches.keys().then(keys=>Promise.all(keys.filter(key=>(key.startsWith('lattice-shell-')||key.startsWith('bedrock-shell-'))&&key!==CACHE).map(key=>caches.delete(key))))));
self.addEventListener('fetch',event=>{
  const url=new URL(event.request.url);
  // Never persist API responses, notebook content, sessions or third-party resources.
  if(event.request.method!=='GET'||url.origin!==self.location.origin||!SHELL.includes(url.pathname)||url.search)return;
  event.respondWith(fetch(event.request).catch(()=>caches.match(url.pathname)));
});
