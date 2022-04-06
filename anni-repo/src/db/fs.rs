// https://github.com/rklaehn/sqlite-vfs/blob/main/examples/mem.rs
use wasm_bindgen::prelude::*;

use std::{collections::BTreeMap, ffi::CStr};

use log::info;
use sqlite_vfs::{OpenOptions, Vfs, VfsResult, SQLITE_IOERR, ffi};

#[wasm_bindgen]
pub struct MemVfs {
    files: BTreeMap<String, Option<Vec<u8>>>,
}

impl std::fmt::Debug for MemVfs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemVfs").finish()
    }
}

#[wasm_bindgen]
impl MemVfs {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MemVfs {
        Self {
            files: Default::default(),
        }
    }

    pub fn add_file(mut self, filename: String, content: Vec<u8>) -> Self {
        self.files.insert(filename, Some(content));
        self
    }

    pub fn build(self, name: String) {
        sqlite_vfs::register(&name, self).unwrap();
    }
}

pub struct MemFile {
    name: String,
    opts: OpenOptions,
    data: Vec<u8>,
}

impl Drop for MemFile {
    fn drop(&mut self) {
        info!("drop {:?}", self);
    }
}

impl std::fmt::Debug for MemFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}({})", self.opts.kind, self.name)
    }
}

impl sqlite_vfs::File for MemFile {
    fn read(&mut self, start: u64, buf: &mut [u8]) -> VfsResult<usize> {
        info!("read {:?} {} {}", self, start, buf.len());
        let start = start as usize;
        let remaining = self.data.len().saturating_sub(start);
        let n = remaining.min(buf.len());
        if n != 0 {
            buf[..n].copy_from_slice(&self.data[start..start + n]);
        }
        Ok(n)
    }

    fn write(&mut self, start: u64, buf: &[u8]) -> VfsResult<usize> {
        info!("write {:?} {} {}", self, start, buf.len());
        let start = start as usize;
        if start > self.data.len() {
            return Err(SQLITE_IOERR);
        }
        let current_len = self.data.len();
        let len = buf.len();
        let end = start + buf.len();
        self.data.extend((current_len..end).map(|_| 0u8));
        self.data[start..end].copy_from_slice(&buf);
        Ok(len)
    }

    fn sync(&mut self) -> VfsResult<()> {
        info!("sync {:?}", self);
        // if self.opts.kind == OpenKind::MainDb {
        //     return Err(SQLITE_IOERR)
        // }
        Ok(())
    }

    fn file_size(&self) -> VfsResult<u64> {
        info!("file_size {:?}", self);
        Ok(self.data.len() as u64)
    }

    fn truncate(&mut self, size: u64) -> VfsResult<()> {
        info!("truncate {:?} {}", self, size);
        let size = size as usize;
        self.data.truncate(size);
        Ok(())
    }

    fn sector_size(&self) -> usize {
        1024
    }

    fn device_characteristics(&self) -> i32 {
        // writes of any size are atomic
        ffi::SQLITE_IOCAP_ATOMIC |
            // after reboot following a crash or power loss, the only bytes in a file that were written
            // at the application level might have changed and that adjacent bytes, even bytes within
            // the same sector are guaranteed to be unchanged
            ffi::SQLITE_IOCAP_POWERSAFE_OVERWRITE |
            // when data is appended to a file, the data is appended first then the size of the file is
            // extended, never the other way around
            ffi::SQLITE_IOCAP_SAFE_APPEND |
            // information is written to disk in the same order as calls to xWrite()
            ffi::SQLITE_IOCAP_SEQUENTIAL
    }
}

impl Vfs for MemVfs {
    type File = MemFile;

    fn open(&mut self, path: &CStr, opts: OpenOptions) -> VfsResult<Self::File> {
        let path = path.to_string_lossy();
        info!("open {:?} {} {:?}", self, path, opts);
        let data = if self.files.contains_key(path.as_ref()) {
            self.files.get_mut(path.as_ref()).unwrap().take().unwrap_or_default()
        } else {
            self.files.insert(path.to_string(), None);
            Default::default()
        };
        Ok(MemFile {
            name: path.into(),
            opts,
            data,
        })
    }

    fn delete(&mut self, path: &CStr) -> VfsResult<()> {
        let path = path.to_string_lossy();
        let t: &str = &path;
        self.files.remove(t);
        info!("delete {:?} {}", self, path);
        Ok(())
    }

    fn exists(&mut self, path: &CStr) -> VfsResult<bool> {
        let path = path.to_string_lossy();
        let t: &str = &path;
        let res = self.files.contains_key(t);
        info!("exists {:?} {}", self, path);
        Ok(res)
    }

    /// Check access to `path`. The default implementation always returns `true`.
    fn access(&mut self, path: &CStr, write: bool) -> VfsResult<bool> {
        let path = path.to_string_lossy();
        info!("access {} {}", path, write);
        Ok(true)
    }
}