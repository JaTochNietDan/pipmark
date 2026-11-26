"""Generate a signed Sparkle feed from one finalized (notarized/stapled) disk image or ZIP."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('archive', type=Path)
parser.add_argument('--download-prefix', required=True, help='HTTPS GitHub release asset directory URL')
args = parser.parse_args()
if not args.download_prefix.startswith('https://'):
    parser.error('An HTTPS download prefix is required')
with tempfile.TemporaryDirectory(prefix='pipmark-appcast-') as temporary:
    folder = Path(temporary)
    shutil.copy2(args.archive, folder / args.archive.name)
    subprocess.run([str(root / 'target/sparkle/bin/generate_appcast'),
                    *(['--ed-key-file', os.environ['PIPMARK_SPARKLE_KEY_FILE']] if os.environ.get('PIPMARK_SPARKLE_KEY_FILE') else ['--account', 'club.oleary.pipmark']), '--maximum-deltas', '0',
                    '--download-url-prefix', args.download_prefix.rstrip('/') + '/', str(folder)], check=True)
    feed = folder / 'appcast.xml'
    enclosure = ET.parse(feed).find('./channel/item/enclosure')
    assert enclosure is not None and enclosure.get('{http://www.andymatuschak.org/xml-namespaces/sparkle}edSignature')
    shutil.copy2(feed, root / 'dist/appcast.xml')
print(root / 'dist/appcast.xml')
