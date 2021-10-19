use std::path::PathBuf;
use std::str::FromStr;
use std::marker::PhantomData;

/// Option trait for InputPath
pub trait InputPathOptions {
    /// Whether to allow directory as input path
    #[inline(always)]
    fn allow_directory() -> bool { true }

    /// Whether to walk dir recursively
    #[inline(always)]
    fn recursive() -> bool { true }

    /// Whether to allow file as input path
    #[inline(always)]
    fn allow_file() -> bool { true }

    /// Whether to follow symbolic link when parsing input path
    #[inline(always)]
    fn follow_symlink() -> bool { true }

    /// File extension filters
    fn allowed_extensions() -> &'static [&'static str] { &[] }
    fn allow_extension(ext: &str) -> bool {
        // empty allowed_extensions means allow all extensions
        Self::allowed_extensions().is_empty() || Self::allowed_extensions().contains(&ext)
    }
}

#[derive(Debug, Clone)]
pub struct InputPath<T: InputPathOptions> {
    marker: PhantomData<T>,
    inner: PathBuf,
}

impl<T: InputPathOptions> FromStr for InputPath<T> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = PathBuf::from_str(s)?;
        // input path must exist
        if !path.exists() {
            bail!("Path {:?} does not exist!", path);
        }

        // symlink
        let meta = path.symlink_metadata()?;
        if meta.file_type().is_symlink() && !T::follow_symlink() {
            bail!("Symbolic link path {:?} detected.", path);
        }

        // file || directory
        if meta.file_type().is_file() && !T::allow_file() {
            bail!("File is now allowed as input!");
        } else if meta.file_type().is_dir() && !T::allow_directory() {
            bail!("Directory is not allow as input!");
        } else if !meta.file_type().is_file() && !meta.file_type().is_dir() {
            bail!("Unsupported file type {:?}.", meta.file_type());
        } else {
            Ok(Self { marker: Default::default(), inner: path })
        }
    }
}

impl<T: InputPathOptions> InputPath<T> {
    pub fn iter(&self) -> impl Iterator<Item=PathBuf> {
        anni_common::fs::PathWalker::new(self.inner.as_path(), T::recursive()).filter(|file| {
            match file.extension() {
                Some(ext) => {
                    let ext = ext.to_string_lossy().to_string();
                    T::allow_extension(&ext)
                }
                None => false,
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct FlacInputPath;

impl InputPathOptions for FlacInputPath {
    fn allowed_extensions() -> &'static [&'static str] {
        &["flac"]
    }
}

#[derive(Debug, Clone)]
pub struct FlacInputFile;

impl InputPathOptions for FlacInputFile {
    fn allow_directory() -> bool {
        false
    }

    fn allowed_extensions() -> &'static [&'static str] {
        &["flac"]
    }
}