//! ZipProcess - ZIP 进程 API
//!
//! 通过真实进程调用 ziprs CLI 工具

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// ZIP 进程错误
#[derive(Debug, thiserror::Error)]
pub enum ZipProcessError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Process failed with exit code: {0}")]
    ExitCode(i32),
    #[error("Process timed out")]
    Timeout,
    #[error("Process already killed")]
    AlreadyKilled,
}

/// ZIP 进程
pub struct ZipProcess {
    child: Option<Child>,
    zipfile: String,
    params_file: String,
    stderr_file: String,
}

impl ZipProcess {
    /// 创建新的 ZIP 进程
    ///
    /// # 参数
    ///
    /// - `zipfile`: ZIP 文件路径
    /// - `files`: 要添加的文件列表
    /// - `recurse`: 是否递归（当前未使用，保留接口兼容性）
    /// - `include_directories`: 是否包含目录条目（当前未使用）
    pub fn new(
        zipfile: impl AsRef<Path>,
        files: &[impl AsRef<str>],
        recurse: bool,
        include_directories: bool,
    ) -> Result<Self, ZipProcessError> {
        let zipfile = zipfile.as_ref().to_string_lossy().to_string();

        // 创建临时参数文件
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .as_nanos();
        let params_file = format!("/tmp/ziprs_params_{}.params", timestamp);

        let params_file = Self::write_params_file(&params_file, files, recurse, include_directories)?;

        // 创建 stderr 文件
        let stderr_file = format!("{}.stderr", params_file);

        // 启动 ziprs 进程
        let child = Command::new("ziprs")
            .arg(&zipfile)
            .arg(&params_file)
            .stderr(Stdio::from(File::create(&stderr_file)?))
            .spawn()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to start ziprs: {:?}", e)))?;

        Ok(Self {
            child: Some(child),
            zipfile,
            params_file,
            stderr_file,
        })
    }

    /// 写入参数文件
    fn write_params_file(
        path: &str,
        files: &[impl AsRef<str>],
        _recurse: bool,
        _include_directories: bool,
    ) -> Result<String, ZipProcessError> {
        let num_files = files.len() as i32;

        // 构建键数据（使用文件名作为键）
        let keys: Vec<String> = files.iter().map(|f| f.as_ref().to_string()).collect();
        let keys_data: Vec<u8> = keys.iter()
            .flat_map(|s| {
                let mut bytes = s.as_bytes().to_vec();
                bytes.push(0); // null 终止符
                bytes
            })
            .collect();
        let keys_len = keys_data.len() as i32;

        // 构建文件名数据
        let filenames: Vec<String> = files.iter().map(|f| f.as_ref().to_string()).collect();
        let filenames_data: Vec<u8> = filenames.iter()
            .flat_map(|s| {
                let mut bytes = s.as_bytes().to_vec();
                bytes.push(0); // null 终止符
                bytes
            })
            .collect();
        let filenames_len = filenames_data.len() as i32;

        // 构建目录标志（全部为 false，因为只处理文件）
        let is_directory: Vec<u8> = files.iter().map(|_| 0u8).collect();

        // 构建修改时间（使用当前时间）
        let mtime: Vec<u8> = files.iter()
            .flat_map(|_| {
                let time = 0.0f64; // 使用占位值
                time.to_le_bytes().to_vec()
            })
            .collect();

        // 写入文件
        let mut file = File::create(path)?;
        file.write_all(&num_files.to_le_bytes())?;
        file.write_all(&keys_len.to_le_bytes())?;
        file.write_all(&keys_data)?;
        file.write_all(&filenames_len.to_le_bytes())?;
        file.write_all(&filenames_data)?;
        file.write_all(&is_directory)?;
        file.write_all(&mtime)?;
        file.flush()?;

        Ok(path.to_string())
    }

    /// 等待进程完成
    ///
    /// # 参数
    ///
    /// - `timeout_ms`: 超时时间（毫秒），None 表示无限等待
    pub fn wait(&mut self, timeout_ms: Option<u64>) -> Result<(), ZipProcessError> {
        let child = self.child.as_mut().ok_or(ZipProcessError::AlreadyKilled)?;

        if let Some(timeout) = timeout_ms {
            // 带超时的等待
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            return Ok(());
                        } else {
                            return Err(ZipProcessError::ExitCode(
                                status.code().unwrap_or(EXIT_FAILURE),
                            ));
                        }
                    }
                    Ok(None) => {
                        // 进程还在运行
                        if start.elapsed() >= Duration::from_millis(timeout) {
                            return Err(ZipProcessError::Timeout);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        return Err(ZipProcessError::Io(e));
                    }
                }
            }
        } else {
            // 无限等待
            let status = child.wait()?;
            if status.success() {
                Ok(())
            } else {
                Err(ZipProcessError::ExitCode(
                    status.code().unwrap_or(EXIT_FAILURE),
                ))
            }
        }
    }

    /// 终止进程
    pub fn kill(&mut self) -> Result<(), ZipProcessError> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            Ok(())
        } else {
            Err(ZipProcessError::AlreadyKilled)
        }
    }

    /// 获取退出状态
    pub fn get_exit_status(&mut self) -> Option<i32> {
        self.child.as_mut().and_then(|c| c.try_wait().ok()).flatten().map(|s| s.code().unwrap_or(EXIT_FAILURE))
    }
}

const EXIT_FAILURE: i32 = 1;

impl Drop for ZipProcess {
    fn drop(&mut self) {
        // 清理临时文件
        let _ = std::fs::remove_file(&self.params_file);
        let _ = std::fs::remove_file(&self.stderr_file);

        // 如果进程还在运行，尝试终止它
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}
