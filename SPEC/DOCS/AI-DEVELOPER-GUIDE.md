# AI-CLi开发者指南

## 项目概述

zip-rs是zip R包的Rust版本，目标100%精确复刻C版本的所有测试用例。

## 关键约束

1. **SPEC权威原则**：所有实现必须严格符合SPEC定义，禁止偏离
2. **禁止FFI**：测试必须纯Rust实现，禁止通过FFI调用C函数验证
3. **测试铁律**：禁止修改测试期望值，禁止占位符，禁止虚假通过
4. **完整交付**：一次性实现所有功能，禁止"先做简单版"

## 项目目录结构

```
zip-rs/
├── src/
│   ├── bin/                    # CLI工具（需要重写）
│   │   ├── cmdzip.rs          # ❌ 需改为ziprs.rs
│   │   └── cmdunzip.rs        # ❌ 需改为unziprs.rs
│   ├── lib.rs                  # 库入口
│   ├── miniz/                  # DEFLATE/INFLATE算法（需要扩展签名）
│   │   ├── mod.rs
│   │   ├── inflate.rs         # 需要添加pos/size参数
│   │   └── deflate.rs          # 需要添加pos/size/level参数
│   ├── zip/                    # ZIP功能（直接复用）
│   ├── unzip/                  # UNZIP功能（直接复用）
│   ├── error.rs                # 错误处理（直接复用）
│   └── platform/               # 平台抽象（直接复用）
├── tests/
│   ├── common.rs               # 测试辅助（需要扩展）
│   ├── snapshots/              # 快照目录（需要创建）
│   └── *.rs                     # 14个测试文件
├── SPEC/
│   ├── 06-TESTING-STRATEGY.md   # 测试策略
│   └── DOCS/
│       ├── PROCESS-API.md       # 进程API设计
│       ├── SNAPSHOT-TESTING.md   # 快照测试规范
│       └── AI-DEVELOPER-GUIDE.md # 本文档
└── Cargo.toml                  # 需要添加insta依赖
```

## 可复用模块清单

以下模块已完全实现，可直接复用：

### src/zip/
- ZipBuilder - ZIP创建构建器
- list() - 列出ZIP内容
- 核心ZIP功能已完整

### src/unzip/
- extract() - 解压ZIP文件
- 核心UNZIP功能已完整

### src/error.rs
- 18种错误码定义
- C/Rust双向转换

### src/miniz/
- DEFLATE/INFLATE算法核心
- CRC32、Adler32计算

## 需要新建/重写的模块

### 1. CLI工具（src/bin/）

**当前问题**：
- cmdzip.rs不符合C版本的参数文件协议
- 需要重写为ziprs.rs和unziprs.rs

**C版本参考**：
- /home/putao/code/c-cpp/zip/src/tools/cmdzip.c
- /home/putao/code/c-cpp/zip/src/tools/cmdunzip.c

**参数文件协议**（二进制格式）：
```
文件数量 (4 bytes, i32, little-endian)
键总长度 (4 bytes)
键数据 (n bytes, null-terminated strings)
文件名总长度 (4 bytes)
文件名数据 (m bytes, null-terminated strings)
目录标志 (n bytes, bool vector)
修改时间 (n * 8 bytes, f64 vector)
```

### 2. 进程API（src/process/）

**需要新建**：
- src/process/mod.rs
- src/process/zip.rs - ZipProcess
- src/process/unzip.rs - UnzipProcess

**C版本参考**：
- /home/putao/code/c-cpp/zip/R/process.R

**核心方法**：
- new() - 创建进程
- wait(timeout) - 等待完成
- kill() - 终止进程
- get_exit_status() - 获取退出码

### 3. 函数签名更新（src/miniz/）

**当前签名**：
```rust
pub fn decompress(data: &[u8]) -> Result<Vec<u8>>
pub fn compress(data: &[u8], level: u8) -> Result<Vec<u8>>
```

**需要改为**：
```rust
pub fn decompress(data: &[u8], pos: i32, size: Option<i32>) -> Result<InflateOutput>
pub fn compress(data: &[u8], level: i32, pos: i32, size: Option<i32>) -> Result<DeflateOutput>

pub struct InflateOutput {
    pub output: Vec<u8>,
    pub bytes_read: i32,
    pub bytes_written: i32,
}

pub struct DeflateOutput {
    pub output: Vec<u8>,
    pub bytes_read: i32,
    pub bytes_written: i32,
}
```

### 4. 快照测试（tests/）

**需要添加**：
- Cargo.toml添加insta依赖
- 创建.config/insta.yaml
- 创建tests/snapshots/目录
- 更新所有测试使用assert_snapshot!

**C版本参考**：
- /home/putao/code/c-cpp/zip/tests/testthat/ - 查看expect_snapshot用法

## 辅助函数实现

### tests/common.rs需要添加

**C版本对应**：
- /home/putao/code/c-cpp/zip/tests/testthat/helper.R

**需要实现**：
```rust
pub fn test_temp_file() -> TempDir  // 对应test_temp_file()
pub fn test_temp_dir() -> TempDir   // 对应test_temp_dir()
pub fn make_a_zip() -> ZipData      // 对应make_a_zip()
pub fn normalize_temp_paths(s: String) -> String  // 对应transform_tempdir()
```

## 实现顺序建议

1. **先实现基础架构**：
   - 创建src/process/模块
   - 更新函数签名

2. **再实现CLI工具**：
   - ziprs.rs读取参数文件
   - unziprs.rs基本功能

3. **最后集成测试**：
   - 配置insta
   - 转换所有测试为快照格式
   - 实现辅助函数

## 关键注意事项

1. **不要修改测试期望值**：测试失败 = 代码实现有问题
2. **不要使用FFI**：纯Rust实现
3. **不要做简化版**：必须一次性完整实现
4. **参考C版本**：遇到问题时先看C版本对应代码

## 相关文件路径

- SPEC目录：./SPEC/
- C版本测试：/home/putao/code/c-cpp/zip/tests/testthat/
- C版本进程API：/home/putao/code/c-cpp/zip/R/process.R
- C版本helper：/home/putao/code/c-cpp/zip/tests/testthat/helper.R
- C版本inflate：/home/putao/code/c-cpp/zip/R/inflate.R
