#!/usr/bin/env python3
"""Bounded, offline owner-side MDM Zinc payload recovery using Wine."""
import hashlib, os, shutil, subprocess, time
from pathlib import Path
INSTALLER_SHA256 = '032966b8238cf4af5c84be5a793e0a5401006bf3b2dae7c06b3cd245b39dc9e8'
MAIN_SHA256 = '8d26128a342b60ae457c0a7476821d11959818e1669308c258125ba4269aa938'

def main():
    inputs=list(Path('/input').glob('*.exe'))
    if len(inputs)!=1: raise RuntimeError('Mount a folder containing only the supported GOG installer at /input (read-only)')
    if hashlib.sha256(inputs[0].read_bytes()).hexdigest()!=INSTALLER_SHA256:
        raise RuntimeError('Unsupported installer hash')
    work=Path('/output'); extracted=work/'extracted'
    if extracted.exists(): raise RuntimeError('Use a new output folder; never replace existing data')
    work.mkdir(parents=True,exist_ok=True)
    subprocess.run(['innoextract','-d',str(extracted),str(inputs[0])],check=True,stdout=(work/'innoextract.log').open('w'))
    env=dict(os.environ,DISPLAY=':88',WINEARCH='win32',WINEPREFIX='/root/dream-prefix',WINEDEBUG='-all')
    xvfb=subprocess.Popen(['Xvfb',':88','-screen','0','1280x720x24'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    wine=None
    try:
        deadline=time.monotonic()+10
        while not Path('/tmp/.X11-unix/X88').exists():
            if time.monotonic()>deadline: raise RuntimeError('Xvfb startup timeout')
            time.sleep(.1)
        wine=subprocess.Popen(['wine','./the_dream_machine.exe'],cwd=extracted,env=env,stdout=(work/'wine.log').open('w'),stderr=subprocess.STDOUT)
        deadline=time.monotonic()+180
        while time.monotonic()<deadline:
            subprocess.run(['python3','/tools/dump_swfs.py',str(wine.pid),str(work/'memory-swfs')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            for swf in (work/'memory-swfs').glob('*.swf'):
                if hashlib.sha256(swf.read_bytes()).hexdigest()==MAIN_SHA256:
                    shutil.copyfile(swf,work/'main-original.swf')
                    print('Recovered verified main-original.swf',MAIN_SHA256,flush=True)
                    return
            if wine.poll() is not None: raise RuntimeError('Game exited before payload recovery; inspect wine.log')
            time.sleep(5)
        raise RuntimeError('SWF recovery timed out; inspect wine.log')
    finally:
        subprocess.run(['wineserver','-k'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        if wine is not None:
            try: wine.wait(timeout=10)
            except subprocess.TimeoutExpired: wine.kill(); wine.wait()
        xvfb.terminate();xvfb.wait(timeout=10)

if __name__=='__main__': main()
