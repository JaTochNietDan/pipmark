"""Build, notarize, staple, package and sign the feed; publication is a CI step."""
import json
import os
from pathlib import Path
import subprocess
root = Path(__file__).resolve().parent.parent
os.chdir(root)
def run(*args):
    subprocess.run(args, check=True)
run('mise', 'run', 'package-signed')
metadata = json.loads(subprocess.check_output(['mise','exec','--','cargo','metadata','--no-deps','--format-version','1']))
version = metadata['packages'][0]['version']
archive = next((root/'dist').glob(f'pipmark-{version}-*-developer-id.zip'))
auth = ['--keychain-profile', os.environ['PIPMARK_NOTARY_PROFILE']]
if os.environ.get('PIPMARK_NOTARY_KEYCHAIN'):
    auth += ['--keychain', os.environ['PIPMARK_NOTARY_KEYCHAIN']]
submission = json.loads(subprocess.check_output(['xcrun','notarytool','submit',str(archive),*auth,'--no-wait','--output-format','json']))
(root/'dist/notary-submission.json').write_text(json.dumps(submission))
result = json.loads(subprocess.check_output(['xcrun','notarytool','wait',submission['id'],*auth,'--timeout','20m','--output-format','json']))
if result.get('status') != 'Accepted':
    raise SystemExit('Apple did not accept this submission: '+submission['id'])
app = 'dist/developer-id/Pipmark.app'
run('xcrun','stapler','staple',app)
run('xcrun','stapler','validate',app)
run('spctl','--assess','--type','execute',app)
run('python3','scripts/package.py','--bundle',app)
run('python3','scripts/appcast.py',str(archive),'--download-prefix',f'https://github.com/JaTochNietDan/pipmark/releases/download/v{version}/')
