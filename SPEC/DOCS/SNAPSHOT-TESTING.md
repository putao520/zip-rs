# 快照测试规范

## 版本信息
- **文档版本**: v1.0.0
- **最后更新**: 2025-12-24
- **状态**: Active
- **关联SPEC**: SPEC/06-TESTING-STRATEGY.md v2.0.0
- **工具**: insta crate

---

## 概述

本文档定义 zip-rs 的快照测试规范，用于100%精确复刻C版本的 `expect_snapshot` 行为。快照测试确保输出与预存的期望值完全一致。

---

## C版本快照测试分析

### testthat expect_snapshot

**C版本位置**: tests/testthat/

**核心特性**:

| 特性 | 说明 |
|------|------|
| **快照存储** | tests/_snaps/目录 |
| **快照格式** | 人类可读的文本格式 |
| **版本控制** | 快照文件纳入Git |
| **变体支持** | UTF-8/ASCII等变体 |

**快照文件命名**:
```
test-file-name.R:
  test_that("test name", {
    expect_snapshot(value)
  })

生成快照:
tests/_snaps/test-file-name/test-name.md
```

**快照内容格式**:
```
# value

<actual output content>
```

**UTF-8变体支持**:
```r
if (l10n_info()[["UTF-8"]]) {
  variant <- "utf8"
} else {
  out <- iconv(out, "UTF-8", "ASCII", sub = "byte")
  variant <- "ascii"
}
expect_snapshot(cat(out), variant = variant)
```

### 典型C版本快照测试

**示例1: 基本输出快照**
```r
data <- inflate(data_gz, 1L, 245L)
out <- rawToChar(data$output)
expect_snapshot(cat(out))
```

**示例2: 错误快照**
```r
expect_snapshot(error = TRUE, inflate(data_gz, 10L, 300L))
```

**示例3: 条件变体**
```r
if (l10n_info()[["UTF-8"]]) {
  variant <- "utf8"
} else {
  variant <- "ascii"
}
expect_snapshot(cat(out), variant = variant)
```

---

## Rust版本实现（insta）

### insta crate配置

**Cargo.toml依赖**:
```toml
[dev-dependencies]
insta = "1.40"
```

**.config/insta.yaml**:
```yaml
# 快照更新行为
behavior:
  # 未找到快照时的行为
  missing_snapshot: "warn"  # warn | error | create
  # 快照不匹配时的行为
  mismatched_snapshot: "warn"  # warn | error | update
  # 未挂载的快照
  unmounted_snapshot: "warn"

# 快照文件路径
snapshot_path: "tests/snapshots"

# 快照更新
# 设置环境变量 INSTA_UPDATE=always 来更新所有快照
```

### 快照断言宏

| 宏 | 用途 | C版本对应 |
|-----|------|----------|
| `assert_snapshot!(value)` | 基本快照 | `expect_snapshot(value)` |
| `assert_snapshot!(expression, value)` | 带表达式的快照 | `expect_snapshot(expr, value)` |
| `assert_rsnapshot!(name)` | 命名快照 | N/A |

### 快照文件命名

**insta命名规则**:

| 测试文件 | 测试函数 | 快照文件 |
|---------|---------|---------|
| tests/inflate.rs | test_inflate | tests/snapshots/inflate__test_inflate.snap |
| tests/zip_process.rs | test_zip_process | tests/snapshots/zip_process__test_zip_process.snap |

**命名格式**: `<test_file>__<test_fn>.snap`

### 快照内容格式

**insta快照格式**:
```snap
---
source: tests/inflate.rs
expression: output
---
<?xml version="1.0" encoding="UTF-8"?>
...
```

**格式说明**:

| 部分 | 说明 | 示例 |
|------|------|------|
| source | 源文件路径 | tests/inflate.rs |
| expression | 快照表达式 | output |
| content | 实际快照内容 | 输出内容 |

---

## 快照测试映射

### 基本映射

| C版本 | Rust版本 | 说明 |
|-------|---------|------|
| `expect_snapshot(cat(out))` | `assert_snapshot!(out)` | 输出快照 |
| `expect_snapshot(error=TRUE, expr)` | `assert_snapshot!(format!("{:?}", err))` | 错误快照 |
| `expect_snapshot(value, variant=v)` | `assert_snapshot!(value, &insta::Settings::new().variant(v))` | 变体快照 |

### 示例映射

**示例1: inflate输出快照**

C版本:
```r
data <- inflate(data_gz, 1L, 245L)
out <- rawToChar(data$output)
expect_snapshot(cat(out))
```

Rust版本:
```rust
let result = inflate(&data_gz, 1, Some(245))?;
let out = String::from_utf8(result.output)?;
insta::assert_snapshot!(out);
```

**示例2: 错误快照**

C版本:
```r
expect_snapshot(error = TRUE, inflate(data_gz, 10L, 300L))
```

Rust版本:
```rust
let err = inflate(&data_gz, 10, Some(300)).unwrap_err();
insta::assert_snapshot!(format!("{:?}", err));
```

**示例3: UTF-8变体快照**

C版本:
```r
if (l10n_info()[["UTF-8"]]) {
  variant <- "utf8"
} else {
  variant <- "ascii"
}
expect_snapshot(cat(out), variant = variant)
```

Rust版本:
```rust
#[cfg(target_os = "linux")]
let variant = "utf8";
#[cfg(not(target_os = "linux"))]
let variant = "ascii";

insta::assert_snapshot!(out, &insta::Settings::new()
    .variant(variant)
    .bind(|| "test_with_variant"));
```

---

## 快照更新流程

### 初次创建快照

**命令**:
```bash
# 方法1: 设置环境变量
INSTA_UPDATE=always cargo test

# 方法2: 使用cargo-insta
cargo install cargo-insta
cargo insta test --accept
```

**行为**:
- 运行测试
- 创建快照文件
- 显示新快照内容

### 更新现有快照

**命令**:
```bash
# 更新所有快照
INSTA_UPDATE=always cargo test

# 更新特定快照
cargo insta test --accept test_name

# 交互式审核
cargo insta test --review
```

**审核流程**:
```
1. 运行测试，检测快照差异
   ↓
2. 显示差异（diff格式）
   ↓
3. 用户选择：接受/拒绝
   ↓
4. 更新快照文件或保持原样
```

### 快照文件管理

**Git提交**:
```bash
# 提交快照文件
git add tests/snapshots/
git commit -m "test: update snapshots for XXX"
```

**版本控制**:
| 规则 | 说明 |
|------|------|
| 纳入Git | 快照文件应该版本控制 |
| 代码审查 | 快照变更需要审查 |
| 分支处理 | 快照可能在不同分支不同 |

---

## 特殊场景处理

### 临时路径标准化

**C版本**: transform_tempdir()
```r
transform_tempdir <- function(x) {
  x <- sub(tempdir(), "<tempdir>", x, fixed = TRUE)
  x <- sub(normalizePath(tempdir()), "<tempdir>", x, fixed = TRUE)
  x <- sub("[\\\\/]file[a-zA-Z0-9]+", "/<tempfile>", x)
  x <- sub("[A-Z]:.*Rtmp[a-zA-Z0-9]+[\\\\/]", "<tempdir>/", x)
  x
}
```

**Rust版本实现**:

```rust
fn normalize_temp_paths(output: String) -> String {
    let mut output = output;

    // 替换临时目录路径
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        output = output.replace(&tmpdir, "<tempdir>");
    }
    output = output.replace("/tmp", "<tempdir>");

    // Windows路径处理
    #[cfg(windows)]
    {
        output = output.replace("\\", "/");
        let re = regex::Regex::new(r"[A-Z]:.*Rtmp[a-zA-Z0-9]+[\\/]").unwrap();
        output = re.replace_all(&output, "<tempdir>/").to_string();
    }

    // 替换临时文件名
    let re = regex::Regex::new(r"[\\/]?file[a-zA-Z0-9]+").unwrap();
    output = re.replace_all(&output, "/<tempfile>").to_string();

    output
}

// 在快照测试中使用
let normalized = normalize_temp_paths(raw_output);
insta::assert_snapshot!(normalized);
```

### 平台差异处理

**路径分隔符**:

| 平台 | 分隔符 | 处理方式 |
|------|--------|---------|
| Unix | / | 统一转换为 / |
| Windows | \ | 转换为 / 后快照 |

**换行符**:

| 场景 | 处理 |
|------|------|
| 文本输出 | 统一转换为 \n |
| 二进制输出 | 保持原始字节 |

**示例**:
```rust
#[cfg(unix)]
let output = output.replace("\r\n", "\n");

insta::assert_snapshot!(output);
```

### 大型输出处理

**问题**: 输出过大导致快照文件过大

**解决方案**:

| 方案 | 适用场景 |
|------|---------|
| 截断 | 只验证关键部分 |
| 采样 | 验证部分内容 |
| 哈希 | 验证内容摘要 |
| 分组 | 拆分为多个快照 |

**示例 - 截断**:
```rust
let truncated = if output.len() > 1000 {
    format!("{}... (truncated, total {} bytes)", &output[..1000], output.len())
} else {
    output
};
insta::assert_snapshot!(truncated);
```

---

## 快照测试最佳实践

### 快照设计原则

| 原则 | 说明 |
|------|------|
| **稳定性** | 快照应该稳定，不随时间变化 |
| **可读性** | 快照应该人类可读，便于审查 |
| **必要性** | 只快照真正需要验证的内容 |
| **隔离性** | 每个测试独立快照，不相互依赖 |

### 快照内容选择

**适合快照**:
- 文本输出（XML, JSON, 文本）
- 错误消息
- 文件列表
- 结构化数据

**不适合快照**:
- 二进制数据（用assert_eq!替代）
- 时间戳（用占位符）
- 随机数据（固定种子）
- 大型数据（采样验证）

### 快照维护

| 任务 | 说明 |
|------|------|
| 定期审查 | 快照是否仍然有效 |
| 及时更新 | 功能变更后更新快照 |
| 版本控制 | 追踪快照变更历史 |
| 文档化 | 复杂快照添加注释 |

---

## 实现检查清单

### 配置

- [ ] 添加 insta 依赖到 Cargo.toml
- [ ] 创建 .config/insta.yaml
- [ ] 配置快照路径为 tests/snapshots/

### 工具

- [ ] 安装 cargo-insta
- [ ] 配置 IDE 快照更新快捷键
- [ ] 配置 CI 快照检查

### 测试代码

- [ ] 替换所有 expect_snapshot 为 assert_snapshot!
- [ ] 实现路径标准化函数
- [ ] 实现平台差异处理
- [ ] 添加变体支持（如需要）

### 快照文件

- [ ] 创建初始快照
- [ ] 审查快照内容
- [ ] 提交快照到Git
- [ ] 文档化快照格式

---

## CI集成

### CI快照检查

**GitHub Actions配置**:
```yaml
- name: Run tests
  run: cargo test
  env:
    INSTA_UPDATE: no  # CI中不允许更新快照
```

**行为**:
| 设置 | 行为 |
|------|------|
| INSTA_UPDATE=no | 快照不匹配时失败 |
| INSTA_UPDATE=always | 自动更新快照 |
| INSTA_UPDATE=new | 只创建新快照 |

### 快照审查流程

**Pull Request**:
1. 运行测试，检查快照变更
2. 如果快照变更，审查差异
3. 确认变更合理后，合并
4. 快照文件纳入提交

**自动化检查**:
```yaml
- name: Check snapshots
  run: |
    if git diff --name-only | grep -q "tests/snapshots"; then
      echo "Snapshot changes detected"
      git diff tests/snapshots/
    fi
```

---

## 故障排查

### 常见问题

| 问题 | 原因 | 解决方案 |
|------|------|---------|
| 快照不匹配 | 输出变化 | 审查差异，更新快照 |
| 快照文件未创建 | 路径错误 | 检查 insta.yaml 配置 |
| 快照过多 | 测试设计问题 | 合并或拆分测试 |
| 平台差异 | 路径/换行符 | 标准化输出 |

### 调试技巧

**显示快照差异**:
```bash
cargo insta test --review
```

**查看快照内容**:
```bash
cat tests/snapshots/test__name.snap
```

**重新创建特定快照**:
```bash
cargo insta test --accept test_name
```

---

## 版本历史

| 版本 | 日期 | 变更说明 |
|------|------|---------|
| v1.0.0 | 2025-12-24 | 初始版本，定义快照测试规范 |
