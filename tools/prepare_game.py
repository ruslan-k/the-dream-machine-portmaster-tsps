#!/usr/bin/env python3
"""Prepare owner-supplied extracted GOG assets and a recovered Zinc SWF."""
import argparse, hashlib, json, shutil
from pathlib import Path
from patch_game import patch
EXE_SHA256 = 'e2f6cefde74549f9d8dd2836566b4f5ea028ed30cb98b4b4bc10f1b4bbb646e2'
CONFIG = 'Config(gamepad_config: {"start": 13, "select": 27}, swf_url: None, swf_name: Some("movie.swf"))\n'

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--extracted', required=True, type=Path)
    p.add_argument('--swf', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    a = p.parse_args()
    if a.output.exists(): p.error('Output must be a new directory; saves must never be replaced')
    exe = a.extracted / 'the_dream_machine.exe'
    if hashlib.sha256(exe.read_bytes()).hexdigest() != EXE_SHA256:
        p.error('Unsupported GOG executable/version')
    movie = patch(a.swf.read_bytes())
    required = ['assets', 'launcher.txt', 'gameAssets.swf', 'gameAsset_fonts.swf', 'gameAsset_floors.swf', 'gameAsset_sounds.swf'] + ['TDM_chapter'+str(n) for n in range(1,7)] + ['TDM_end']
    for name in required:
        if not (a.extracted/name).exists(): p.error('Missing asset: '+name)
    a.output.mkdir(parents=True)
    for name in required:
        src=a.extracted/name; dst=a.output/name
        if src.is_dir(): shutil.copytree(src,dst)
        else: shutil.copy2(src,dst)
    (a.output/'movie.swf').write_bytes(movie)
    (a.output/'config.ron').write_text(CONFIG)
    files=sorted(x for x in a.output.rglob('*') if x.is_file())
    manifest={str(x.relative_to(a.output)):hashlib.sha256(x.read_bytes()).hexdigest() for x in files}
    (a.output.parent/(a.output.name+'-sha256.json')).write_text(json.dumps(manifest,indent=2)+'\n')
    print('Prepared',len(files),'files;',sum(x.stat().st_size for x in files),'bytes')

if __name__ == '__main__': main()
