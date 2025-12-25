//! ZIP writer module.

pub mod builder;
pub mod writer;
pub mod data;
pub mod reader;

pub use builder::{ZipBuildOutput, ZipBuilder, ZipBuilderOptions};
pub use writer::ZipWriter;
pub use reader::{ZipReader, ZipEntryInfo};

use crate::error::Result;
use std::path::Path;

/// Append files to an existing ZIP.
pub fn append(zipfile: impl AsRef<Path>, root: impl AsRef<Path>, files: &[impl AsRef<str>]) -> Result<()> {
    ZipBuilder::new(zipfile)?
        .append(true)
        .root(root)
        .files(files)?
        .build()?;
    Ok(())
}
