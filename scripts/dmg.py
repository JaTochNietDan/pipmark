"""Create a compressed drag-to-Applications disk image; signing is optional."""
import argparse,json,os,plistlib,re,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser()
parser.add_argument('--bundle',type=Path,default=root/'dist/developer-id/Pipmark.app')
parser.add_argument('--sign',action='store_true')
args=parser.parse_args()
app=args.bundle.resolve()
info=plistlib.loads((app/'Contents/Info.plist').read_bytes())
version=info['CFBundleShortVersionString']
assert re.fullmatch(r'[0-9A-Za-z.-]+',version)
architecture=subprocess.check_output(['lipo','-archs',str(app/'Contents/MacOS/Pipmark')],text=True).strip()
architecture='universal' if ' ' in architecture else architecture
output=root/'dist'/f'pipmark-{version}-{architecture}.dmg'
subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
with tempfile.TemporaryDirectory(prefix='pipmark-dmg-') as temporary:
 folder=Path(temporary);stage=folder/'stage';stage.mkdir()
 subprocess.run(['ditto',str(app),str(stage/'Pipmark.app')],check=True)
 (stage/'Applications').symlink_to('/Applications')
 candidate=folder/'Pipmark.dmg'
 subprocess.run(['hdiutil','create','-volname','Pipmark','-srcfolder',str(stage),'-format','UDZO','-ov',str(candidate)],check=True)
 if args.sign:
  identities=subprocess.check_output(['security','find-identity','-v','-p','codesigning'],text=True)
  matching=[m.group(1) for line in identities.splitlines() if 'Developer ID Application:' in line for m in [re.search(r'\b([0-9A-F]{40})\b',line)] if m]
  identity=os.environ.get('BADGEIFY_SIGN_IDENTITY') or (matching[0] if len(matching)==1 else None)
  if identity not in matching:raise SystemExit('Select an available Developer ID identity with BADGEIFY_SIGN_IDENTITY')
  subprocess.run(['codesign','--force','--timestamp','--sign',identity,str(candidate)],check=True)
  subprocess.run(['codesign','--verify','--strict',str(candidate)],check=True)
 subprocess.run(['hdiutil','verify',str(candidate)],check=True)
 os.replace(candidate,output)
print(output)
