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

## 3. 技术方案细节

> **方案**：`load-dynamic` + 微软官方 `onnxruntime.dll`。官方包为 **SSE3 基线 + MLAS 运行时 CPUID dispatch**（源码 `onnxruntime/core/common/cpuid_info.*` 与 `mlas/lib/platform.cpp` 可证），AVX2 代码路径受 CPUID 保护，旧 CPU 自动走低指令集路径——**兼容旧 CPU 且无需自编译**；崩溃的根源只是 ort 默认用了 pyke 的 v3 编译包。

### 3.1 编译期
- `Cargo.toml`：Windows target 的 `ort` 使用 `load-dynamic` → 不再静态链接 onnxruntime，exe 内不含 AVX2 指令，旧 CPU 可正常启动。
- **范围仅 Windows**：`load-dynamic` 只作用于 `cfg(target_os = "windows")`；**Linux/macOS 保持 `download-binaries` 静态链接，行为与改动前完全一致**，无需任何适配。

### 3.2 运行时加载
- 新增 `src-tauri/src/utils/onnx.rs`，在 `setup` 早期（任何 `ort::Session` 创建之前）调用：
  1. 探测 `onnxruntime.dll`：**exe 同目录**（开发/便携）→ **tauri 资源目录**（打包安装后）;
  2. 找到后 `ort::init_from(path)` **显式加载**（`load-dynamic` 下必须显式 `init_from`，设置 `ORT_DYLIB_PATH` 环境变量在 raw-dylib 机制下**无效**——已从 ort 源码确认）;
  3. 找不到 / 加载失败 → 记录日志并返回失败，情绪分类 / VAD / 本地 TTS 走既有降级路径（分类器返回 None、VAD 返回 Err、TTS 停用），**不影响应用主体**。

**降级机制（关键）**：`onnx.rs` 维护全局 `onnx_available()` 标志（`init_from` 成功才置 true；非 Windows 恒 true），**所有 onnx 使用点（情绪分类 / VAD / 本地 TTS）在创建 `ort::Session` 之前检查**——因为 `load-dynamic` 下 dll 缺失/损坏时首次创建 Session 会触发 ort 内部 `setup_api` 的 `expect` panic（整个应用崩溃）。另：`ort::init_from` **只能调用一次**（失败后 ort 内部 `G_ORT_LIB` inserter 已被消费，重调是 UB），故只探测**第一个存在**的候选，绝不循环重试。

### 3.3 打包分发
- `src-tauri/tauri.windows.conf.json`（新增，**Windows 专属配置**）：resources 增加 `binaries/onnxruntime.dll → onnxruntime.dll`（Windows 打包到 exe 同目录；macOS/Linux 不加载该配置、不校验此文件）。
- `scripts/download-onnxruntime.mjs`（新增）：下载微软官方 win-x64 包并解出 `onnxruntime.dll` 到 `src-tauri/binaries/`（不入 git，见 `.gitignore`）。

### 3.4 版本匹配（重要坑）
- `ort 2.0.0-rc.13` 编译时默认 **`api-27`**（对应 onnxruntime ≥ 1.27）。
- 因此官方 dll 必须 **≥ 1.27**。更旧的（如 1.17.x，SSE3 但 API 太旧）会被 `ort::init_from` 以 `BadVersion` 拒绝。
- 本实现默认下载 **官方 1.27.1**（脚本支持 `ORT_VERSION` 环境变量切换）。

## 4. 已实现改动（本地 `dev` 分支，提交 `aa8be97c` → `242c072b` 共 12 笔）

| 文件 | 改动 |
|---|---|
| `src-tauri/Cargo.toml` | `ort` 按平台隔离：Windows 用 `load-dynamic`；Linux x86_64 / macOS aarch64 / Linux aarch64 用 `download-binaries`（附注释说明） |
| `src-tauri/src/utils/onnx.rs` | **新增**：`init_onnx_runtime()`（探测 → `ort::init_from` → 降级），含 `#[ignore]` 运行时验证测试；整体限定 `cfg(target_os = "windows")`（`init_from` 仅在 `load-dynamic` 下存在） |
| `src-tauri/src/lib.rs` | setup 中在 `init_data_dir` 之后调用 `utils::onnx::init_onnx_runtime()`（限 Windows） |
| `src-tauri/src/utils/mod.rs` | 注册 `pub mod onnx` |
| `src-tauri/tauri.windows.conf.json` | **新增**（Windows 专属）：resources 打包 `onnxruntime.dll`；macOS/Linux 不校验 |
| `scripts/download-onnxruntime.mjs` | **新增**：官方 dll 下载脚本（默认 1.27.1；解压后递归查找 dll，兼容 zip 顶层目录） |
| `scripts/ensure-onnxruntime.mjs` | **新增**：幂等确保 dll（缺才下载 + 复制到 target/{debug,release}），挂 `beforeDevCommand`/`beforeBuildCommand`，仅 Windows 生效 |
| `src-tauri/src/ai_service/emotion/classifier.rs` | `load()` 前检查 `onnx_available()`，不可用返回 Err（降级为禁用） |
| `src-tauri/src/ai_service/asr/vad.rs` | `load()` 前检查，不可用返回 Err（`init_asr` 降级不传播） |
| `src-tauri/src/ai_service/tts/local/engine.rs` | `init()` / `load_voice()` 前检查，不可用返回 Err（本地 TTS 停用） |
| `src-tauri/src/init/mod.rs` | `init_asr`：VAD 加载失败降级为 ASR 不可用，启动不受影响 |
| `.github/workflows/dev-build.yml` | 初始化步骤显式调用 ensure（裸 `cargo build` 不走 tauri 钩子） |
| `.github/workflows/dev-build-full.yml` | 初始化 ensure 兜底 + 上传产物附带 `onnxruntime.dll`（Windows） |
| `.gitignore` | 忽略 `src-tauri/binaries/`（~15MB 二进制不入库） |

## 5. 已验证

- ✅ `cargo check` 通过（`load-dynamic` 编译不依赖 dll）
- ✅ 运行时测试：`ort::init_from` 成功加载官方 **1.27.1** 并创建 SessionBuilder（`cargo test load_official_onnxruntime -- --ignored`）
- ✅ `pnpm tauri build` 成功：`ling_chat.exe` + NSIS 安装包生成，安装包内确认包含 `onnxruntime.dll`
- ✅ dll 缺失时降级逻辑存在（emotion/vad/tts 均有容错）
- ✅ **旧 CPU 实机验证通过**：无 AVX2 的旧 CPU（三代酷睿 Ivy Bridge）上运行打包后的 exe，应用正常启动，情绪 / VAD / 本地 TTS 正常工作，无 0xC000001D 崩溃
- ✅ **CI 全绿**：`dev-build`（4 平台 `cargo build`）与 `dev-build-full`（4 平台 `tauri build --no-bundle`）全部通过
- ✅ **CI 产物确凿含 dll**：Windows artifact 解包验证包含 `onnxruntime.dll`（15.4MB）
- ✅ **`release.yml` 构建链路通过**：打包发行版 4 平台构建全部成功（至 updater 签名前；签名失败为 fork secrets 配置问题，见第 6 节）
- ✅ **dll 缺失降级实测**：移走 dll 启动应用，正常初始化（AIService / 剧本 / 主动系统），进程存活不崩溃
- ✅ **dll 损坏（空/异常文件）降级实测**：正式版 exe + 空文件 dll，进程存活不崩溃（`0xC0000409` UB 已修）
- ✅ **正式版打包验证**：`ling_chat.exe`（含前端资源）+ NSIS 安装包正常生成，应用正常启动

## 6. 对开发流程的影响与自动化

- **开发者零操作**：新增 `scripts/ensure-onnxruntime.mjs`（幂等：dll 缺失才下载，并复制到 `target/debug`、`target/release`），挂到 `beforeDevCommand` / `beforeBuildCommand`：
  - `pnpm tauri dev` / `pnpm tauri build` 自动确保 dll 就位，无需手动跑下载脚本；
  - 新 clone 首次构建自动下载；
  - dll 已存在时不重复下载（幂等）。
- **范围仅 Windows**：`load-dynamic` 只作用于 Windows target；Linux/macOS 保持 `download-binaries` 静态链接，行为与改动前一致，无需适配。
- **注意**：`src-tauri/binaries/` 不入 git（~15MB 二进制）；升级 ort 版本时需同步 `scripts/download-onnxruntime.mjs` 的 `ORT_VERSION`（ort 2.0.0-rc.13 需 onnxruntime ≥1.27）。
- **release 的 updater 签名**：`tauri build` 在 `createUpdaterArtifacts` 阶段需正确的 `TAURI_SIGNING_PRIVATE_KEY` + 密码 secrets；fork/CI 配置错误会在最后一步失败（安装包已生成，仅缺 updater 增量包），上游用正确 secrets 无此问题。

## 7. 待验证 / 风险

1. **性能**：旧 CPU 上无 AVX2 路径，情绪/VAD/本地 TTS 推理会走 SSE 内核，速度略慢（可接受）。
2. **安装包体积**：增加约 15MB（官方 dll 未压缩前 15MB，NSIS 压缩后增量更小）。
3. ~~**dll 缺失/损坏场景**~~（已解决）：相关 AI 功能自动降级且应用不崩溃（本地实测）；发布流程已保证 dll 随包分发。

## 8. 决策点（请维护者确认）

1. 是否同意采用**方案 A（load-dynamic + 官方 onnxruntime.dll）**作为兼容旧 CPU 的路线？
2. 是否接受**安装包 +~15MB** 换取旧 CPU 兼容？
3. 范围确认：`load-dynamic` **仅 Windows**，Linux/macOS 保持 `download-binaries` 静态链接（行为不变）——是否同意？
4. **合入方式**：直接合入 `dev` 正式开发主线，还是**另开独立分支**再合入（参考 Heiyahand 的 `pr-XXX-xxx` 模式，如 `pr-oldcpu-avx2`）？当前改动已在本地 `dev` 分支（提交 `aa8be97c` → `242c072b` 共 12 笔），如需独立分支可据此切出。
5. 若批准，上述提交即可合入。（旧 CPU 实机验证已通过，见上文「已验证」）
