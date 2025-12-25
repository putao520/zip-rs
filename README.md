# zip-rs: Pure Rust ZIP Library

> **⚠️ 注意**: 这是一个基于 **AI 半自动化**的 C 到 Rust 语言重写研究项目，展示如何利用 AI 工具进行语义等价的跨语言代码迁移。

## 项目概述

`zip-rs` 是 R 语言 [zip](https://github.com/r-lib/zip) 包的纯 Rust 重写版本，提供 ZIP 文件的创建、读取和操作功能。

### 研究背景

本项目是一个**AI 辅助的代码重写研究案例**，目标是验证：
- ✅ AI 能否理解复杂的 C 语言代码库（包括 miniz 压缩算法）
- ✅ AI 能否生成语义等价的 Rust 实现
- ✅ AI 能否保证 100% 的测试一致性
- ✅ 如何建立系统化的跨语言重写流程

### 核心成果

- ✅ **100% 测试一致性**: 68 个测试用例与 C 版本完全对等
- ✅ **1:1 语义忠实**: 所有功能行为与 C 版本完全一致
- ✅ **纯 Rust 实现**: 无任何 C 依赖或 FFI 调用
- ✅ **包含 miniz**: 完整重写了 miniz 压缩/解压缩算法

## 功能特性

- ✅ 创建 ZIP 归档文件
- ✅ 读取和列出 ZIP 文件内容
- ✅ 追加文件到现有 ZIP
- ✅ 递归目录处理
- ✅ 压缩级别控制 (0-9)
- ✅ Unix 权限保留
- ✅ 符号链接支持（Unix）
- ✅ 路径安全验证
- ✅ ZIP64 支持（大文件）

## 编译 Rust 版本

### 前置要求

- Rust 工具链 (>= 1.70)
- Cargo 包管理器

### 编译步骤

```bash
# 1. 克隆仓库
git clone https://github.com/YOUR_USERNAME/zip-rs.git
cd zip-rs

# 2. 编译库
cargo build --release

# 3. 编译命令行工具
cargo build --release --bin ziprs
cargo build --release --bin unziprs
```

### 编译输出

编译完成后，二进制文件位于：
- `target/release/ziprs` - ZIP 创建工具
- `target/release/unziprs` - ZIP 解压工具
- `target/release/libzip_rs.rlib` - Rust 库文件

## 测试 Rust 版本

### 运行所有测试

```bash
# 运行完整测试套件
cargo test

# 运行测试并显示输出
cargo test -- --nocapture

# 运行特定测试文件
cargo test --test zipr
```

### 测试覆盖

| 测试类别 | 测试数 | 说明 |
|---------|--------|------|
| 集成测试 | 68 | 与 C 版本 100% 对等 |
| 单元测试 | 30 | 核心算法测试 |
| 文档测试 | 4 | API 示例测试 |
| **总计** | **102** | ✅ 全部通过 |

### 关键测试文件

| 测试文件 | 对应 C 版本 | 测试数 |
|---------|------------|--------|
| zip.rs | test-zip.R | 14 |
| unzip.rs | test-unzip.R | 9 |
| zipr.rs | test-zipr.R | 13 |
| errors.rs | test-errors.R | 8 |
| paths.rs | test-paths.R | 6 |
| inflate.rs | test-inflate.R | 2 |
| large_files.rs | test-large-files.R | 3 |
| weird_paths.rs | test-weird-paths.R | 4 |
| zip_process.rs | test-zip-process.R | 2 |
| unzip_process.rs | test-unzip-process.R | 1 |
| get_zip_data.rs | test-get-zip-data.R | 1 |
| get_zip_data_path.rs | test-get-zip-data-path.R | 2 |
| zip_list.rs | test-zip-list.R | 2 |
| special_dot.rs | test-special-dot.R | 1 |

## 使用示例

### 创建 ZIP 文件

```bash
# 压缩单个文件
./target/release/ziprs archive.zip file1.txt file2.txt

# 压缩目录（递归）
./target/release/ziprs archive.zip my_directory/

# 指定压缩级别（0-9，0=无压缩，9=最高压缩）
./target/release/ziprs -l 9 archive.zip my_files/

# 追加文件到现有 ZIP
./target/release/ziprs -a archive.zip additional_file.txt
```

### 解压 ZIP 文件

```bash
# 解压到当前目录
./target/release/unziprs archive.zip

# 解压到指定目录
./target/release/unziprs archive.zip -d target_directory

# 列出 ZIP 内容（不解压）
./target/release/unziprs -l archive.zip
```

### Rust API 使用

```rust
use zip_rs::{ZipBuilder, list, CompressionLevel};

// 创建 ZIP
ZipBuilder::new("archive.zip")?
    .compression_level(CompressionLevel::Level6)
    .recurse(true)
    .root(".")
    .files(&["file1.txt", "file2.txt"])?
    .build()?;

// 列出 ZIP 内容
let entries = list("archive.zip")?;
for entry in entries {
    println!("{} ({} bytes)", entry.filename, entry.uncompressed_size);
}
```

## AI 重写方法论

本项目采用了一套**系统化的 AI 辅助重写流程**：

### 1. 语义理解优先

- ❌ **不使用**代码转换工具（c2rust 等）
- ✅ **人工+AI**逐函数理解 C 代码语义
- ✅ **手动**设计 Rust 对应的数据结构
- ✅ **验证**理解正确性（通过测试）

### 2. 测试驱动验证

- ✅ **100% 测试对等**: 每个 C 测试都有对应 Rust 测试
- ✅ **测试数据一致性**: 字面量完全相同
- ✅ **断言逻辑对等**: 验证标准完全一致
- ✅ **禁止简化测试**: 不允许"差不多就行"

### 3. 渐进式实现

```
理解 C 函数 → 设计 Rust API → 编写测试 → 实现功能 → 测试验证 → 修复 bug
```

每个循环都保证测试通过，不积累技术债务。

### 4. 质量铁律

- **禁止占位符**: 无 `todo!()`, `unimplemented!()`, `TODO` 注释
- **禁止简化版本**: 要么完整实现，要么不实现
- **禁止偏离行为**: 任何与 C 版本的行为差异都是 bug
- **禁止 FFI**: 纯 Rust 实现，通过测试验证一致性

## 项目结构

```
zip-rs/
├── src/
│   ├── bin/            # 命令行工具
│   │   ├── ziprs.rs    # ZIP 创建工具
│   │   └── unziprs.rs  # ZIP 解压工具
│   ├── miniz/          # miniz 压缩算法重写
│   │   ├── deflate.rs  # DEFLATE 压缩
│   │   ├── inflate.rs  # INFLATE 解压
│   │   ├── huffman.rs  # Huffman 编码
│   │   └── ...
│   ├── zip/            # ZIP 写入功能
│   │   ├── builder.rs  # Builder API
│   │   ├── writer.rs   # ZIP writer
│   │   ├── data.rs     # 数据收集
│   │   └── reader.rs   # ZIP reader
│   ├── unzip/          # ZIP 解压功能
│   │   ├── archive.rs  # 解压逻辑
│   │   └── extractor.rs
│   ├── error.rs        # 错误类型
│   └── lib.rs          # 库入口
├── tests/              # 集成测试（与 C 版本对等）
├── SPEC/               # 项目规范文档
├── examples/           # 使用示例
└── Cargo.toml          # 项目配置
```

## 与 C 版本的关系

### C 版本（参考实现）

- **仓库**: [r-lib/zip](https://github.com/r-lib/zip)
- **语言**: C + R
- **核心依赖**: miniz (C 压缩库)
- **测试**: 68 个 R 测试用例

### Rust 版本（本项目）

- **语言**: 纯 Rust
- **核心实现**: 完全重写 miniz + zip 功能
- **测试**: 68 个 Rust 测试用例（100% 对等）
- **目标**: 语义等价，无 C 依赖

### 对比总结

| 特性 | C 版本 | Rust 版本 |
|------|--------|-----------|
| 语言 | C + R | Rust |
| 测试数量 | 68 | 68 (100%) |
| miniz | C 库 | Rust 重写 |
| 内存安全 | 手动管理 | 编译时保证 |
| 跨平台 | 需编译 | 二进制分发 |
| 测试一致性 | 基准 | 100% 对等 |

## 性能特点

- ✅ **零拷贝**: 尽可能减少内存复制
- ✅ **流式处理**: 支持大文件处理
- ✅ **高效压缩**: DEFLATE 算法优化
- ✅ **编译优化**: release 模式下性能接近 C 版本

## 平台支持

- ✅ Linux (x86_64, ARM64)
- ✅ macOS (x86_64, ARM64)
- ✅ Windows (x86_64)
- ⚠️ 其他平台：Rust 支持的平台均可编译

## 开发状态

- [x] 核心功能实现（100%）
- [x] miniz 算法重写（100%）
- [x] 测试对等验证（100%）
- [x] 命令行工具（100%）
- [ ] 性能优化（持续改进）
- [ ] 文档完善（进行中）

## 贡献指南

本项目是**研究项目**，主要目标不是生产使用，而是：
1. 验证 AI 辅助代码重写的可行性
2. 建立跨语言重写的方法论
3. 探索测试驱动迁移的最佳实践

**欢迎贡献**：
- 报告 bug（附 C 版本对比）
- 提出性能优化建议
- 改进文档和示例
- 分享 AI 辅助开发的经验

## 许可证

与原项目保持一致，详见 LICENSE 文件。

## 致谢

- 原始 C 版本: [r-lib/zip](https://github.com/r-lib/zip)
- miniz 压缩库: [richgel999/miniz](https://github.com/richgel999/miniz)
- AI 工具: Claude (Anthropic)

## 联系方式

- 问题反馈: GitHub Issues
- 技术讨论: GitHub Discussions

---

**⭐ 如果这个项目对你的研究有帮助，请给个 Star！**
