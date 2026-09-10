"""Local differential check. Decompressed game bytes exist only in a temporary directory."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'research'))
import native_count

parser = argparse.ArgumentParser()
parser.add_argument('--reader', type=Path, required=True)
parser.add_argument('--cooked', type=Path, default=Path(native_count.GAME))
args = parser.parse_args()
packages = ['Core', 'Engine', 'GameFramework', 'GearboxFramework', 'WillowGame', 'GFxUI', 'IpDrv', 'OnlineSubsystemSteamworks', 'AkAudio']
with tempfile.TemporaryDirectory(prefix='openwillow-') as folder:
    temporary = Path(folder) / 'package.bin'
    for package in packages:
        data = native_count.unwrap_fully_compressed(args.cooked / (package + '.upk'))
        version, licensee, names, imports, exports = native_count.parse_package(data)
        temporary.write_bytes(data)
        result = subprocess.run([str(args.reader.resolve()), str(args.cooked / (package + '.upk')),
                                 '--verify-decoded', str(temporary)], check=True, capture_output=True, text=True)
        actual = json.loads(result.stdout)
        expected = dict(version=version, licensee=licensee, names=len(names), imports=len(imports), exports=len(exports))
        if actual != expected:
            raise RuntimeError(f'{package}: {actual} != {expected}')
        print(f'{package}: {len(data)} bytes, {len(exports)} exports; every decoded byte and table count matches', flush=True)
