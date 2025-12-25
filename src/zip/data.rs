use crate::error::{Result, ZipError, ZipMode};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ZipDataEntry {
    pub key: String,
    pub file: PathBuf,
    pub dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipWarning {
    DirectoriesIgnored,
    DroppedLeadingSlash,
    DotSlashPaths,
    DotDotPaths,
    ColonPaths,
}

#[derive(Debug, Clone)]
pub struct ZipData {
    pub entries: Vec<ZipDataEntry>,
    pub warnings: Vec<ZipWarning>,
}

pub fn get_zip_data(
    files: &[String],
    recurse: bool,
    mode: ZipMode,
    include_directories: bool,
    root: &Path,
) -> Result<ZipData> {
    let mut warnings = Vec::new();
    let mut entries = if mode == ZipMode::Mirror {
        get_zip_data_path(files, recurse, root, &mut warnings)?
    } else {
        get_zip_data_nopath(files, recurse, root, &mut warnings)?
    };

    if !include_directories {
        entries.retain(|entry| !entry.dir);
    }

    apply_key_warnings(&mut entries, &mut warnings);

    Ok(ZipData { entries, warnings })
}

fn get_zip_data_path(
    files: &[String],
    recurse: bool,
    root: &Path,
    warnings: &mut Vec<ZipWarning>,
) -> Result<Vec<ZipDataEntry>> {
    if recurse && !files.is_empty() {
        let mut entries = Vec::new();
        for file in files {
            entries.extend(get_zip_data_path_recursive(file, root)?);
        }
        dedup_by_file(entries)
    } else {
        let filtered = ignore_dirs_with_warning(files, root, warnings)?;
        Ok(filtered
            .into_iter()
            .map(|file| ZipDataEntry {
                key: file.clone(),
                file: resolve_path(root, &file),
                dir: false,
            })
            .collect())
    }
}

fn get_zip_data_nopath(
    files: &[String],
    recurse: bool,
    root: &Path,
    warnings: &mut Vec<ZipWarning>,
) -> Result<Vec<ZipDataEntry>> {
    let files = expand_dot_in_nopath(files, root)?;

    if recurse && !files.is_empty() {
        let mut entries = Vec::new();
        for file in files {
            entries.extend(get_zip_data_nopath_recursive(&file, root)?);
        }
        dedup_by_file(entries)
    } else {
        let filtered = ignore_dirs_with_warning(&files, root, warnings)?;
        Ok(filtered
            .into_iter()
            .map(|file| ZipDataEntry {
                key: basename(&file),
                file: resolve_path(root, &file),
                dir: false,
            })
            .collect())
    }
}

fn get_zip_data_path_recursive(x: &str, root: &Path) -> Result<Vec<ZipDataEntry>> {
    let path = resolve_path(root, x);
    let meta = fs::metadata(&path).map_err(|e| ZipError::file_open(&path, e))?;

    if meta.is_dir() {
        let mut entries = Vec::new();
        entries.push(ZipDataEntry {
            key: ensure_dir_suffix(x),
            file: normalize_path(&path)?,
            dir: true,
        });

        let mut children = list_dir_recursive(&path)?;
        children.sort();
        for child in children {
            let rel = child.strip_prefix(&path).unwrap_or(&child);
            let rel_key = join_key(x, rel);
            let is_dir = fs::metadata(&child)
                .map(|m| m.is_dir())
                .unwrap_or(false);
            entries.push(ZipDataEntry {
                key: if is_dir {
                    ensure_dir_suffix(&rel_key)
                } else {
                    rel_key
                },
                file: normalize_path(&child)?,
                dir: is_dir,
            });
        }
        Ok(entries)
    } else {
        Ok(vec![ZipDataEntry {
            key: x.to_string(),
            file: normalize_path(&path)?,
            dir: false,
        }])
    }
}

fn get_zip_data_nopath_recursive(x: &str, root: &Path) -> Result<Vec<ZipDataEntry>> {
    let expanded = expand_dot_single(x, root)?;
    let mut entries = Vec::new();
    for item in expanded {
        let path = resolve_path(root, &item);
        let meta = fs::metadata(&path).map_err(|e| ZipError::file_open(&path, e))?;
        if meta.is_dir() {
            let base = basename(&item);
            entries.push(ZipDataEntry {
                key: ensure_dir_suffix(&base),
                file: normalize_path(&path)?,
                dir: true,
            });

            let mut children = list_dir_recursive(&path)?;
            children.sort();
            for child in children {
                let rel = child.strip_prefix(&path).unwrap_or(&child);
                let rel_key = join_key(&base, rel);
                let is_dir = fs::metadata(&child)
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                entries.push(ZipDataEntry {
                    key: if is_dir {
                        ensure_dir_suffix(&rel_key)
                    } else {
                        rel_key
                    },
                    file: normalize_path(&child)?,
                    dir: is_dir,
                });
            }
        } else {
            entries.push(ZipDataEntry {
                key: basename(&item),
                file: normalize_path(&path)?,
                dir: false,
            });
        }
    }
    Ok(entries)
}

fn expand_dot_in_nopath(files: &[String], root: &Path) -> Result<Vec<String>> {
    if files.iter().any(|f| f == ".") {
        let mut out = Vec::new();
        for file in files {
            if file == "." {
                let entries = fs::read_dir(root).map_err(|e| ZipError::file_open(root, e))?;
                for entry in entries {
                    let entry = entry.map_err(|e| ZipError::file_read(root, e))?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name != "." && name != ".." {
                        out.push(name);
                    }
                }
            } else {
                out.push(file.clone());
            }
        }
        Ok(out)
    } else {
        Ok(files.to_vec())
    }
}

fn expand_dot_single(x: &str, root: &Path) -> Result<Vec<String>> {
    if x == "." {
        let mut out = Vec::new();
        let entries = fs::read_dir(root).map_err(|e| ZipError::file_open(root, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| ZipError::file_read(root, e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "." && name != ".." {
                out.push(name);
            }
        }
        Ok(out)
    } else {
        Ok(vec![x.to_string()])
    }
}

fn ignore_dirs_with_warning(
    files: &[String],
    root: &Path,
    warnings: &mut Vec<ZipWarning>,
) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut saw_dir = false;
    for file in files {
        let path = resolve_path(root, file);
        let meta = fs::metadata(&path).map_err(|e| ZipError::file_open(&path, e))?;
        if meta.is_dir() {
            saw_dir = true;
        } else {
            result.push(file.clone());
        }
    }
    if saw_dir && !warnings.contains(&ZipWarning::DirectoriesIgnored) {
        warnings.push(ZipWarning::DirectoriesIgnored);
    }
    Ok(result)
}

fn apply_key_warnings(entries: &mut [ZipDataEntry], warnings: &mut Vec<ZipWarning>) {
    let mut dropped = false;
    let mut dot_slash = false;
    let mut dotdot = false;
    let mut colon = false;

    for entry in entries.iter_mut() {
        if entry.key.starts_with('/') {
            entry.key = entry.key.trim_start_matches('/').to_string();
            dropped = true;
        }
        if entry.key.starts_with("./") || entry.key.starts_with(".\\") {
            dot_slash = true;
        }
        if entry.key.starts_with("../") || entry.key.starts_with("..\\") {
            dotdot = true;
        }
        if entry.key.contains(':') {
            colon = true;
        }
    }

    if dropped && !warnings.contains(&ZipWarning::DroppedLeadingSlash) {
        warnings.push(ZipWarning::DroppedLeadingSlash);
    }
    if dot_slash && !warnings.contains(&ZipWarning::DotSlashPaths) {
        warnings.push(ZipWarning::DotSlashPaths);
    }
    if dotdot && !warnings.contains(&ZipWarning::DotDotPaths) {
        warnings.push(ZipWarning::DotDotPaths);
    }
    if colon && !warnings.contains(&ZipWarning::ColonPaths) {
        warnings.push(ZipWarning::ColonPaths);
    }
}

fn resolve_path(root: &Path, file: &str) -> PathBuf {
    let path = Path::new(file);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|e| ZipError::file_open(path, e))
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

fn ensure_dir_suffix(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

fn join_key(base: &str, rel: &Path) -> String {
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if base.ends_with('/') {
        format!("{base}{rel_str}")
    } else {
        format!("{base}/{rel_str}")
    }
}

fn list_dir_recursive(path: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(path).map_err(|e| ZipError::file_read(path, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| ZipError::file_read(path, e))?;
        let entry_path = entry.path();
        out.push(entry_path.clone());
        if entry_path.is_dir() {
            out.extend(list_dir_recursive(&entry_path)?);
        }
    }
    Ok(out)
}

fn dedup_by_file(entries: Vec<ZipDataEntry>) -> Result<Vec<ZipDataEntry>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in entries {
        if seen.insert(entry.file.clone()) {
            out.push(entry);
        }
    }
    Ok(out)
}
