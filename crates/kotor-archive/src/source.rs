use kotor_core::{Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Where an archive's bytes live: a file, opened at the first read and kept open, or memory.
///
/// Keeping it open means one open per file for the life of the archive. Opening for every read gave any other program
/// that held the file for a moment a chance to fail thousands of reads, and a test once lost 62 scripts in a row that
/// way or some other (`docs/PLAY.md`).
#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf, Arc<Mutex<Option<File>>>),
    Memory(Arc<[u8]>),
}

/// An error that says which file and what the operating system answered.
fn named(p: &Path, e: std::io::Error) -> Error {
    Error::format(format!(
        "{}: {e} (os error {:?})",
        p.display(),
        e.raw_os_error()
    ))
}

impl Source {
    pub fn file(path: impl AsRef<Path>) -> Source {
        Source::File(path.as_ref().to_path_buf(), Arc::default())
    }

    pub fn memory(data: Vec<u8>) -> Source {
        Source::Memory(data.into())
    }

    /// Read `len` bytes at `offset`.
    pub fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        match self {
            Source::File(p, held) => {
                let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
                let f = match &mut *held {
                    Some(f) => f,
                    empty => empty.insert(File::open(p).map_err(|e| named(p, e))?),
                };
                let size = f.metadata().map_err(|e| named(p, e))?.len();
                if offset.checked_add(len as u64).is_none_or(|e| e > size) {
                    return Err(Error::format(format!(
                        "range {offset}+{len} past end of {} ({size} bytes)",
                        p.display()
                    )));
                }
                f.seek(SeekFrom::Start(offset)).map_err(|e| named(p, e))?;
                let mut buf = vec![0u8; len];
                f.read_exact(&mut buf).map_err(|e| named(p, e))?;
                Ok(buf)
            }
            Source::Memory(m) => {
                let start = usize::try_from(offset).unwrap_or(usize::MAX);
                match start.checked_add(len).filter(|&e| e <= m.len()) {
                    Some(end) => Ok(m[start..end].to_vec()),
                    None => Err(Error::format("range past end of memory archive")),
                }
            }
        }
    }

    pub fn len(&self) -> Result<u64> {
        match self {
            Source::File(p, _) => Ok(std::fs::metadata(p)?.len()),
            Source::Memory(m) => Ok(m.len() as u64),
        }
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_opened_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("kotor_rs_source_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.bin"), dir.join("b.bin"));
        std::fs::write(&a, b"hello").unwrap();
        let s = Source::file(&a);
        assert_eq!(s.read_at(1, 3).unwrap(), b"ell");
        // Gone from its name, and still readable: this read did not open anything.
        std::fs::rename(&a, &b).unwrap();
        assert_eq!(s.read_at(0, 5).unwrap(), b"hello");
        assert!(s.read_at(3, 5).is_err(), "past the end");
        // A file that is not there says which, and what the system said.
        let e = Source::file(&a).read_at(0, 1).unwrap_err().to_string();
        assert!(e.contains("a.bin") && e.contains("os error"), "{e}");
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
