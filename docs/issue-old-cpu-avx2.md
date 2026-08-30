# Issue 提案：支持无 AVX2 的旧 CPU（三代酷睿等）—— onnxruntime 改用动态加载官方 DLL

> 状态：**提案待批准**（实现已在本地 `dev` 分支完成，见文末「已实现改动」，需维护者确认技术路线后合并）
>
> 类型：兼容性 / 崩溃修复
> 建议标签：`needs-triage` → `ready-for-human`（或 `ready-for-agent` 视实现策略而定）

---

## 1. 问题现象

- **三代酷睿（Ivy Bridge，2012）/ 更早** 等**无 AVX2** 的 CPU 上，应用**启动即闪退**，Windows 事件日志报 **0xC000001D（Illegal Instruction，非法指令）**。
- 现代 CPU（Haswell 2013 之后）完全正常。
- 用户机器可能没有任何错误提示，表现为「双击没反应」。

**影响功能**：情绪分类（`emotion/classifier.rs`）、ASR 语音活动检测（`asr/vad.rs`，Silero VAD）、本地 TTS（`tts/local`，SBV2）——这些功能都走 ONNX Runtime。

## 2. 根因分析（已确认）

`ort` crate 默认开启 `download-binaries`，会在编译时**静态链接 pyke.io 预编译的 onnxruntime**（本工程锁定 `2.0.0-rc.13`，实际下载 **onnxruntime 1.28.0**）。

**关键事实**：pyke.io 预编译的 x86-64 二进制按 **x86-64-v3 基线**编译（要求 **AVX2 / FMA3**），官方原话：

> All x86-64 binaries are compiled with a baseline requirement of x86-64-v3 — that's Intel Haswell (Cores/Xeons after 2013).

三代酷睿只有 **AVX（无 AVX2 / FMA3）**，属于 v2 级别。由于 onnxruntime 被**静态链接进 exe**，程序一启动就会执行 AVX2 指令 → 进程直接崩溃（0xC000001D），连降级的机会都没有。

ort 源码（`environment.rs`）也自带警告：

> `WARNING: This CPU does not support AVX2... The app will likely crash with an illegal instruction error; use a custom build of ONNX Runtime to fix.`

## 3. 技术方案细节（方案 A）

> **方案**：`load-dynamic` + 微软官方 `onnxruntime.dll`。官方包为 **SSE3 基线 + MLAS 运行时 CPUID dispatch**（源码 `onnxruntime/core/common/cpuid_info.*` 与 `mlas/lib/platform.cpp` 可证），AVX2 代码路径受 CPUID 保护，旧 CPU 自动走低指令集路径——**兼容旧 CPU 且无需自编译**；崩溃的根源只是 ort 默认用了 pyke 的 v3 编译包。

### 3.1 编译期
- `Cargo.toml`：Windows target 的 `ort` 使用 `load-dynamic` → 不再静态链接 onnxruntime，exe 内不含 AVX2 指令，旧 CPU 可正常启动。
- **范围仅 Windows**：`load-dynamic` 只作用于 `cfg(target_os = "windows")`；**Linux/macOS 保持 `download-binaries` 静态链接，行为与改动前完全一致**，无需任何适配。

### 3.2 运行时加载
- 新增 `src-tauri/src/utils/onnx.rs`，在 `setup` 早期（任何 `ort::Session` 创建之前）调用：
  1. 探测 `onnxruntime.dll`：**exe 同目录**（开发/便携）→ **tauri 资源目录**（打包安装后）;
  2. 找到后 `ort::init_from(path)` **显式加载**（`load-dynamic` 下必须显式 `init_from`，设置 `ORT_DYLIB_PATH` 环境变量在 raw-dylib 机制下**无效**——已从 ort 源码确认）;
  3. 找不到 / 加载失败 → 记录日志并返回失败，情绪分类 / VAD / 本地 TTS 走既有降级路径（分类器返回 None、VAD 返回 Err、TTS 停用），**不影响应用主体**。

### 3.3 打包分发
- `tauri.conf.json` resources 增加 `binaries/onnxruntime.dll → onnxruntime.dll`（Windows 打包到 exe 同目录）。
- `scripts/download-onnxruntime.mjs`（新增）：下载微软官方 win-x64 包并解出 `onnxruntime.dll` 到 `src-tauri/binaries/`（不入 git，见 `.gitignore`）。

### 3.4 版本匹配（重要坑）
- `ort 2.0.0-rc.13` 编译时默认 **`api-27`**（对应 onnxruntime ≥ 1.27）。
- 因此官方 dll 必须 **≥ 1.27**。更旧的（如 1.17.x，SSE3 但 API 太旧）会被 `ort::init_from` 以 `BadVersion` 拒绝。
- 本实现默认下载 **官方 1.27.1**（脚本支持 `ORT_VERSION` 环境变量切换）。

## 4. 已实现改动（本地 `dev` 分支，提交 `aa8be97c`）

| 文件 | 改动 |
|---|---|
| `src-tauri/Cargo.toml` | `ort` 按平台隔离：Windows target 用 `load-dynamic`，Linux/macOS 保持 `download-binaries`（附注释说明） |
| `src-tauri/src/utils/onnx.rs` | **新增**：`init_onnx_runtime()`（探测 → `ort::init_from` → 降级），含 `#[ignore]` 运行时验证测试 |
| `src-tauri/src/lib.rs` | setup 中在 `init_data_dir` 之后调用 `utils::onnx::init_onnx_runtime()` |
| `src-tauri/src/utils/mod.rs` | 注册 `pub mod onnx` |
| `src-tauri/tauri.conf.json` | resources 打包 `onnxruntime.dll` |
| `scripts/download-onnxruntime.mjs` | **新增**：官方 dll 下载脚本（默认 1.27.1） |
| `.gitignore` | 忽略 `src-tauri/binaries/`（~15MB 二进制不入库） |

## 5. 已验证

- ✅ `cargo check` 通过（`load-dynamic` 编译不依赖 dll）
- ✅ 运行时测试：`ort::init_from` 成功加载官方 **1.27.1** 并创建 SessionBuilder（`cargo test load_official_onnxruntime -- --ignored`）
- ✅ `pnpm tauri build` 成功：`ling_chat.exe` + NSIS 安装包生成，安装包内确认包含 `onnxruntime.dll`
- ✅ dll 缺失时降级逻辑存在（emotion/vad/tts 均有容错）
- ✅ **旧 CPU 实机验证通过**：无 AVX2 的旧 CPU（三代酷睿 Ivy Bridge）上运行打包后的 exe，应用正常启动，情绪 / VAD / 本地 TTS 正常工作，无 0xC000001D 崩溃

## 6. 对开发流程的影响与自动化

- **开发者零操作**：新增 `scripts/ensure-onnxruntime.mjs`（幂等：dll 缺失才下载，并复制到 `target/debug`、`target/release`），挂到 `beforeDevCommand` / `beforeBuildCommand`：
  - `pnpm tauri dev` / `pnpm tauri build` 自动确保 dll 就位，无需手动跑下载脚本；
  - 新 clone 首次构建自动下载；
  - dll 已存在时不重复下载（幂等）。
- **范围仅 Windows**：`load-dynamic` 只作用于 Windows target；Linux/macOS 保持 `download-binaries` 静态链接，行为与改动前一致，无需适配。
- **注意**：`src-tauri/binaries/` 不入 git（~15MB 二进制）；升级 ort 版本时需同步 `scripts/download-onnxruntime.mjs` 的 `ORT_VERSION`（ort 2.0.0-rc.13 需 onnxruntime ≥1.27）。

## 7. 待验证 / 风险

1. **性能**：旧 CPU 上无 AVX2 路径，情绪/VAD/本地 TTS 推理会走 SSE 内核，速度略慢（可接受）。
2. **安装包体积**：增加约 15MB（官方 dll 未压缩前 15MB，NSIS 压缩后增量更小）。
3. **dll 缺失场景**：安装包/便携目录不含 dll 时相关 AI 功能自动降级（不影响主流程），需在发布流程保证 dll 随包分发。

## 8. 决策点（请维护者确认）

1. 是否同意采用**方案 A（load-dynamic + 官方 onnxruntime.dll）**作为兼容旧 CPU 的路线？
2. 是否接受**安装包 +~15MB** 换取旧 CPU 兼容？
3. 范围确认：`load-dynamic` **仅 Windows**，Linux/macOS 保持 `download-binaries` 静态链接（行为不变）——是否同意？
4. **合入方式**：直接合入 `dev` 正式开发主线，还是**另开独立分支**再合入（参考 Heiyahand 的 `pr-XXX-xxx` 模式，如 `pr-oldcpu-avx2`）？当前改动已在本地 `dev` 分支（提交 `aa8be97c` / `d4f00087` / `89048f8a`），如需独立分支可据此切出。
5. 若批准，上述提交即可合入。（旧 CPU 实机验证已通过，见上文「已验证」）
