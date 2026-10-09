//! Opt-in lossless disk backing for deferred bitmap pixels.
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
#[derive(Debug)]
pub enum Pixels { Memory(Vec<u8>), File(PathBuf, usize) }
impl Pixels {
    pub fn len(&self) -> usize { match self { Self::Memory(b) => b.len(), Self::File(_, n) => *n } }
    pub fn compressed(self) -> Vec<u8> { match self {
        Self::Memory(b) => b,
        Self::File(p, _) => checked_read(&p).expect("Disk bitmap cache changed while the game was running"),
    } }
}
fn checked_read(p: &Path) -> Option<Vec<u8>> {
    let b = std::fs::read(p).ok()?;
    if b.len() < 36 || &b[..4] != b"TBC1" || Sha256::digest(&b[36..]).as_slice() != &b[4..36] { return None; }
    Some(b[36..].to_vec())
}
fn prepare_at(dir: Option<PathBuf>, format:u32, w:u32, h:u32, raw:&[u8]) -> (Pixels,bool) {
    let path = dir.map(|dir| {
        let mut hash=Sha256::new();
        hash.update(format.to_le_bytes()); hash.update(w.to_le_bytes()); hash.update(h.to_le_bytes()); hash.update(raw);
        let key=format!("{:x}",hash.finalize());
        dir.join(&key[..2]).join(format!("{key}.bin"))
    });
    if let Some(p)=&path { if let Some(b)=checked_read(p) { return (Pixels::File(p.clone(),b.len()),true); } }
    let b=miniz_oxide::deflate::compress_to_vec_zlib(raw,1);
    if let Some(p)=path {
        let write = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(p.parent().unwrap())?;
            let tmp=p.with_extension(format!("tmp-{}",std::process::id()));
            let mut data=Vec::with_capacity(b.len()+36); data.extend_from_slice(b"TBC1");
            data.extend_from_slice(&Sha256::digest(&b)); data.extend_from_slice(&b);
            std::fs::write(&tmp,&data)?; std::fs::rename(tmp,&p)?; Ok(())
        })();
        if write.is_ok() { return (Pixels::File(p,b.len()),false); }
        eprintln!("bitmap_cache_write_failed: {}; using RAM",p.display());
    }
    (Pixels::Memory(b),false)
}
pub fn prepare(format:u32,w:u32,h:u32,raw:&[u8]) -> Pixels {
    let dir=std::env::var_os("TOR_BITMAP_CACHE_DIR").map(PathBuf::from);
    let enabled=dir.is_some(); let (b,hit)=prepare_at(dir,format,w,h,raw);
    static HITS:AtomicUsize=AtomicUsize::new(0); static MISSES:AtomicUsize=AtomicUsize::new(0);
    static BACKED:AtomicUsize=AtomicUsize::new(0);
    if enabled {
        if hit { HITS.fetch_add(1,Relaxed); } else { MISSES.fetch_add(1,Relaxed); }
        if matches!(&b,Pixels::File(_, _)) { BACKED.fetch_add(b.len(),Relaxed); }
        let count=HITS.load(Relaxed)+MISSES.load(Relaxed);
        if count%250==0 {eprintln!("bitmap_disk_cache hits={} misses={} backed_bytes={}",HITS.load(Relaxed),MISSES.load(Relaxed),BACKED.load(Relaxed));}
    }
    b
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn lossless_cold_warm_corrupt_and_fallback() {
        let dir=std::env::temp_dir().join(format!("tor-bitmap-test-{}",std::process::id()));
        let raw:Vec<u8>=(0..16384).map(|n|(n%251) as u8).collect();
        let (cold,hit)=prepare_at(Some(dir.clone()),6408,64,64,&raw); assert!(!hit);
        let path=match &cold { Pixels::File(p,_)=>p.clone(),_=>panic!("Expected disk backing") };
        assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&cold.compressed()).unwrap(),raw);
        let (warm,hit)=prepare_at(Some(dir.clone()),6408,64,64,&raw);assert!(hit);
        assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&warm.compressed()).unwrap(),raw);
        std::fs::write(&path,b"corrupt").unwrap();
        let (repaired,hit)=prepare_at(Some(dir.clone()),6408,64,64,&raw);assert!(!hit);
        assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&repaired.compressed()).unwrap(),raw);
        let (ram,_)=prepare_at(Some(path),6408,32,128,&raw);assert!(matches!(&ram,Pixels::Memory(_)));
        assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&ram.compressed()).unwrap(),raw);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
