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
