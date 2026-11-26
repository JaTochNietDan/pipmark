export default {
  async fetch(request) {
    const url = new URL(request.url);
    if (url.hostname === 'www.pipmark.io') {
      url.hostname = 'pipmark.io';
      url.protocol = 'https:';
      return Response.redirect(url.toString(), 301);
    }
    const path = url.pathname;
    if (!['GET', 'HEAD'].includes(request.method)) return new Response('Method not allowed', {status:405,headers:{Allow:'GET, HEAD'}});
    if (path === '/download') {
      const releasePage = 'https://github.com/JaTochNietDan/pipmark/releases/latest';
      try {
        const response = await fetch('https://api.github.com/repos/JaTochNietDan/pipmark/releases/latest', {
          headers: {Accept:'application/vnd.github+json', 'User-Agent':'Pipmark-Website'},
          cf: {cacheTtl:300, cacheEverything:true}
        });
        if (response.ok) {
          const release = await response.json();
          const dmg = release.assets?.find(asset => /^pipmark-[0-9A-Za-z.-]+-arm64\.dmg$/.test(asset.name));
          if (dmg?.browser_download_url?.startsWith('https://github.com/JaTochNietDan/pipmark/releases/download/'))
            return Response.redirect(dmg.browser_download_url, 302);
        }
      } catch (_) { /* The release page remains a usable fallback. */ }
      return Response.redirect(releasePage, 302);
    }
    const asset = ASSETS[path === '/' ? '/index.html' : path];
    if (!asset) return new Response('Not found', {status:404});
    const bytes = Uint8Array.from(atob(asset.body), c => c.charCodeAt(0));
    return new Response(request.method === 'HEAD' ? null : bytes, {headers:{
      'Content-Type':asset.type, 'Cache-Control':'public, max-age=300',
      'X-Content-Type-Options':'nosniff', 'Referrer-Policy':'strict-origin-when-cross-origin',
      'Content-Security-Policy':"default-src 'none'; style-src 'self'; img-src 'self'; base-uri 'none'; frame-ancestors 'none'"
    }});
  }
};
