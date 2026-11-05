use kotor_core::{Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where an archive's bytes live: a file read on demand, or memory.
#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf),
    Memory(Arc<[u8]>),
}

impl Source {
    pub fn file(path: impl AsRef<Path>) -> Source {
        Source::File(path.as_ref().to_path_buf())
    }

    pub fn memory(data: Vec<u8>) -> Source {
        Source::Memory(data.into())
    }

    /// Read `len` bytes at `offset`.
    pub fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        match self {
            Source::File(p) => {
                let mut f = File::open(p)?;
                let size = f.metadata()?.len();
                if offset.checked_add(len as u64).is_none_or(|e| e > size) {
                    return Err(Error::format(format!(
                        "range {offset}+{len} past end of {} ({size} bytes)",
                        p.display()
                    )));
                }
                f.seek(SeekFrom::Start(offset))?;
                let mut buf = vec![0u8; len];
                f.read_exact(&mut buf)?;
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
            Source::File(p) => Ok(std::fs::metadata(p)?.len()),
            Source::Memory(m) => Ok(m.len() as u64),
        }
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
}
