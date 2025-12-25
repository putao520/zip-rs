# 进程API架构设计

## 版本信息
- **文档版本**: v1.0.0
- **最后更新**: 2025-12-24
- **状态**: Active
- **关联SPEC**: SPEC/06-TESTING-STRATEGY.md v2.0.0

---

## 概述

本文档定义 zip-rs 的进程API架构，用于100%精确复刻C版本的测试行为。进程API通过真实进程调用CLI工具，模拟C版本的 `zip_process()` 和 `unzip_process()` 行为。

---

## C版本实现分析

### C版本进程API

**R包实现位置**: `R/process.R`

**核心特性**:

| 特性 | C版本实现 |
|------|----------|
| **基类** | processx::process |
| **进程创建** | 调用外部可执行文件（cmdzip/cmdunzip） |
| **参数传递** | 通过二进制参数文件 |
| **通信方式** | 命令行参数 + 退出码 + stderr文件 |

**zip_process结构**:

| 字段 | 类型 | 说明 |
|------|------|------|
| zipfile | 字符串 | ZIP文件路径 |
| files | 字符串向量 | 要添加的文件/目录 |
| recurse | 逻辑值 | 是否递归 |
| include_directories | 逻辑值 | 是否包含目录条目 |
| params_file | 字符串 | 参数文件路径 |

**方法签名**:

```
zip_process()$new(zipfile, files, recurse = TRUE,
                   include_directories = TRUE,
                   poll_connection = TRUE,
                   stderr = tempfile(), ...)

unzip_process()$new(zipfile, exdir = ".",
                     poll_connection = TRUE,
                     stderr = tempfile(), ...)
```

### C版本参数文件格式

**写入函数**: `write_zip_params(files, recurse, include_directories, outfile)`

**二进制格式**:

| 字段 | 类型 | 说明 |
|------|------|------|
| 文件数量 | integer | 4字节，文件总数 |
| 键总长度 | integer | 4字节，所有key长度+1 |
| 键数据 | 字符串向量 | 以null分隔 |
| 文件名总长度 | integer | 4字节，所有文件名长度+1 |
| 文件名数据 | 字符串向量 | 以null分隔 |
| 目录标志 | 逻辑向量 | 每个文件是否为目录 |
| 修改时间 | double | 文件修改时间戳 |

---

## Rust版本架构

### 架构组件

```
┌─────────────────────────────────────────────────────────────┐
│                        测试代码                              │
│  tests/zip_process.rs, tests/unzip_process.rs              │
└────────────────────────┬────────────────────────────────────┘
                         │ 调用
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                     进程API层                                │
│  src/process/zip.rs, src/process/unzip.rs                   │
│  - ZipProcess::new()                                        │
│  - UnzipProcess::new()                                      │
│  - wait(), kill(), get_exit_status()                        │
└────────────────────────┬────────────────────────────────────┘
                         │ 启动进程
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      CLI工具层                               │
│  src/bin/ziprs.rs, src/bin/unziprs.rs                       │
│  - 命令行参数解析                                            │
│  - 调用核心库函数                                            │
└────────────────────────┬────────────────────────────────────┘
                         │ 使用
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      核心库层                                │
│  src/zip/, src/unzip/, src/miniz/                           │
└─────────────────────────────────────────────────────────────┘
```

### 进程API规范

**ZipProcess结构**:

| 字段 | 类型 | 说明 | C版本对应 |
|------|------|------|----------|
| child | Option<Child> | 子进程句柄 | processx::process |
| zipfile | PathBuf | ZIP文件路径 | zipfile |
| params_file | PathBuf | 参数文件路径 | params_file |
| stderr_file | PathBuf | 错误输出文件 | stderr |

**ZipProcess方法**:

| 方法 | 参数 | 返回值 | 说明 | C版本对应 |
|------|------|--------|------|----------|
| new | zipfile, files, options | Result<Self> | 创建ZIP进程 | $new() |
| wait | timeout_ms | Result<ExitStatus> | 等待完成 | $wait() |
| kill | - | Result<()> | 终止进程 | $kill() |
| get_exit_status | - | Option<i32> | 获取退出码 | $get_exit_status() |

**UnzipProcess结构**:

| 字段 | 类型 | 说明 | C版本对应 |
|------|------|------|----------|
| child | Option<Child> | 子进程句柄 | processx::process |
| zipfile | PathBuf | ZIP文件路径 | zipfile |
| exdir | PathBuf | 解压目录 | exdir |
| stderr_file | PathBuf | 错误输出文件 | stderr |

**UnzipProcess方法**:

| 方法 | 参数 | 返回值 | 说明 | C版本对应 |
|------|------|--------|------|----------|
| new | zipfile, exdir | Result<Self> | 创建UNZIP进程 | $new() |
| wait | timeout_ms | Result<ExitStatus> | 等待完成 | $wait() |
| kill | - | Result<()> | 终止进程 | $kill() |
| get_exit_status | - | Option<i32> | 获取退出码 | $get_exit_status() |

---

## CLI工具规范

### ziprs命令行工具

**文件位置**: `src/bin/ziprs.rs`

**命令格式**:
```
ziprs <zipfile> <params-file>
```

**参数说明**:

| 参数 | 类型 | 说明 | C版本对应 |
|------|------|------|----------|
| zipfile | 字符串 | ZIP文件路径 | 第一个参数 |
| params-file | 字符串 | 参数文件路径 | 第二个参数 |

**退出码**:

| 退出码 | 说明 | C版本对应 |
|--------|------|----------|
| 0 | 成功 | 0L |
| 非0 | 失败 | 错误状态 |

**参数文件读取**:

| 步骤 | 操作 |
|------|------|
| 1 | 读取文件数量（4字节，integer） |
| 2 | 读取键总长度（4字节） |
| 3 | 读取键数据（null分隔字符串） |
| 4 | 读取文件名总长度（4字节） |
| 5 | 读取文件名数据（null分隔字符串） |
| 6 | 读取目录标志（逻辑向量） |
| 7 | 读取修改时间（double） |

### unziprs命令行工具

**文件位置**: `src/bin/unziprs.rs`

**命令格式**:
```
unziprs <zipfile> <exdir>
```

**参数说明**:

| 参数 | 类型 | 说明 | C版本对应 |
|------|------|------|----------|
| zipfile | 字符串 | ZIP文件路径 | 第一个参数 |
| exdir | 字符串 | 解压目录 | 第二个参数 |

**退出码**:

| 退出码 | 说明 | C版本对应 |
|--------|------|----------|
| 0 | 成功 | 0L |
| 非0 | 失败 | 错误状态 |

---

## 进程启动流程

### ZipProcess启动流程

```
1. 创建参数文件
   ↓
2. 写入二进制参数数据（文件列表、标志等）
   ↓
3. 启动ziprs进程
   Command::new("ziprs")
       .arg(zipfile)
       .arg(params_file)
       .stderr(Stdio::from(stderr_file))
       .spawn()
   ↓
4. 保存进程句柄
```

### UnzipProcess启动流程

```
1. 确保exdir存在（如不存在则创建）
   ↓
2. 启动unziprs进程
   Command::new("unziprs")
       .arg(zipfile)
       .arg(exdir)
       .stderr(Stdio::from(stderr_file))
       .spawn()
   ↓
3. 保存进程句柄
```

---

## 参数文件格式详解

### 二进制布局

**文件结构**:

```
┌─────────────────────────────────────────────────────────┐
│ 文件数量 (4 bytes, i32, little-endian)                  │
├─────────────────────────────────────────────────────────┤
│ 键总长度 (4 bytes, i32, little-endian)                  │
├─────────────────────────────────────────────────────────┤
│ 键数据 (n bytes, null-terminated strings)               │
├─────────────────────────────────────────────────────────┤
│ 文件名总长度 (4 bytes, i32, little-endian)              │
├─────────────────────────────────────────────────────────┤
│ 文件名数据 (m bytes, null-terminated strings)           │
├─────────────────────────────────────────────────────────┤
│ 目录标志 (n bytes, bool vector)                         │
├─────────────────────────────────────────────────────────┤
│ 修改时间 (n * 8 bytes, f64 vector)                      │
└─────────────────────────────────────────────────────────┘
```

**数据类型**:

| 类型 | 大小 | 字节序 | 说明 |
|------|------|--------|------|
| i32 | 4字节 | little-endian | 整数 |
| f64 | 8字节 | little-endian | 双精度浮点 |
| bool | 1字节 | - | 布尔值 |
| string | 变长 | - | null结尾字符串 |

**示例数据**:

假设有2个文件：
- file1.txt (不是目录)
- dir/file2.txt (dir是目录，file2.txt在dir中)

```
文件数量: [02 00 00 00]
键总长度: [15 00 00 00]
键数据: "file1.txt\0dir\0" (15字节)
文件名总长度: [21 00 00 00]
文件名数据: "file1.txt\0dir\0dir/file2.txt\0" (21字节)
目录标志: [00 01 00]
修改时间: [f64_1, f64_2, f64_3]
```

---

## 错误处理

### 进程启动错误

| 错误类型 | 返回值 | 说明 |
|---------|--------|------|
| 可执行文件不存在 | Err(io::Error) | 无法启动进程 |
| 参数文件创建失败 | Err(io::Error) | 无法写入参数 |
| 权限不足 | Err(io::Error) | 无法执行CLI |

### 进程执行错误

| 错误类型 | 检测方式 | 说明 |
|---------|---------|------|
| 非零退出码 | get_exit_status() != Some(0) | CLI执行失败 |
| 超时 | wait() 返回超时错误 | 进程未在规定时间完成 |
| 进程崩溃 | get_exit_status() 通过信号 | 异常终止 |

### stderr文件

| 用途 | 说明 |
|------|------|
| 错误诊断 | 进程失败时查看错误信息 |
| 调试 | 开发时诊断问题 |
| 测试验证 | 某些测试验证错误消息 |

---

## 实现检查清单

### ZipProcess实现

- [ ] new() 方法：创建参数文件并启动进程
- [ ] wait() 方法：等待进程完成
- [ ] kill() 方法：终止进程
- [ ] get_exit_status() 方法：获取退出码
- [ ] Drop trait：自动清理资源

### UnzipProcess实现

- [ ] new() 方法：创建解压目录并启动进程
- [ ] wait() 方法：等待进程完成
- [ ] kill() 方法：终止进程
- [ ] get_exit_status() 方法：获取退出码
- [ ] Drop trait：自动清理资源

### ziprs CLI实现

- [ ] 命令行参数解析
- [ ] 参数文件读取（二进制格式）
- [ ] 调用核心ZIP功能
- [ ] 退出码设置
- [ ] 错误处理

### unziprs CLI实现

- [ ] 命令行参数解析
- [ ] 调用核心UNZIP功能
- [ ] 退出码设置
- [ ] 错误处理

---

## 测试覆盖

### 单元测试

| 测试项 | 说明 |
|--------|------|
| 参数文件写入 | 验证二进制格式正确 |
| 进程启动 | 验证可执行文件正确启动 |
| 进程等待 | 验证wait()正确等待 |
| 进程终止 | 验证kill()正确终止 |
| 退出码获取 | 验证get_exit_status()正确返回 |

### 集成测试

| 测试项 | 说明 |
|--------|------|
| 完整ZIP流程 | 从参数文件到ZIP创建 |
| 完整UNZIP流程 | 从ZIP文件到解压 |
| 错误处理 | 各种错误场景 |
| 并发进程 | 多个进程同时运行 |

---

## 版本历史

| 版本 | 日期 | 变更说明 |
|------|------|---------|
| v1.0.0 | 2025-12-24 | 初始版本，定义进程API架构 |
