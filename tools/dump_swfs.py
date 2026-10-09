#!/usr/bin/env python3
"""Owner-only diagnostic: recover valid SWFs from a Wine game process."""
import sys, re, struct, zlib, hashlib
from pathlib import Path
pid=int(sys.argv[1]); out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
seen=set()
with open(f'/proc/{pid}/mem','rb',buffering=0) as mem:
 for line in Path(f'/proc/{pid}/maps').read_text().splitlines():
  parts=line.split();lo,hi=(int(s,16) for s in parts[0].split('-'))
  if 'r' not in parts[1] or hi-lo>512*1024*1024:continue
  try:mem.seek(lo);data=mem.read(hi-lo)
  except (OSError,OverflowError):continue
  for match in re.finditer(b'[FC]WS',data):
   i=match.start(); h=data[i:i+8]
   if len(h)<8 or not 5<=h[3]<=40:continue
   n=struct.unpack_from('<I',h,4)[0]
   if not 20<n<100*1024*1024:continue
   try:
    if h[:3]==b'CWS':
     o=zlib.decompressobj();r=o.decompress(data[i+8:],n)
     if len(r)!=n-8 or not o.eof:continue
     swf=h+data[i+8:i+8+len(data[i+8:])-len(o.unused_data)]
    else:
     if i+n>len(data):continue
     swf=data[i:i+n]
    digest=hashlib.sha256(swf).hexdigest()
    if digest in seen:continue
    seen.add(digest);p=out/f'{lo+i:x}-{n}-{digest[:12]}.swf';p.write_bytes(swf)
    print(p.name,len(swf),flush=True)
   except (zlib.error,ValueError):continue
