//! Error types for zip-rs
//!
//! This module defines all error types that can occur during ZIP operations.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ZipErrorCode {
    Success = 0,
    Open = 1,
    NoMem = 2,
    NoEntry = 3,
    Broken = 4,
    BrokenEntry = 5,
    Overwrite = 6,
    CreateDir = 7,
    SetPerm = 8,
    SetMtime = 9,
    OpenWrite = 10,
    OpenAppend = 11,
    AddDir = 12,
    AddFile = 13,
    SetZipPerm = 14,
    Create = 15,
    OpenX = 16,
    FileSize = 17,
    CreateLink = 18,
}

impl ZipErrorCode {
    pub fn from_c_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(ZipErrorCode::Success),
            1 => Some(ZipErrorCode::Open),
            2 => Some(ZipErrorCode::NoMem),
            3 => Some(ZipErrorCode::NoEntry),
            4 => Some(ZipErrorCode::Broken),
            5 => Some(ZipErrorCode::BrokenEntry),
            6 => Some(ZipErrorCode::Overwrite),
            7 => Some(ZipErrorCode::CreateDir),
            8 => Some(ZipErrorCode::SetPerm),
            9 => Some(ZipErrorCode::SetMtime),
            10 => Some(ZipErrorCode::OpenWrite),
            11 => Some(ZipErrorCode::OpenAppend),
            12 => Some(ZipErrorCode::AddDir),
            13 => Some(ZipErrorCode::AddFile),
            14 => Some(ZipErrorCode::SetZipPerm),
            15 => Some(ZipErrorCode::Create),
            16 => Some(ZipErrorCode::OpenX),
            17 => Some(ZipErrorCode::FileSize),
            18 => Some(ZipErrorCode::CreateLink),
            _ => None,
        }
    }

    pub fn to_c_code(self) -> i32 {
        self as i32
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Main error type for zip-rs
#[derive(Error, Debug)]
pub enum ZipError {
    #[error("zip error code: {0:?}")]
    CError(ZipErrorCode),

    /// File open failed
    #[error("cannot open file '{path}': {source}")]
    FileOpen {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// File read failed
    #[error("cannot read file '{path}': {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// File write failed
    #[error("cannot write file '{path}': {source}")]
    FileWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Out of memory
    #[error("out of memory")]
    OutOfMemory,

    /// Entry not found in archive
    #[error("entry '{name}' not found in archive '{archive}'")]
    EntryNotFound { name: String, archive: PathBuf },

    /// Corrupt archive
    #[error("corrupt ZIP archive '{archive}': {reason}")]
    CorruptArchive {
        archive: PathBuf,
        reason: String,
    },

    /// Corrupt entry in archive
    #[error("corrupt ZIP entry '{name}' in archive '{archive}': {reason}")]
    CorruptEntry {
        name: String,
        archive: PathBuf,
        reason: String,
    },

    /// Overwrite conflict
    #[error("not overwriting '{path}' when extracting '{archive}'")]
    OverwriteConflict { path: PathBuf, archive: PathBuf },

    /// Directory creation failed
    #[error("cannot create directory '{path}': {source}")]
    CreateDirFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Permission setting failed
    #[error("cannot set permissions for '{path}': {source}")]
    SetPermFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Modification time setting failed
    #[error("failed to set mtime on '{path}': {source}")]
    SetMtimeFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Open for writing failed
    #[error("cannot open zip file '{path}' for writing: {source}")]
    OpenWriteFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Open for appending failed
    #[error("cannot open zip file '{path}' for appending: {source}")]
    OpenAppendFailed {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Add directory failed
    #[error("cannot add directory '{path}' to archive '{archive}'")]
    AddDirFailed { path: String, archive: PathBuf },

    /// Add file failed
    #[error("cannot add file '{path}' to archive '{archive}'")]
    AddFileFailed { path: String, archive: PathBuf },

    /// Set zip permissions failed
    #[error("cannot set permission on file '{path}' in archive '{archive}'")]
    SetZipPermFailed { path: String, archive: PathBuf },

    /// Create zip archive failed
    #[error("could not create zip archive '{archive}'")]
    CreateFailed { archive: PathBuf },

    /// Open extract failed
    #[error("cannot extract file '{path}'")]
    OpenExtractFailed { path: String },

    /// File size failed
    #[error("cannot determine size of '{path}'")]
    FileSizeFailed { path: PathBuf },

    /// Unsupported compression method
    #[error("unsupported compression method: {method}")]
    UnsupportedCompression { method: u16 },

    /// CRC32 mismatch
    #[error("CRC32 mismatch for entry '{name}'")]
    Crc32Mismatch { name: String },

    /// Path error
    #[error("invalid path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },

    /// Symlink creation failed
    #[error("cannot create symlink '{target}' -> '{link}': {source}")]
    CreateSymlinkFailed {
        target: PathBuf,
        link: PathBuf,
        #[source]
        source: io::Error,
    },

    /// IO error with context
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Generic error
    #[error("{0}")]
    Generic(String),
}

impl ZipError {
    pub fn from_c_code(code: i32) -> Self {
        match ZipErrorCode::from_c_code(code) {
            Some(ZipErrorCode::Success) => ZipError::CError(ZipErrorCode::Success),
            Some(ZipErrorCode::Open) => ZipError::FileOpen {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "zip open failed"),
            },
            Some(ZipErrorCode::NoMem) => ZipError::OutOfMemory,
            Some(ZipErrorCode::NoEntry) => ZipError::EntryNotFound {
                name: String::new(),
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::Broken) => ZipError::CorruptArchive {
                archive: PathBuf::new(),
                reason: "corrupt archive".to_string(),
            },
            Some(ZipErrorCode::BrokenEntry) => ZipError::CorruptEntry {
                name: String::new(),
                archive: PathBuf::new(),
                reason: "corrupt entry".to_string(),
            },
            Some(ZipErrorCode::Overwrite) => ZipError::OverwriteConflict {
                path: PathBuf::new(),
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::CreateDir) => ZipError::CreateDirFailed {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "create dir failed"),
            },
            Some(ZipErrorCode::SetPerm) => ZipError::SetPermFailed {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "set permissions failed"),
            },
            Some(ZipErrorCode::SetMtime) => ZipError::SetMtimeFailed {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "set mtime failed"),
            },
            Some(ZipErrorCode::OpenWrite) => ZipError::OpenWriteFailed {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "open for write failed"),
            },
            Some(ZipErrorCode::OpenAppend) => ZipError::OpenAppendFailed {
                path: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "open for append failed"),
            },
            Some(ZipErrorCode::AddDir) => ZipError::AddDirFailed {
                path: String::new(),
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::AddFile) => ZipError::AddFileFailed {
                path: String::new(),
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::SetZipPerm) => ZipError::SetZipPermFailed {
                path: String::new(),
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::Create) => ZipError::CreateFailed {
                archive: PathBuf::new(),
            },
            Some(ZipErrorCode::OpenX) => ZipError::OpenExtractFailed {
                path: String::new(),
            },
            Some(ZipErrorCode::FileSize) => ZipError::FileSizeFailed {
                path: PathBuf::new(),
            },
            Some(ZipErrorCode::CreateLink) => ZipError::CreateSymlinkFailed {
                target: PathBuf::new(),
                link: PathBuf::new(),
                source: io::Error::new(io::ErrorKind::Other, "create symlink failed"),
            },
            None => ZipError::Generic(format!("unknown zip error code {}", code)),
        }
    }

    pub fn to_c_code(&self) -> ZipErrorCode {
        match self {
            ZipError::CError(code) => *code,
            ZipError::FileOpen { .. } => ZipErrorCode::Open,
            ZipError::FileRead { .. } => ZipErrorCode::Open,
            ZipError::FileWrite { .. } => ZipErrorCode::OpenWrite,
            ZipError::OutOfMemory => ZipErrorCode::NoMem,
            ZipError::EntryNotFound { .. } => ZipErrorCode::NoEntry,
            ZipError::CorruptArchive { .. } => ZipErrorCode::Broken,
            ZipError::CorruptEntry { .. } => ZipErrorCode::BrokenEntry,
            ZipError::OverwriteConflict { .. } => ZipErrorCode::Overwrite,
            ZipError::CreateDirFailed { .. } => ZipErrorCode::CreateDir,
            ZipError::SetPermFailed { .. } => ZipErrorCode::SetPerm,
            ZipError::SetMtimeFailed { .. } => ZipErrorCode::SetMtime,
            ZipError::OpenWriteFailed { .. } => ZipErrorCode::OpenWrite,
            ZipError::OpenAppendFailed { .. } => ZipErrorCode::OpenAppend,
            ZipError::AddDirFailed { .. } => ZipErrorCode::AddDir,
            ZipError::AddFileFailed { .. } => ZipErrorCode::AddFile,
            ZipError::SetZipPermFailed { .. } => ZipErrorCode::SetZipPerm,
            ZipError::CreateFailed { .. } => ZipErrorCode::Create,
            ZipError::OpenExtractFailed { .. } => ZipErrorCode::OpenX,
            ZipError::FileSizeFailed { .. } => ZipErrorCode::FileSize,
            ZipError::UnsupportedCompression { .. } => ZipErrorCode::BrokenEntry,
            ZipError::Crc32Mismatch { .. } => ZipErrorCode::BrokenEntry,
            ZipError::InvalidPath { .. } => ZipErrorCode::OpenX,
            ZipError::CreateSymlinkFailed { .. } => ZipErrorCode::CreateLink,
            ZipError::Io(_) => ZipErrorCode::Open,
            ZipError::Generic(_) => ZipErrorCode::Create,
        }
    }

    pub fn as_c_code(&self) -> i32 {
        self.to_c_code().as_i32()
    }

    /// Create a file open error
    pub fn file_open(path: impl Into<PathBuf>, source: io::Error) -> Self {
        ZipError::FileOpen {
            path: path.into(),
            source,
        }
    }

    /// Create a file read error
    pub fn file_read(path: impl Into<PathBuf>, source: io::Error) -> Self {
        ZipError::FileRead {
            path: path.into(),
            source,
        }
    }

    /// Create a file write error
    pub fn file_write(path: impl Into<PathBuf>, source: io::Error) -> Self {
        ZipError::FileWrite {
            path: path.into(),
            source,
        }
    }

    /// Create a corrupt archive error
    pub fn corrupt_archive(archive: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        ZipError::CorruptArchive {
            archive: archive.into(),
            reason: reason.into(),
        }
    }

    /// Create a generic error
    pub fn generic(msg: impl Into<String>) -> Self {
        ZipError::Generic(msg.into())
    }
}

/// Result type for zip-rs operations
pub type Result<T> = std::result::Result<T, ZipError>;

/// Compression level (1-9)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CompressionLevel {
    NoCompression = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
    Level5 = 5,
    #[default]
    Level6 = 6,
    Level7 = 7,
    Level8 = 8,
    Level9 = 9,
}

impl CompressionLevel {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            1 => Some(CompressionLevel::Level1),
            2 => Some(CompressionLevel::Level2),
            3 => Some(CompressionLevel::Level3),
            4 => Some(CompressionLevel::Level4),
            5 => Some(CompressionLevel::Level5),
            6 => Some(CompressionLevel::Level6),
            7 => Some(CompressionLevel::Level7),
            8 => Some(CompressionLevel::Level8),
            9 => Some(CompressionLevel::Level9),
            _ => None,
        }
    }
}

/// Path mode for storing files in the archive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZipMode {
    /// Mirror mode - keep directory structure
    #[default]
    Mirror,
    /// Cherry-pick mode - flatten to root
    CherryPick,
}

/// File type in ZIP archive
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
}

impl FileType {
    pub fn from_u32(mode: u32) -> Self {
        // Unix file type from mode_t
        match mode & 0o170000 {
            0o010000 => FileType::Fifo,
            0o020000 => FileType::CharDevice,
            0o040000 => FileType::Directory,
            0o060000 => FileType::BlockDevice,
            0o100000 => FileType::File,
            0o120000 => FileType::Symlink,
            0o140000 => FileType::Socket,
            _ => FileType::File,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::File => "file",
            FileType::Directory => "directory",
            FileType::Symlink => "symlink",
            FileType::BlockDevice => "block_device",
            FileType::CharDevice => "character_device",
            FileType::Fifo => "FIFO",
            FileType::Socket => "socket",
        }
    }
}

/// ZIP entry metadata
#[derive(Debug, Clone)]
pub struct ZipEntry {
    /// File name (UTF-8)
    pub filename: String,
    /// Compressed size in bytes
    pub compressed_size: u64,
    /// Uncompressed size in bytes
    pub uncompressed_size: u64,
    /// Modification time
    pub timestamp: std::time::SystemTime,
    /// Unix permissions (mode)
    pub permissions: u32,
    /// CRC32 checksum
    pub crc32: u32,
    /// Offset of local header
    pub offset: u64,
    /// Is directory
    pub is_directory: bool,
    /// File type
    pub file_type: FileType,
    /// Is symlink
    pub is_symlink: bool,
}

impl ZipEntry {
    /// Create a new ZipEntry
    pub fn new(filename: String) -> Self {
        ZipEntry {
            filename,
            compressed_size: 0,
            uncompressed_size: 0,
            timestamp: std::time::SystemTime::now(),
            permissions: 0o644,
            crc32: 0,
            offset: 0,
            is_directory: false,
            file_type: FileType::File,
            is_symlink: false,
        }
    }

    /// Set the directory flag
    pub fn with_directory(mut self, is_dir: bool) -> Self {
        self.is_directory = is_dir;
        if is_dir {
            self.file_type = FileType::Directory;
            self.permissions = 0o755;
        }
        self
    }

    /// Set the permissions
    pub fn with_permissions(mut self, perm: u32) -> Self {
        self.permissions = perm & 0o7777;
        self
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, ts: std::time::SystemTime) -> Self {
        self.timestamp = ts;
        self
    }

    /// Set the size
    pub fn with_size(mut self, size: u64) -> Self {
        self.uncompressed_size = size;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_level() {
        assert_eq!(CompressionLevel::Level1.as_u8(), 1);
        assert_eq!(CompressionLevel::Level9.as_u8(), 9);
        assert!(CompressionLevel::from_u8(5).is_some());
        assert!(CompressionLevel::from_u8(10).is_none());
    }

    #[test]
    fn test_file_type() {
        assert_eq!(FileType::from_u32(0o100644), FileType::File);
        assert_eq!(FileType::from_u32(0o40755), FileType::Directory);
        assert_eq!(FileType::from_u32(0o120755), FileType::Symlink);
    }
}
