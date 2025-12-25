//! UnzipProcess - UNZIP 进程 API
//!
//! 通过真实进程调用 unziprs CLI 工具

use std::fs::File;
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// UNZIP 进程错误
#[derive(Debug, thiserror::Error)]
pub enum UnzipProcessError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Process failed with exit code: {0}")]
    ExitCode(i32),
    #[error("Process timed out")]
    Timeout,
    #[error("Process already killed")]
    AlreadyKilled,
}

/// UNZIP 进程
pub struct UnzipProcess {
    child: Option<Child>,
    zipfile: String,
    exdir: String,
    stderr_file: String,
}

impl UnzipProcess {
    /// 创建新的 UNZIP 进程
    ///
    /// # 参数
    ///
    /// - `zipfile`: ZIP 文件路径
    /// - `exdir`: 解压目录
    pub fn new(zipfile: impl AsRef<Path>, exdir: impl AsRef<Path>) -> Result<Self, UnzipProcessError> {
        let zipfile = zipfile.as_ref().to_string_lossy().to_string();
        let exdir = exdir.as_ref().to_string_lossy().to_string();

        // 创建 stderr 文件
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .as_nanos();
        let stderr_file = format!("/tmp/unziprs_{}.stderr", timestamp);

        // 启动 unziprs 进程
        let child = Command::new("unziprs")
            .arg(&zipfile)
            .arg(&exdir)
            .stderr(Stdio::from(File::create(&stderr_file)?))
            .spawn()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to start unziprs: {:?}", e)))?;

        Ok(Self {
            child: Some(child),
            zipfile,
            exdir,
            stderr_file,
        })
    }

    /// 等待进程完成
    ///
    /// # 参数
    ///
    /// - `timeout_ms`: 超时时间（毫秒），None 表示无限等待
    pub fn wait(&mut self, timeout_ms: Option<u64>) -> Result<(), UnzipProcessError> {
        let child = self.child.as_mut().ok_or(UnzipProcessError::AlreadyKilled)?;

        if let Some(timeout) = timeout_ms {
            // 带超时的等待
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            return Ok(());
                        } else {
                            return Err(UnzipProcessError::ExitCode(
                                status.code().unwrap_or(EXIT_FAILURE),
                            ));
                        }
                    }
                    Ok(None) => {
                        // 进程还在运行
                        if start.elapsed() >= Duration::from_millis(timeout) {
                            return Err(UnzipProcessError::Timeout);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        return Err(UnzipProcessError::Io(e));
                    }
                }
            }
        } else {
            // 无限等待
            let status = child.wait()?;
            if status.success() {
                Ok(())
            } else {
                Err(UnzipProcessError::ExitCode(
                    status.code().unwrap_or(EXIT_FAILURE),
                ))
            }
        }
    }

    /// 终止进程
    pub fn kill(&mut self) -> Result<(), UnzipProcessError> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            Ok(())
        } else {
            Err(UnzipProcessError::AlreadyKilled)
        }
    }

    /// 获取退出状态
    pub fn get_exit_status(&mut self) -> Option<i32> {
        self.child.as_mut().and_then(|c| c.try_wait().ok()).flatten().map(|s| s.code().unwrap_or(EXIT_FAILURE))
    }
}

const EXIT_FAILURE: i32 = 1;

impl Drop for UnzipProcess {
    fn drop(&mut self) {
        // 清理临时文件
        let _ = std::fs::remove_file(&self.stderr_file);

        // 如果进程还在运行，尝试终止它
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}
