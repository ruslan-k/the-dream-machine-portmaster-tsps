import ctypes,fcntl,json,mmap,os,struct,time,collections
from pathlib import Path
root=Path('/mnt/SDCARD/Roms/PORTS/dreammachine')
processes=[]
for p in Path('/proc').glob('[0-9]*'):
 try:
  if b'dreammachine/ruffle' in (p/'cmdline').read_bytes():processes.append(p)
 except OSError:pass
assert len(processes)==1,processes
p=processes[0];pid=int(p.name);page=os.sysconf('SC_PAGE_SIZE')
libc=ctypes.CDLL(None,use_errno=True);libc.syscall.restype=ctypes.c_long
rings=[];failures=[];counts=collections.Counter();lost=0;waits=collections.Counter();timeline=[]
for t in (p/'task').iterdir():
 attr=ctypes.create_string_buffer(112)
 struct.pack_into('<IIQQQQQ',attr,0,1,112,0,10000000,3,0,1|(1<<5)|(1<<6))
 struct.pack_into('<I',attr,48,1)
 fd=libc.syscall(241,ctypes.byref(attr),int(t.name),-1,-1,8)
 if fd<0:failures.append({'tid':int(t.name),'errno':ctypes.get_errno()});continue
 try:
  ring=mmap.mmap(fd,page*9,flags=mmap.MAP_SHARED,prot=mmap.PROT_READ|mmap.PROT_WRITE)
  fcntl.ioctl(fd,0x2403,0);fcntl.ioctl(fd,0x2400,0)
  rings.append((fd,ring,0))
 except Exception:
  os.close(fd);raise
print('sampling',pid,'perf_threads',len(rings),'errors',failures,flush=True)
initial_maps=(p/'maps').read_text();vm0=Path('/proc/vmstat').read_text();start=time.monotonic();last=-1
try:
 while time.monotonic()-start<45:
  for i,(fd,ring,tail) in enumerate(rings):
   head=struct.unpack_from('<Q',ring,1024)[0];dataoff=struct.unpack_from('<Q',ring,1040)[0] or page;size=struct.unpack_from('<Q',ring,1048)[0] or page*8
   def read(off,n):
    off%=size;first=min(n,size-off)
    return ring[dataoff+off:dataoff+off+first]+(ring[dataoff:dataoff+n-first] if first<n else b'')
   while tail<head:
    typ,misc,n=struct.unpack('<IHH',read(tail,8))
    if n<8 or n>size:raise RuntimeError('Invalid perf record size')
    body=read(tail+8,n-8)
    if typ==9:
     ip,tidpid,tid=struct.unpack_from('<QII',body);counts[(tid,ip)]+=1
    elif typ==2:lost+=struct.unpack_from('<QQ',body)[1]
    tail+=n
   struct.pack_into('<Q',ring,1032,tail);rings[i]=(fd,ring,tail)
  waits[(p/'wchan').read_text().strip()]+=1
  sec=int(time.monotonic()-start)
  if sec!=last:
   last=sec;threads={}
   for t in (p/'task').iterdir():
    try:
     a=(t/'stat').read_text().rsplit(')',1)[1].split();threads[t.name]={'name':(t/'comm').read_text().strip(),'ticks':int(a[11])+int(a[12]),'state':a[0]}
    except OSError:pass
   st={l.split(':')[0]:l.split(':')[1].strip() for l in (p/'status').read_text().splitlines() if l.startswith(('VmRSS:','VmSwap:','RssAnon:','RssFile:','VmData:'))}
   io=dict(l.split(': ') for l in (p/'io').read_text().splitlines()) if (p/'io').exists() else {}
   timeline.append({'elapsed':time.monotonic()-start,'memory':st,'threads':threads,'io':io})
  time.sleep(.1)
finally:
 for fd,ring,tail in rings:
  fcntl.ioctl(fd,0x2401,0);ring.close();os.close(fd)
result={'pid':pid,'duration':time.monotonic()-start,'perf_errors':failures,'lost':lost,'samples':[{'tid':tid,'ip':hex(ip),'count':n} for (tid,ip),n in counts.most_common()],'wchan_counts':dict(waits),'timeline':timeline,'maps':initial_maps,'vmstat_before':vm0,'vmstat_after':Path('/proc/vmstat').read_text(),'log_tail':(root/'log.txt').read_text()[-12000:]}
(root/'logs/live-profile.json').write_text(json.dumps(result))
print('samples',sum(counts.values()),'lost',lost,'wchan',dict(waits),flush=True)
