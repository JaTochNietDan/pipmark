"""Deploy the static marketing site as a Cloudflare Worker (token from environment)."""
import base64,json,mimetypes,os,urllib.request,uuid
from pathlib import Path
root=Path(__file__).resolve().parent.parent
account='e78e0111cdb60c145307f1333efbc9ce'
assets={}
for name in ['index.html','style.css','icon.svg','settings.png']:
 p=root/'site'/name
 assets['/'+name]={'type':mimetypes.guess_type(name)[0], 'body':base64.b64encode(p.read_bytes()).decode()}
module='const ASSETS='+json.dumps(assets)+';\n'+(root/'site/worker.js').read_text()
boundary=uuid.uuid4().hex
parts=[]
for name,filename,kind,body in [('metadata','metadata.json','application/json',json.dumps({'main_module':'worker.js','compatibility_date':'2026-11-25'})),('worker.js','worker.js','application/javascript+module',module)]:
 parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"; filename="{filename}"\r\nContent-Type: {kind}\r\n\r\n{body}\r\n'.encode())
data=b''.join(parts)+f'--{boundary}--\r\n'.encode()
req=urllib.request.Request(f'https://api.cloudflare.com/client/v4/accounts/{account}/workers/scripts/pipmark-site',data=data,method='PUT',headers={'Authorization':'Bearer '+os.environ['CLOUDFLARE_API_TOKEN'],'Content-Type':'multipart/form-data; boundary='+boundary})
result=json.load(urllib.request.urlopen(req))
if not result['success']:raise SystemExit('Worker deployment failed')
print('Deployed pipmark-site')
