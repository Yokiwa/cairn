# Cairn 阶段 1「能玩」实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 做出阶段 1 的可用版本：在 Windows / macOS / Linux 上登录（微软或离线）、建实例（原版或 Fabric / Quilt / Forge / NeoForge）、自动配 Java、下载并启动游戏，有中英文界面、命令行版、CI 和自动发布。

**Architecture:** Rust workspace：`crates/core` 放全部业务逻辑（不依赖 Tauri），`crates/cli` 和 `src-tauri` 都只是它的外壳。界面在 `ui/`（React + TS），通过 `ui/src/lib/backend.ts` 里的 `Backend` 接口调用后端；在 Tauri 里用真实实现，在浏览器 / Playwright 里用内存模拟实现。Rust 和 TS 的共享类型用 ts-rs 从 Rust 生成。

**Tech Stack:** Rust stable（edition 2021）、tokio、reqwest（rustls）、serde、zip / flate2 / tar、keyring、ts-rs、clap、wiremock、Tauri 2；Vite、React 19、TypeScript、Tailwind CSS 4、Motion、react-i18next、TanStack Query 5、Zustand 5、React Router 7、Vitest、Playwright；pnpm。

**Spec:** `docs/superpowers/specs/2026-11-04-mc-launcher-design.md`（本计划覆盖第 2 节的阶段 1、第 3、4、8、9、10、11、12、13 节中属于阶段 1 的部分）。界面以原型为准：https://claude.ai/artifact/HGjG8aTc9U3V1urxufBtBi

## Global Constraints

- 名字：产品名 `Cairn`，命令行 `cairn`，crate 名 `cairn-core`、`cairn-cli`、`cairn-app`；GitHub 仓库 `Yokiwa/cairn`
- 许可证：GPL-3.0（`LICENSE` 放 GPL-3.0 全文，`Cargo.toml` / `package.json` 写 `GPL-3.0-only`）
- 数据目录：Windows `%APPDATA%\Cairn`，macOS `~/Library/Application Support/Cairn`，Linux `~/.local/share/cairn`；环境变量 `CAIRN_HOME` 覆盖（测试和命令行用）
- User-Agent：`Cairn/<版本> (github.com/Yokiwa/cairn)`
- 下载：默认 32 并发，可调 1–64；先写 `*.part` 再改名；失败重试 3 次，等待 1 秒、2 秒、4 秒；SHA1 不对删掉重下
- Java：需要的大版本取 `javaVersion.majorVersion`，没写按 8；缺的从 Adoptium 下载 Temurin JRE
- 默认内存：电脑内存 ≤ 8 GB → 3072 MB；≤ 16 GB → 6144 MB；更大 → 8192 MB
- 游戏异常退出：显示日志最后 50 行
- 重要文件（`instance.json`、`settings.json`、`accounts.json`）一律原子写入（临时文件 + 改名）
- 令牌只存系统钥匙串（keyring service 名 `cairn`），不写进任何文件，也不写进日志
- 所有错误是 `AppError { key, params, detail }`，`key` 必须在 `ui/src/locales/en.json` 和 `zh-CN.json` 里都有：**哪个任务新增了错误 key，就在同一个任务里把中英文翻译加进这两个文件**（错误文案放在 `errors.<key>` 下）
- Forge 支持 1.12.2 及以上；NeoForge 全部版本；低于 1.8 的游戏版本不做测试保证
- 界面：浅色 / 深色默认跟随系统；强调色默认 `#34D399`；系统"减少动态效果"时关闭动画；文字对比度 ≥ 4.5:1；全部功能可键盘操作
- 界面语言默认跟随系统，系统语言以 `zh` 开头用简体中文，否则英文
- 阶段 1 的导航只有 首页、实例、设置；"发现""存档""换版本"留给阶段 2、3
- 没有遥测
- 每次提交信息末尾加两行署名：
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` 和 `Claude-Session: https://claude.ai/code/session_01YSmZu72ReqFzyzuSWac6U3`

## Review Focus

1. **路径里有空格或中文**（如 `C:\Users\张 三\AppData\Roaming\Cairn`）：classpath 和启动参数必须原样传给进程，不能被拆开或乱码 → Task 10 加测试 `build_command_handles_spaces_and_non_ascii`
2. **安装到一半被关掉**：再次安装或启动时只补缺的文件，不会把半截文件当成好的 → Task 2 `resumes_from_part_file`、Task 4 `second_install_only_fetches_missing`
3. **实例名重复、或名字全是中文 / 符号**（slug 为空）：id 仍然唯一且合法 → Task 6 `create_with_cjk_name_gets_fallback_id`、`create_duplicate_name_gets_suffix`
4. **`instance.json` / `settings.json` 被手改坏**：列表跳过坏实例并给出警告，不崩溃；设置回退默认值并把坏文件改名为 `settings.json.bak` → Task 1 `corrupt_settings_falls_back_and_backs_up`、Task 6 `list_skips_corrupt_instance`
5. **同一个实例连点两次启动**：第二次返回 `error.launch.already_running`，不会开两个游戏抢同一个存档 → Task 11 `second_launch_of_running_instance_is_rejected`

---

## 文件结构

```
Cargo.toml                         workspace
LICENSE  README.md  README.zh-CN.md  CONTRIBUTING.md
crates/core/src/
  lib.rs            Core（把各模块拼起来的上下文）
  error.rs          AppError
  paths.rs          Paths
  fsutil.rs         write_atomic、copy_dir 等
  settings.rs       Settings、recommended_memory_mb
  http.rs           HttpClient
  download.rs       Downloader、DownloadItem
  tasks.rs          Progress、TaskManager
  versions/{mod,manifest,version_json,rules,maven,install}.rs
  java/{mod,detect,adoptium}.rs
  loaders/{mod,fabric,forge}.rs
  instances.rs      Instance、InstanceStore
  auth/{mod,offline,msa,store}.rs
  launch/{mod,args,natives,process}.rs
crates/core/tests/  fixtures/ 和集成测试；live.rs（需要联网，#[ignore]）
crates/cli/src/main.rs
src-tauri/          tauri.conf.json、capabilities/、src/{main,commands,events,state}.rs
ui/                 Vite 项目；src/{main.tsx,App.tsx,bindings/,lib/,components/,pages/,locales/,styles/}
ui/e2e/             Playwright
.github/workflows/  ci.yml、release.yml
docs/setup/         用户本人要办的事
```

---

### Task 1: Workspace、错误类型、路径、设置

**Files:**
- Create: `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/{lib.rs,error.rs,paths.rs,fsutil.rs,settings.rs}`, `LICENSE`, `.gitignore`, `rust-toolchain.toml`（channel = "stable"）, `ui/src/locales/{en.json,zh-CN.json}`（先只放 `errors` 下本任务的 key：`error.io`、`error.network`、`error.parse`、`error.cancelled`）
- Test: 各文件内 `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub struct AppError { pub key: String, pub params: BTreeMap<String, String>, pub detail: String }`（Serialize、TS）；`AppError::new(key) -> Self`、`.with_param(k, v) -> Self`、`.with_detail(d) -> Self`；`impl From<std::io::Error>`（key `error.io`）、`From<reqwest::Error>`（`error.network`）、`From<serde_json::Error>`（`error.parse`）；`pub type Result<T> = std::result::Result<T, AppError>`
  - `pub struct Paths`：`Paths::new(root: PathBuf)`、`Paths::default_root() -> Result<PathBuf>`（先看 `CAIRN_HOME`）、`root()`、`settings_file()`、`accounts_file()`、`instances_dir()`、`instance_dir(id)`、`trash_dir()`、`versions_dir()`、`libraries_dir()`、`assets_dir()`、`java_dir()`、`backups_dir()`、`cache_dir()`、`logs_dir()`；子目录名照抄规格第 3 节（`shared/versions` 等）
  - `pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()>`（同目录临时文件 → fsync → rename）
  - `pub struct Settings { schema: u32, language: Option<String>, theme: Theme, accent: String, download_threads: u8, default_memory_mb: Option<u32>, show_snapshots: bool }`；`pub enum Theme { System, Light, Dark }`；`Settings::load(&Paths) -> Result<Settings>`、`save(&self, &Paths) -> Result<()>`；`pub fn recommended_memory_mb(total_mb: u64) -> u32`

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn recommended_memory_tiers() {
    assert_eq!(recommended_memory_mb(8 * 1024), 3072);
    assert_eq!(recommended_memory_mb(16 * 1024), 6144);
    assert_eq!(recommended_memory_mb(32 * 1024), 8192);
}
#[test] fn settings_default_values() {
    let s = Settings::default();
    assert_eq!((s.download_threads, s.accent.as_str(), s.theme), (32, "#34D399", Theme::System));
}
#[test] fn settings_roundtrip_and_threads_clamped() { /* 保存 download_threads=200 → 读回 64；0 → 1 */ }
#[test] fn corrupt_settings_falls_back_and_backs_up() {
    // 写入 "{not json" → load 得到 Settings::default()，并且 settings.json.bak 存在、内容是 "{not json"
}
#[test] fn write_atomic_replaces_whole_file() { /* 先写长内容再写短内容，读回等于短内容，目录里没有残留临时文件 */ }
#[test] fn cairn_home_overrides_default_root() { /* 设 CAIRN_HOME → default_root() 等于它 */ }
```

- [ ] **Step 2: 运行，确认失败** — `cargo test -p cairn-core`，预期：编译错误（类型未定义）
- [ ] **Step 3: 实现上面 Produces 里的全部类型和函数**。默认数据目录用 `directories::BaseDirs` 的 data dir，Windows / macOS 拼 `Cairn`，Linux 拼 `cairn`。`AppError` 同时 derive `ts_rs::TS`，导出到 `ui/src/bindings/`（之后所有给界面用的类型都这样做）
- [ ] **Step 4: 运行，确认通过** — `cargo test -p cairn-core`，预期全部 PASS；`cargo clippy --workspace -- -D warnings` 无警告
- [ ] **Step 5: 提交** — `git commit -m "feat(core): workspace, errors, paths, settings"`

---

### Task 2: HTTP 客户端、下载引擎、任务和进度

**Files:**
- Create: `crates/core/src/{http.rs,download.rs,tasks.rs}`
- Test: `crates/core/tests/download.rs`（wiremock）

**Interfaces:**
- Consumes: `AppError`、`Result`、`Paths::cache_dir`
- Produces:
  - `pub struct HttpClient`（Clone）：`HttpClient::new() -> Result<Self>`（UA 见全局约束）、`get_json<T: DeserializeOwned>(&self, url) -> Result<T>`、`get_json_cached<T>(&self, url, cache_dir: &Path, ttl: Duration) -> Result<T>`（离线时缓存过期也返回旧缓存）、`post_form<T>(url, &[(&str,&str)])`、`post_json<B, T>(url, &B)`、`get_bytes(url) -> Result<Bytes>`、`inner() -> &reqwest::Client`。429 按 `Retry-After` 等待（最多 60 秒）后重试；5xx 和连接错误重试 3 次
  - `pub struct DownloadItem { pub url: String, pub dest: PathBuf, pub sha1: Option<String>, pub size: Option<u64> }`
  - `pub struct Progress { pub stage: String, pub done_files: u64, pub total_files: u64, pub done_bytes: u64, pub total_bytes: u64, pub bytes_per_sec: u64 }`（TS）；`pub type ProgressFn = Arc<dyn Fn(&Progress) + Send + Sync>`
  - `pub struct Downloader`：`Downloader::new(http, concurrency: usize)`、`with_retry_base(Duration)`（测试里设成 1ms）、`async fn download_all(&self, stage: &str, items: Vec<DownloadItem>, progress: ProgressFn, cancel: CancellationToken) -> Result<()>`
  - `pub fn sha1_file(path: &Path) -> Result<String>`
  - `pub struct Endpoints { mojang_meta, libraries, resources, adoptium, fabric_meta, quilt_meta, forge_maven, forge_files, neoforge_maven, msa: MsaEndpoints }`（`MsaEndpoints` 在 Task 9 定义前先用占位结构，Task 9 补全），`Default` 是真实地址：`https://piston-meta.mojang.com`、`https://libraries.minecraft.net`、`https://resources.download.minecraft.net`、`https://api.adoptium.net`、`https://meta.fabricmc.net`、`https://meta.quiltmc.org`、`https://maven.minecraftforge.net`、`https://files.minecraftforge.net`、`https://maven.neoforged.net`
  - `pub struct Core { pub paths: Paths, pub http: HttpClient, pub downloader: Downloader, pub endpoints: Endpoints, pub tasks: TaskManager }`（在 `lib.rs`）；`Core::with_endpoints(paths: Paths, endpoints: Endpoints) -> Result<Core>`。后面的任务往 `Core` 里加字段：Task 6 加 `instances`，Task 9 加 `auth`，Task 11 加 `running` 和 `Core::new(paths)`
  - `pub struct TaskManager`：`spawn<F>(&self, title: String, f: impl FnOnce(ProgressFn, CancellationToken) -> F) -> TaskId`（F: Future<Output = Result<serde_json::Value>>）、`cancel(id)`、`list() -> Vec<TaskInfo>`、`subscribe() -> broadcast::Receiver<TaskEvent>`；`pub enum TaskEvent { Progress { id, progress }, Done { id, result }, Failed { id, error: AppError } }`（TS，serde tag = "type"）。`TaskId` 是 `String`（uuid v4）

- [ ] **Step 1: 写失败的测试**

```rust
#[tokio::test] async fn skips_file_with_matching_sha1() { /* dest 已存在且 sha1 对 → mock 收到 0 次请求 */ }
#[tokio::test] async fn resumes_from_part_file() {
    // dest.part 已有前 5 字节；mock 校验请求头 Range: bytes=5- 并返回 206 剩余部分 → 最终文件完整、sha1 正确、.part 不存在
}
#[tokio::test] async fn checksum_mismatch_retries_then_fails() {
    // mock 永远返回错内容 → 请求 4 次（1 + 3 次重试），返回 key == "error.download.checksum"，dest 不存在
}
#[tokio::test] async fn server_error_then_success() { /* 前两次 500，第三次 200 → 成功 */ }
#[tokio::test] async fn progress_reaches_totals() { /* 3 个文件 → 最后一次回调 done_files == total_files == 3 */ }
#[tokio::test] async fn cancel_stops_download() { /* 取消后返回 key == "error.cancelled" */ }
#[tokio::test] async fn task_manager_emits_done_and_failed() { /* 成功任务收到 Done，失败任务收到带 AppError 的 Failed */ }
#[tokio::test] async fn get_json_cached_serves_stale_when_offline() { /* 先缓存，再让 mock 返回连接错误，ttl 已过 → 仍拿到旧值 */ }
```

- [ ] **Step 2: 运行，确认失败** — `cargo test -p cairn-core --test download`
- [ ] **Step 3: 实现。** 并发用 `futures::stream::iter(...).buffer_unordered(concurrency)`；速度用最近 1 秒内的字节数算；缓存文件名用 URL 的 SHA1
- [ ] **Step 4: 运行，确认通过** — `cargo test -p cairn-core`
- [ ] **Step 5: 提交** — `feat(core): http client, parallel downloader, task manager`

---

### Task 3: 版本清单、version JSON、规则、Maven 路径

**Files:**
- Create: `crates/core/src/versions/{mod.rs,manifest.rs,version_json.rs,rules.rs,maven.rs}`
- Test: `crates/core/tests/versions_parse.rs`；fixtures `crates/core/tests/fixtures/versions/{manifest.json,1.20.1.json,1.8.9.json,fabric-1.20.1.json}`（按 Mojang / Fabric 的真实结构手写精简版：每个保留 3–5 个库，至少一个带 `rules`、一个带 `natives`）

**Interfaces:**
- Produces:
  - `pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"`
  - `pub struct VersionManifest { latest: Latest { release, snapshot }, versions: Vec<ManifestEntry> }`；`pub struct ManifestEntry { id, kind: VersionKind, url, sha1, release_time: DateTime<Utc> }`（TS）；`pub enum VersionKind { Release, Snapshot, OldBeta, OldAlpha }`（serde 名 `release` / `snapshot` / `old_beta` / `old_alpha`）
  - `pub async fn fetch_manifest(http: &HttpClient, cache_dir: &Path) -> Result<VersionManifest>`（缓存 10 分钟）
  - `pub struct VersionJson { id, inherits_from: Option<String>, main_class: String, arguments: Option<Arguments>, minecraft_arguments: Option<String>, libraries: Vec<Library>, downloads: Option<ClientDownloads>, asset_index: Option<AssetIndexRef>, assets: Option<String>, java_version: Option<JavaVersionReq>, logging: Option<Logging>, kind: Option<String> }` 及子类型；`pub enum Arg { Plain(String), Conditional { rules: Vec<Rule>, value: OneOrMany } }`（untagged）
  - `pub fn merge(parent: VersionJson, child: VersionJson) -> VersionJson`：`id`、`main_class` 取 child；`arguments` 拼接（parent 在前）；`libraries` 是 child 的加上 parent 里 `group:artifact` 没被 child 覆盖的（child 在前）；其余字段 child 有就用 child
  - `pub struct Environment { os: OsName, arch: String, os_version: String, features: HashMap<String, bool> }`；`Environment::current()`；`pub fn rules_allow(rules: &[Rule], env: &Environment) -> bool`（没有规则 → 允许；否则初始不允许，逐条匹配，最后一条匹配的生效）
  - `pub fn maven_path(coord: &str) -> Result<String>`：`group:artifact:version[:classifier][@ext]` → `group/路径/artifact/version/artifact-version[-classifier].ext`，默认 ext 为 `jar`

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn parses_manifest_kinds() { /* fixture 里 release / snapshot / old_beta 都能识别 */ }
#[test] fn parses_modern_and_legacy_args() {
    // 1.20.1：arguments.game 里有 Conditional（feature is_demo_user）；1.8.9：minecraft_arguments 是 Some
}
#[test] fn rules_os_disallow_and_allow() {
    // [{allow},{disallow os=osx}] 在 osx 上 false、linux 上 true；[{allow os=windows}] 只在 windows 上 true；空规则 true
}
#[test] fn rules_feature_flag() { /* has_custom_resolution 为 true 时 Conditional 生效 */ }
#[test] fn maven_path_variants() {
    assert_eq!(maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(), "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
    assert_eq!(maven_path("org.lwjgl:lwjgl:3.3.1:natives-linux").unwrap(), "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar");
    assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
}
#[test] fn merge_fabric_over_vanilla() {
    // main_class 是 Fabric 的 KnotClient；库里 ASM 只有 child 的版本一份；vanilla 的 client 下载信息保留
}
```

- [ ] **Step 2: 运行，确认失败** — `cargo test -p cairn-core --test versions_parse`
- [ ] **Step 3: 实现。** `Environment::current()` 的 arch：`x86_64` → `"x86_64"`，`aarch64` → `"arm64"`，`x86` → `"x86"`；规则里的 `os.arch` 按这个比较，`os.version` 是正则
- [ ] **Step 4: 运行，确认通过**
- [ ] **Step 5: 提交** — `feat(core): version manifest, version json, rules, maven paths`

---

### Task 4: 安装游戏本体（库、natives、资源文件）

**Files:**
- Create: `crates/core/src/versions/install.rs`
- Test: `crates/core/tests/versions_install.rs`（wiremock 托管 fixture 里的所有 URL；测试前把 fixture 里的 URL 改写到 mock 地址）

**Interfaces:**
- Consumes: Task 2 的 `Downloader`、`DownloadItem`、`ProgressFn`；Task 3 的类型和函数
- Produces:
  - `pub struct AssetIndex { objects: HashMap<String, AssetObject { hash, size }>, map_to_resources: bool, r#virtual: bool }`
  - `pub fn plan_game_files(v: &VersionJson, env: &Environment, paths: &Paths) -> Result<Vec<DownloadItem>>`：client jar → `versions/<id>/<id>.jar`；每个 `rules_allow` 的库 → `libraries/<path>`（有 `downloads.artifact` 用它的 URL 和 SHA1；只有 `url` 的用 `<url>/<maven_path>`，没有 SHA1；都没有用 `https://libraries.minecraft.net/`）；带 `natives` 的库按 `natives[os]`（替换 `${arch}` 为 `32`/`64`）取 classifier；`logging.client.file` → `assets/log_configs/<id>`
  - `pub fn plan_assets(index: &AssetIndex, paths: &Paths) -> Vec<DownloadItem>`：`assets/objects/<hash 前两位>/<hash>`，URL `https://resources.download.minecraft.net/<前两位>/<hash>`
  - `pub fn load_installed(paths: &Paths, id: &str) -> Result<Option<VersionJson>>`（读本地 `versions/<id>/<id>.json`，有 `inherits_from` 就递归读并 `merge`）
  - `pub async fn install_version(core: &Core, id: &str, progress: ProgressFn, cancel: CancellationToken) -> Result<VersionJson>`：本地 JSON 存在就不访问版本清单；否则从清单取 URL 下载 JSON 存盘；再下载资源索引、游戏文件、资源文件；返回合并后的 `VersionJson`
  - 上面的固定地址都从 `core.endpoints`（Task 2）读取，测试里换成 mock 地址

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn plan_includes_client_libs_and_natives_for_current_os() { /* 1.8.9 fixture 在 linux 环境下包含 natives-linux，不含 natives-windows */ }
#[test] fn plan_skips_disallowed_library() { /* 规则排除 osx 的库在 osx 环境下不出现 */ }
#[test] fn plan_library_with_only_maven_url() { /* fabric fixture 的库 URL == "<url>/<maven_path>" 且 sha1 为 None */ }
#[tokio::test] async fn install_downloads_everything_then_loads_offline() {
    // install_version 成功；之后关掉 mock，load_installed 仍返回同一个合并结果
}
#[tokio::test] async fn second_install_only_fetches_missing() {
    // 第一次装好后删掉一个库文件；第二次 install 时 mock 只收到这一个文件的请求（再加清单 / JSON 不请求）
}
```

- [ ] **Step 2: 运行，确认失败**
- [ ] **Step 3: 实现**
- [ ] **Step 4: 运行，确认通过**
- [ ] **Step 5: 提交** — `feat(core): install vanilla game files and assets`

---

### Task 5: Java 检测、选择、自动下载

**Files:**
- Create: `crates/core/src/java/{mod.rs,detect.rs,adoptium.rs}`
- Test: 文件内单元测试 + `crates/core/tests/java.rs`（wiremock 托管 Adoptium 接口和一个测试代码生成的 `.zip` / `.tar.gz`）

**Interfaces:**
- Consumes: `HttpClient`、`Downloader`、`Paths::java_dir`、`VersionJson`
- Produces:
  - `pub struct JavaInstall { pub path: PathBuf /* java 可执行文件 */, pub major: u32, pub version: String, pub arch: String, pub managed: bool }`（TS）
  - `pub fn parse_version(s: &str) -> Option<u32>`：`"1.8.0_392"` → 8，`"17.0.9"` → 17，`"21"` → 21
  - `pub fn find_installations(paths: &Paths) -> Vec<JavaInstall>`：`JAVA_HOME`、`PATH`、各系统常见目录（规格第 4 节）、`java_dir` 下；优先读 `release` 文件的 `JAVA_VERSION`，读不到运行 `java -version`；按 `path` 去重
  - `pub fn required_major(v: &VersionJson) -> u32`（默认 8）
  - `pub fn select(installs: &[JavaInstall], major: u32) -> Option<&JavaInstall>`：大版本必须相等；同架构优先；`managed` 优先
  - `pub async fn ensure_java(core: &Core, major: u32, progress, cancel) -> Result<JavaInstall>`：先 `select`；没有就请求 `{adoptium}/v3/assets/latest/{major}/hotspot?architecture={x64|aarch64}&image_type=jre&os={windows|mac|linux}&vendor=eclipse`，下载、校验 `checksum`（SHA256）、解压到 `java/temurin-{major}/`；macOS 的可执行文件在 `Contents/Home/bin/java`；Windows 用 `javaw.exe`。macOS arm64 上 Java 8 没有 aarch64 包时退回 x64

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn parse_version_forms() { assert_eq!(parse_version("1.8.0_392"), Some(8)); assert_eq!(parse_version("17.0.9"), Some(17)); assert_eq!(parse_version("21"), Some(21)); assert_eq!(parse_version("garbage"), None); }
#[test] fn required_major_defaults_to_8() { /* 1.8.9 fixture 无 javaVersion → 8；1.20.1 fixture → 17 */ }
#[test] fn select_requires_exact_major_and_prefers_managed() {}
#[test] fn reads_release_file() { /* 临时目录造一个 release 文件 JAVA_VERSION="17.0.9" 和假的 bin/java → find_installations 认出 17 */ }
#[tokio::test] async fn ensure_java_downloads_and_extracts() { /* mock 返回资产信息和 tar.gz → 返回的 path 存在且 managed == true */ }
#[tokio::test] async fn ensure_java_rejects_bad_checksum() { /* key == "error.java.checksum" */ }
```

- [ ] **Step 2: 运行，确认失败**
- [ ] **Step 3: 实现**（SHA256 用 `sha2` crate；解压用 `zip` / `flate2` + `tar`，并防止压缩包里的路径跳出目标目录）
- [ ] **Step 4: 运行，确认通过**
- [ ] **Step 5: 提交** — `feat(core): java detection and adoptium download`

---

### Task 6: 实例管理

**Files:**
- Create: `crates/core/src/instances.rs`
- Test: 文件内单元测试

**Interfaces:**
- Consumes: `Paths`、`write_atomic`；往 `Core` 加字段 `instances: InstanceStore`
- Produces（全部 TS 导出）：
  - `pub struct Instance { schema: u32, id: String, name: String, minecraft: String, loader: Option<LoaderSpec>, java: JavaSetting, memory_mb: u32, jvm_args: Vec<String>, created_at: DateTime<Utc>, last_played_at: Option<DateTime<Utc>>, mods: BTreeMap<String, ModSource> }`——字段和规格第 3 节 `instance.json` 示例一致
  - `pub struct LoaderSpec { kind: LoaderKind, version: String }`；`pub enum LoaderKind { Fabric, Quilt, Forge, NeoForge }`（serde 小写：`fabric`/`quilt`/`forge`/`neoforge`）
  - `pub enum JavaSetting { Auto, Path { path: PathBuf } }`（serde tag = "mode"，小写）
  - `pub struct ModSource { source: String, project_id: String, version_id: String }`（阶段 2 才写入，阶段 1 只要能读写）
  - `pub struct NewInstance { name: String, minecraft: String, loader: Option<LoaderSpec>, memory_mb: Option<u32> }`
  - `pub struct ModFile { file_name: String, enabled: bool, size: u64 }`
  - `pub struct InstanceStore { paths: Paths }`：`list() -> Result<(Vec<Instance>, Vec<AppError>)>`（坏文件进第二个列表，key `error.instance.corrupt`）、`get(id)`、`create(req, default_memory_mb: u32) -> Result<Instance>`（建 `instances/<id>/minecraft/{mods,saves,resourcepacks,shaderpacks,config}`）、`save(&Instance)`、`delete(id)`（整个目录移到 `trash/<id>-<时间戳>`）、`game_dir(id) -> PathBuf`、`list_mods(id) -> Result<Vec<ModFile>>`（`.jar` 和 `.jar.disabled`，按文件名排序）、`set_mod_enabled(id, file_name, enabled) -> Result<()>`
  - `pub fn slugify(name: &str) -> String`：小写；非 ASCII 字母数字的连续字符变成一个 `-`；去掉首尾 `-`；结果为空时用 `instance`

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn slugify_examples() { assert_eq!(slugify("Survival 1.20.1"), "survival-1-20-1"); assert_eq!(slugify("我的世界"), "instance"); assert_eq!(slugify("  A--B  "), "a-b"); }
#[test] fn create_duplicate_name_gets_suffix() { /* 两次建 "Test" → id "test" 和 "test-2" */ }
#[test] fn create_with_cjk_name_gets_fallback_id() { /* name 保持 "我的世界"，id == "instance" */ }
#[test] fn create_makes_game_dirs_and_json_roundtrips() {}
#[test] fn list_skips_corrupt_instance() { /* 一个好的 + 一个 instance.json 内容为 "oops" → 返回 1 个实例、1 个错误 */ }
#[test] fn toggle_mod_renames_file() { /* a.jar → a.jar.disabled → a.jar */ }
#[test] fn delete_moves_to_trash() {}
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): instance store`

---

### Task 7: Fabric 和 Quilt

**Files:**
- Create: `crates/core/src/loaders/{mod.rs,fabric.rs}`
- Test: `crates/core/tests/loaders_fabric.rs`；fixtures `fixtures/loaders/{fabric-versions.json,fabric-profile.json,quilt-versions.json,quilt-profile.json}`

**Interfaces:**
- Consumes: `HttpClient`、`Downloader`、`plan_game_files`、`install_version`、`LoaderKind`
- Produces:
  - `pub struct LoaderVersion { version: String, stable: bool }`（TS）
  - `pub async fn list_loader_versions(core: &Core, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>>`（新的在前）
  - `pub async fn install_loader(core: &Core, mc: &str, loader: &LoaderSpec, progress, cancel) -> Result<String>`：返回要启动的 version id。Fabric / Quilt 的 id 用 profile JSON 自带的（如 `fabric-loader-0.16.9-1.20.1`），JSON 存到 `versions/<id>/<id>.json`；先 `install_version(mc)`，再下载 profile 里的库
  - 接口：Fabric `{fabric_meta}/v2/versions/loader/{mc}`、`{fabric_meta}/v2/versions/loader/{mc}/{v}/profile/json`；Quilt `{quilt_meta}/v3/versions/loader/{mc}`、`{quilt_meta}/v3/versions/loader/{mc}/{v}/profile/json`。Fabric 用返回里的 `loader.stable`；Quilt 版本号不含 `-beta` / `-pre` 的算稳定

- [ ] **Step 1: 写失败的测试**

```rust
#[tokio::test] async fn lists_fabric_versions_with_stable_flag() {}
#[tokio::test] async fn lists_quilt_versions_marks_betas_unstable() {}
#[tokio::test] async fn install_fabric_writes_profile_and_libraries() {
    // 返回 id == "fabric-loader-0.16.9-1.20.1"；load_installed(该 id) 的 main_class 是 "net.fabricmc.loader.impl.launch.knot.KnotClient"
}
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): fabric and quilt loaders`

---

### Task 8: Forge 和 NeoForge

**Files:**
- Create: `crates/core/src/loaders/forge.rs`
- Test: `crates/core/tests/loaders_forge.rs`。测试代码生成安装器 jar：zip 里放 `install_profile.json`、`version.json`、`data/client.lzma`，再用测试代码编译一个最小的 processor jar（一个 `Main` 类把参数写进文件；用 `javac`，CI 机器都装了 JDK，没有 `javac` 时跳过这个测试并打印原因）

**Interfaces:**
- Consumes: Task 5 `ensure_java`、Task 7 的 `LoaderVersion` / `install_loader` 分派
- Produces:
  - 版本列表：Forge 读 `{forge_maven}/net/minecraftforge/forge/maven-metadata.xml`（`<version>` 形如 `1.20.1-47.3.0`，按 `mc-` 前缀筛），用 `{forge_files}/net/minecraftforge/forge/promotions_slim.json` 的 `<mc>-recommended` 标稳定；NeoForge 读 `{neoforge_maven}/api/maven/versions/releases/net/neoforged/neoforge`（`21.1.77` 对应 mc `1.21.1`：`1.` + 前两段），mc `1.20.1` 改读 `net/neoforged/forge`；含 `-beta` 的不稳定
  - 安装器 URL：`{forge_maven}/net/minecraftforge/forge/{v}/forge-{v}-installer.jar`；`{neoforge_maven}/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar`
  - `pub async fn install_forge_like(core, mc, loader, progress, cancel) -> Result<String>`：
    1. `install_version(mc)` 确保原版文件齐全
    2. 下载安装器到 `cache/installers/`
    3. 新格式（有 `json` 和 `processors` 字段）：读 `version.json` 存为 `versions/<id>/<id>.json`；下载 `install_profile.libraries` 和 version 里的库；解压安装器里的 `maven/` 目录到 `libraries/`；按下面的规则运行 processors
    4. 旧格式（有 `versionInfo` 和 `install` 字段，用于 1.12.2 的老构建）：`versionInfo` 存为 version JSON；把 `install.filePath` 解压到 `install.path` 对应的库路径
  - processors 规则：`data` 的值取 `client` 一侧：`[coord]` → 库文件绝对路径；`'text'` → 字面量；以 `/` 开头 → 从安装器里解压到临时目录后的路径。内置变量 `SIDE=client`、`MINECRAFT_JAR`、`MINECRAFT_VERSION`、`ROOT`、`INSTALLER`、`LIBRARY_DIR`。跳过 `sides` 不含 `client` 的。每个 processor 的 classpath = 它的 jar + `classpath` 里的库；主类读 jar 的 `META-INF/MANIFEST.MF` 的 `Main-Class`；用该 MC 版本要求的 Java 运行；有 `outputs` 且所有输出文件都存在并且 SHA1 匹配时跳过；运行后校验 `outputs`，不匹配返回 `error.loader.processor_output`

- [ ] **Step 1: 写失败的测试**

```rust
#[tokio::test] async fn lists_forge_versions_for_mc_and_marks_recommended() {}
#[tokio::test] async fn maps_neoforge_versions_to_mc() { /* 21.1.77 → 1.21.1；20.4.1-beta 不稳定 */ }
#[tokio::test] async fn runs_client_processors_with_substituted_args() {
    // 生成的 processor 把收到的参数写进文件 → 文件内容里 {MINECRAFT_JAR} 已替换成真实路径、[coord] 已替换成库路径；sides=["server"] 的 processor 没运行
}
#[tokio::test] async fn skips_processor_when_outputs_match() {}
#[tokio::test] async fn legacy_installer_format_extracts_universal_jar() {}
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): forge and neoforge installers`

---

### Task 9: 账号（离线、微软设备码、钥匙串）

**Files:**
- Create: `crates/core/src/auth/{mod.rs,offline.rs,msa.rs,store.rs}`
- Test: 文件内单元测试 + `crates/core/tests/auth_msa.rs`（wiremock 模拟四个服务）

**Interfaces:**
- Produces（DTO 都 TS 导出）：
  - `pub struct Account { id: String /* 带横线的 UUID */, username: String, kind: AccountKind, active: bool }`；`pub enum AccountKind { Microsoft, Offline }`；`accounts.json` 是 `Vec<Account>`
  - `pub fn offline_uuid(name: &str) -> Uuid`：`OfflinePlayer:<name>` 的 MD5，按 Java `UUID.nameUUIDFromBytes` 设置版本 3 和变体位
  - `pub trait SecretStore: Send + Sync { fn get(&self, key: &str) -> Result<Option<String>>; fn set(&self, key: &str, value: &str) -> Result<()>; fn delete(&self, key: &str) -> Result<()>; }`；`KeyringStore`（service `cairn`）、`MemoryStore`（测试用）
  - `pub struct MsaEndpoints { login, xbl, xsts, mc }`，`Default` 为 `https://login.microsoftonline.com`、`https://user.auth.xboxlive.com`、`https://xsts.auth.xboxlive.com`、`https://api.minecraftservices.com`
  - `pub fn msa_client_id() -> Option<&'static str>`：`option_env!("CAIRN_MSA_CLIENT_ID")`，空字符串当作没有
  - `pub struct DeviceCode { user_code: String, verification_uri: String, expires_in: u64, interval: u64 }`（TS）
  - `pub struct AuthService`：`new(paths, http, endpoints, store: Arc<dyn SecretStore>)`；`accounts() -> Result<Vec<Account>>`；`add_offline(name) -> Result<Account>`（名字 3–16 位，只能有字母、数字、下划线，否则 `error.auth.invalid_name`）；`start_device_login() -> Result<(DeviceCode, PendingLogin)>`；`PendingLogin::finish(self) -> Result<Account>`（轮询直到成功或超时）；`remove(id)`；`set_active(id)`；`session(id) -> Result<Session>`
  - `pub struct Session { username, uuid, access_token, user_type: String /* "msa" 或 "legacy" */, xuid: Option<String> }`；`Debug` 实现里隐藏 `access_token`
  - 流程（规格第 4 节）：设备码 `POST {login}/consumers/oauth2/v2.0/devicecode`（scope `XboxLive.signin offline_access`）；轮询 `POST {login}/consumers/oauth2/v2.0/token`，处理 `authorization_pending`（继续）、`slow_down`（间隔 +5 秒）、`expired_token` / `authorization_declined`（失败）；XBL `POST {xbl}/user/authenticate`（`RpsTicket: "d=<token>"`）；XSTS `POST {xsts}/xsts/authorize`（`RelyingParty: "rp://api.minecraftservices.com/"`），`XErr 2148916233` → `error.auth.no_xbox`，`2148916238` → `error.auth.child_account`；`POST {mc}/authentication/login_with_xbox`（`identityToken: "XBL3.0 x=<uhs>;<xsts>"`）；`GET {mc}/minecraft/profile`，404 → `error.auth.no_game`
  - 刷新令牌存 `msa-refresh:<uuid>`；访问令牌和过期时间只放内存；`session()` 时过期就用刷新令牌续期，失败返回 `error.auth.relogin`
  - 没有 client id 时 `start_device_login` 返回 `error.auth.not_configured`

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn offline_uuid_is_v3_and_stable() { let a = offline_uuid("Alex"); assert_eq!(a.get_version_num(), 3); assert_eq!(a, offline_uuid("Alex")); assert_ne!(a, offline_uuid("Steve2")); }
#[test] fn offline_name_validation() { /* "ab" 和 "has space" 被拒；"Player_01" 通过 */ }
#[tokio::test] async fn device_login_full_chain() {
    // pending 一次、slow_down 一次后成功 → 账号 kind == Microsoft、username 来自 profile；MemoryStore 里有 msa-refresh:<uuid>
}
#[tokio::test] async fn xsts_no_xbox_account_error() {}
#[tokio::test] async fn profile_404_means_no_game() {}
#[tokio::test] async fn session_refreshes_expired_token() {}
#[test] fn session_debug_hides_token() { assert!(!format!("{:?}", session).contains("secret-token")); }
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): offline and microsoft accounts`

---

### Task 10: 启动参数和 natives

**Files:**
- Create: `crates/core/src/launch/{args.rs,natives.rs}`
- Test: 文件内单元测试（用 Task 3 的 fixtures）

**Interfaces:**
- Consumes: `VersionJson`、`Environment`、`rules_allow`、`maven_path`、`Instance`、`Session`、`JavaInstall`、`Paths`
- Produces:
  - `pub struct LaunchContext<'a> { version: &'a VersionJson, instance: &'a Instance, session: &'a Session, java: &'a JavaInstall, paths: &'a Paths, env: &'a Environment, natives_dir: PathBuf, window: Option<(u32, u32)> }`
  - `pub struct LaunchCommand { program: PathBuf, args: Vec<String>, cwd: PathBuf }`；`LaunchCommand::redacted(&self) -> Vec<String>`（把访问令牌换成 `***`）
  - `pub fn build_command(ctx: &LaunchContext) -> Result<LaunchCommand>`：
    - JVM 参数：`-Xmx{memory_mb}M`；有 `logging.client` 时加它的 `argument`（`${path}` 换成日志配置文件路径）；version 有 `arguments.jvm` 就按规则展开；没有（旧版本）就用 `-Djava.library.path=${natives_directory}`、`-cp`、`${classpath}`；最后加实例的 `jvm_args`；再加 `main_class`
    - 游戏参数：新格式按规则展开 `arguments.game`；旧格式按空格拆 `minecraft_arguments`
    - 占位符：`auth_player_name`、`version_name`、`game_directory`、`assets_root`、`assets_index_name`、`auth_uuid`（不带横线）、`auth_access_token`、`auth_session`、`clientid`、`auth_xuid`、`user_type`、`version_type`、`natives_directory`、`launcher_name`（`cairn`）、`launcher_version`、`classpath`、`classpath_separator`、`library_directory`、`game_assets`、`user_properties`（`{}`）、`resolution_width`、`resolution_height`；未知占位符原样保留
    - features：`is_demo_user` 恒为 false；`has_custom_resolution` 等于 `window.is_some()`；其他 `quickPlay*` 为 false
    - classpath：所有允许的、非 natives 的库，按 `group:artifact` 去重（保留第一个），最后加 client jar；分隔符 Windows `;`，其他 `:`
    - 旧资源布局：`assets` 为 `legacy` 时 `game_assets` = `assets/virtual/legacy`；`pre-1.6` 时 = `<游戏目录>/resources`，启动前由 `prepare_legacy_assets` 复制
  - `pub fn extract_natives(v: &VersionJson, env: &Environment, paths: &Paths, dest: &Path) -> Result<()>`：只处理带 `natives` 映射的库；跳过 `extract.exclude` 里的前缀（默认 `META-INF/`）；`dest` 每次启动前清空
  - `pub fn prepare_legacy_assets(index: &AssetIndex, paths: &Paths, target: &Path) -> Result<()>`

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn modern_command_has_expected_args() {
    // 1.20.1：args 包含 "-Xmx6144M"、"net.minecraft.client.main.Main"、"--username" 后接 "PlayerOne"、"--gameDir" 后接实例游戏目录；不包含 "--demo"
}
#[test] fn legacy_command_uses_minecraft_arguments() { /* 1.8.9：含 "-Djava.library.path=" 前缀的参数和 "--userProperties" "{}" */ }
#[test] fn custom_resolution_feature() { /* window = Some((1280,720)) → 含 "--width" "1280" */ }
#[test] fn build_command_handles_spaces_and_non_ascii() {
    // paths 根目录为 "<tmp>/张 三/Cairn" → "--gameDir" 后面的那一个参数完整等于该路径（没有被拆成两个）；classpath 参数里含 "张 三"
}
#[test] fn redacted_hides_token() {}
#[test] fn classpath_dedupes_and_ends_with_client_jar() {}
#[test] fn extract_natives_skips_meta_inf() {}
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): launch arguments and natives`

---

### Task 11: 启动流程、进程和日志

**Files:**
- Create: `crates/core/src/launch/{mod.rs,process.rs}`；Modify: `crates/core/src/lib.rs`
- Test: `crates/core/tests/launch_flow.rs`（用一个假的 "java"：测试代码写的脚本 / 小程序，打印几行日志后按参数决定退出码）

**Interfaces:**
- Consumes: Task 4–10 的全部公开函数
- Produces:
  - `Core` 最终字段：`paths, http, downloader, endpoints, tasks, instances, auth, running`；`Core::new(paths: Paths) -> Result<Core>`（真实地址 + `KeyringStore`）；`Core::with_endpoints(paths, endpoints)` 改为额外接收 `secret_store: Arc<dyn SecretStore>`（测试传 `MemoryStore`）
  - `pub type LogFn = Arc<dyn Fn(&str) + Send + Sync>`
  - `pub struct LogTail`：保存最后 50 行
  - `pub struct RunningRegistry`：`try_register(instance_id) -> Result<RunningGuard>`（已在运行 → `error.launch.already_running`）、`is_running(id) -> bool`、`list() -> Vec<String>`；`RunningGuard` drop 时注销
  - `pub struct GameExit { pub code: Option<i32>, pub tail: Vec<String> }`（TS）
  - `pub async fn prepare_instance(core: &Core, instance: &Instance, progress, cancel) -> Result<(VersionJson, JavaInstall)>`：装原版 → 装加载器（如果有）→ 读合并后的 version → 选或下 Java（实例指定了 `JavaSetting::Path` 就用它，读不到版本返回 `error.java.invalid_path`）
  - `pub async fn launch_instance(core: &Core, instance_id: &str, account_id: &str, log: LogFn, progress, cancel) -> Result<GameExit>`：注册 → 准备 → 取 session → natives → 构建命令 → 用 `tokio::process::Command` 启动（不经过 shell）→ 逐行转发 stdout / stderr 给 `log` 和 `LogTail` → 更新 `last_played_at`（启动时）→ 等待退出；把 `redacted()` 后的命令写进启动器日志
  - 启动器自己的日志：`tracing` 写到 `logs/cairn.log`，按天滚动，保留 7 天

- [ ] **Step 1: 写失败的测试**

```rust
#[tokio::test] async fn launch_streams_logs_and_returns_exit() { /* 假 java 打印 "hello" 并以 0 退出 → log 收到 "hello"；GameExit.code == Some(0)；last_played_at 已更新 */ }
#[tokio::test] async fn crash_returns_last_50_lines() { /* 打印 120 行后以 1 退出 → tail.len() == 50，最后一行是第 120 行 */ }
#[tokio::test] async fn second_launch_of_running_instance_is_rejected() { /* 第一次还在运行时第二次 → key == "error.launch.already_running" */ }
#[tokio::test] async fn explicit_java_path_must_be_valid() {}
#[test] fn launcher_log_never_contains_token() { /* 启动后读 logs/cairn.log，不包含 session 的 access_token */ }
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(core): launch flow, process supervision, logs`

---

### Task 12: 命令行版 `cairn`

**Files:**
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`
- Test: `crates/cli/tests/cli.rs`（`assert_cmd`，设 `CAIRN_HOME` 到临时目录）

**Interfaces:**
- Consumes: `Core` 和各模块公开函数
- Produces: 子命令（clap derive）：
  - `cairn versions list [--snapshots]`
  - `cairn loaders list <fabric|quilt|forge|neoforge> --mc <版本>`
  - `cairn instance create <名字> --mc <版本> [--loader <种类>[@<版本>]] [--memory <MB>]`（不写加载器版本时用最新稳定版），创建后立即安装，显示进度条（indicatif）
  - `cairn instance list`、`cairn instance launch <id> [--offline <名字>] [--smoke-test <正则> --timeout <秒>]`
  - `cairn login`（打印设备码和网址，等待完成）、`cairn accounts list`
  - `--smoke-test`：日志里出现匹配正则的行就结束游戏进程并以 0 退出；超时以 1 退出（Task 13 用）
  - 错误输出：`error: <英文说明>` + 详情；退出码 1。英文说明从 `ui/src/locales/en.json` 编译期 `include_str!` 读取

- [ ] **Step 1: 写失败的测试**

```rust
#[test] fn help_lists_subcommands() { /* 输出含 "versions"、"instance"、"login" */ }
#[test] fn instance_list_empty() { /* 新 CAIRN_HOME → 退出码 0，输出 "No instances" */ }
#[test] fn offline_name_error_is_readable() { /* launch --offline "a b" → 退出码 1，stderr 含 en.json 里 error.auth.invalid_name 的英文 */ }
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(cli): cairn command line`

---

### Task 13: 真实联网测试（需要网络白名单）

**Files:**
- Create: `crates/core/tests/live.rs`（全部 `#[ignore]`）、`scripts/smoke.sh`

**前置条件:** 用户批准网络白名单（规格第 10 节第 2 项）。没批之前这个任务标为"阻塞"，先做 Task 14 及以后。

- [ ] **Step 1: 写测试** — `live_manifest_has_latest_release`；`live_install_vanilla_1_20_1`；`live_install_fabric_1_20_1`；`live_install_neoforge_1_21_1`；`live_install_forge_1_20_1`（每个都断言 `load_installed` 成功、client jar 的 SHA1 正确）
- [ ] **Step 2: 运行** — `cargo test -p cairn-core --test live -- --ignored --test-threads=1`，预期全部 PASS
- [ ] **Step 3: 写 `scripts/smoke.sh`**：`xvfb-run -a -s "-screen 0 1280x720x24"` + `LIBGL_ALWAYS_SOFTWARE=1` 运行 `cairn instance create smoke --mc 1.20.1 --loader fabric` 和 `cairn instance launch smoke --offline Smoke --smoke-test "Backend library: LWJGL version" --timeout 300`
- [ ] **Step 4: 运行** — `bash scripts/smoke.sh`，预期退出码 0。失败时按 superpowers:systematic-debugging 排查，把结论写进提交信息
- [ ] **Step 5: 提交** — `test: live install and launch smoke tests`

---

### Task 14: Tauri 外壳

**Files:**
- Create: `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/}`、`src-tauri/src/{main.rs,state.rs,commands.rs,events.rs}`
- Test: `src-tauri/src/commands.rs` 内单元测试（直接调用 command 函数，不起窗口）

**Interfaces:**
- Consumes: `Core`
- Produces（command 名就是界面调用名；参数和返回值都是 ts-rs 导出的类型）：
  - `system_info() -> SystemInfo { os, total_memory_mb, recommended_memory_mb, app_version }`
  - `versions_list(snapshots: bool) -> Vec<ManifestEntry>`、`loader_versions(kind: LoaderKind, mc: String) -> Vec<LoaderVersion>`
  - `instances_list() -> InstancesList { instances: Vec<Instance>, errors: Vec<AppError> }`、`instance_get(id)`、`instance_create(req: NewInstance) -> TaskId`（创建后在后台安装）、`instance_update(instance: Instance)`、`instance_delete(id)`、`instance_mods(id) -> Vec<ModFile>`、`instance_mod_toggle(id, file_name, enabled)`、`instance_launch(id) -> TaskId`（用当前账号；没有账号返回 `error.auth.no_account`）、`instance_open_folder(id)`
  - `settings_get() -> Settings`、`settings_set(settings: Settings)`
  - `accounts_list() -> Vec<Account>`、`account_login_start() -> DeviceCode`（后台等待，完成后发事件）、`account_add_offline(name) -> Account`、`account_remove(id)`、`account_set_active(id)`
  - `tasks_list() -> Vec<TaskInfo>`、`task_cancel(id)`
  - 事件：`task://event`（`TaskEvent`）、`instance://log`（`{ instance_id, line }`）、`instance://exit`（`{ instance_id, exit: GameExit }`）、`auth://done`（`Account`）、`auth://failed`（`AppError`）
  - 插件：`tauri-plugin-opener`、`tauri-plugin-updater`、`tauri-plugin-process`、`tauri-plugin-single-instance`
  - `tauri.conf.json`：`productName` `Cairn`，`identifier` `io.github.yokiwa.cairn`，窗口 1200×780、最小 900×600，`frontendDist` `../ui/dist`，`devUrl` `http://localhost:5173`；updater endpoint `https://github.com/Yokiwa/cairn/releases/latest/download/latest.json`，`pubkey` 在发布时通过 `--config` 注入（Task 19）
  - ts-rs 导出：在 `crates/core` 加测试 `export_bindings`，运行后 `ui/src/bindings/` 下生成全部类型；CI 检查生成结果和仓库里的一致

- [ ] **Step 1: 写失败的测试** — `instance_create_returns_task_and_instance_appears`；`launch_without_account_errors`（key `error.auth.no_account`）；`settings_set_then_get_roundtrip`
- [ ] **Step 2–4: 失败 → 实现 → 通过**（Linux 需要 `libwebkit2gtk-4.1-dev` 等系统包，安装命令写进 `CONTRIBUTING.md`）
- [ ] **Step 5: 验证能编译** — `cargo build -p cairn-app`，预期成功
- [ ] **Step 6: 提交** — `feat(app): tauri shell, commands and events`

---

### Task 15: 界面骨架（主题、多语言、布局、后端接口）

**Files:**
- Create: `ui/{package.json,vite.config.ts,tsconfig.json,eslint.config.js,index.html}`、`ui/src/{main.tsx,App.tsx}`、`ui/src/styles/theme.css`、`ui/src/locales/{en.json,zh-CN.json}`、`ui/src/lib/{backend.ts,tauriBackend.ts,mockBackend.ts,i18n.ts,store.ts,format.ts}`、`ui/src/components/{Sidebar.tsx,DownloadCapsule.tsx,Modal.tsx,Button.tsx,Pill.tsx,Switch.tsx,Toast.tsx,ErrorMessage.tsx}`
- Test: `ui/src/lib/*.test.ts`、`ui/src/components/*.test.tsx`（Vitest + Testing Library）

**Interfaces:**
- Consumes: `ui/src/bindings/*`（Task 14 生成）
- Produces:
  - `export interface Backend`：方法和 Task 14 的 command 一一对应（驼峰：`instancesList()`、`instanceCreate(req)` …），外加 `onTaskEvent(cb) => Unsubscribe`、`onInstanceLog(cb)`、`onInstanceExit(cb)`、`onAuthDone(cb)`、`onAuthFailed(cb)`
  - `export function getBackend(): Backend`：`window.__TAURI_INTERNALS__` 存在用 `tauriBackend`，否则用 `mockBackend`（内存数据：4 个示例实例、离线账号、假的进度推进）
  - `mockBackend` 额外导出 `resetMock()`（测试用）
  - `theme.css`：颜色全部用 CSS 变量（`--bg`、`--panel`、`--border`、`--text`、`--muted`、`--accent`、`--accent-text`、`--ok`、`--warn`、`--danger`、`--info`），`:root[data-theme="dark"]` 用原型的深色值，`:root[data-theme="light"]` 用对应浅色值（两套都要满足 4.5:1）；`@media (prefers-reduced-motion: reduce)` 关闭动画
  - `useTheme()`：按设置和系统偏好在 `<html>` 上设 `data-theme` 和 `--accent`
  - `i18n.ts`：react-i18next；`detectLanguage(navigatorLanguage: string, setting?: string): "en" | "zh-CN"`；`errorText(err: AppError, t)` 用 `err.key` 和 `err.params` 翻译
  - `Sidebar`：首页、实例、设置三个入口和账号卡；`DownloadCapsule`：订阅 `onTaskEvent`，显示正在进行的任务、百分比、速度，点开看列表；全部完成时显示"全部完成"
  - 翻译文件里必须包含 Rust 里出现的全部错误 key（Task 19 的 CI 检查会扫描 Rust 源码中的 `AppError::new("…")` 并比对）

- [ ] **Step 1: 写失败的测试**

```ts
test("detectLanguage", () => {
  expect(detectLanguage("zh-CN")).toBe("zh-CN");
  expect(detectLanguage("zh-TW")).toBe("zh-CN");
  expect(detectLanguage("en-US")).toBe("en");
  expect(detectLanguage("de-DE")).toBe("en");
  expect(detectLanguage("en-US", "zh-CN")).toBe("zh-CN");
});
test("en and zh-CN have identical key sets", () => { /* 递归比较 key */ });
test("errorText interpolates params", () => { /* key error.download.checksum + {file:"a.jar"} → 含 "a.jar" */ });
test("DownloadCapsule shows percent and all-done", async () => { /* 推一个 Progress 50% → 显示 "50%"；推 Done → 显示 "全部完成"/"All done" */ });
test("Sidebar marks active route with aria-current", () => {});
```

- [ ] **Step 2: 运行，确认失败** — `pnpm -C ui test`
- [ ] **Step 3: 实现**
- [ ] **Step 4: 运行，确认通过** — `pnpm -C ui test && pnpm -C ui typecheck && pnpm -C ui lint`
- [ ] **Step 5: 提交** — `feat(ui): theme, i18n, layout, backend abstraction`

---

### Task 16: 界面：首页、实例列表、新建实例向导

**Files:**
- Create: `ui/src/pages/{HomePage.tsx,InstancesPage.tsx}`、`ui/src/components/{InstanceCard.tsx,NewInstanceWizard.tsx,LaunchButton.tsx,GameLogPanel.tsx}`
- Test: 同名 `*.test.tsx`

**Interfaces:**
- Consumes: `Backend`、Task 15 的组件
- Produces:
  - 首页：上次游玩的实例（`last_played_at` 最新的；没有就是最新创建的）、版本和加载器标签、大"开始游戏"按钮、"切换实例"、最近实例（最多 4 个）。没有实例时显示空状态和"新建实例"按钮。阶段 1 不放"反过来查"卡片
  - 实例页：卡片墙（游戏版本、加载器、模组数、上次游玩），"新建实例"按钮
  - 向导三步：选版本（默认只列正式版，开关显示快照，数据来自 `versionsList`）→ 选加载器（不装 / Fabric / Quilt / Forge / NeoForge；选中后调用 `loaderVersions` 取最新稳定版，所选 MC 版本没有该加载器时卡片禁用并说明）→ 命名（默认 `<加载器或 Vanilla> <版本>`，显示将要用的 Java 大版本：≥ 1.20.5 是 21，≥ 1.18 是 17，≥ 1.17 是 16，否则 8）。创建后关闭向导，下载胶囊出现进度
  - `LaunchButton`：调用 `instanceLaunch`；没有账号时弹出提示并引导去设置添加；运行中显示"游戏运行中"并禁用
  - `GameLogPanel`：游戏异常退出时自动弹出，显示 `GameExit.tail`（最后 50 行）和"打开日志文件夹"按钮

- [ ] **Step 1: 写失败的测试**

```tsx
test("wizard creates instance with defaults", async () => { /* 选 1.21.1 → Fabric → 下一步 → 名字默认 "Fabric 1.21.1"，Java 显示 21 → 创建 → 实例页出现该卡片 */ });
test("wizard shows Java 17 for 1.20.1 and 8 for 1.12.2", async () => {});
test("loader card disabled when no versions for mc", async () => {});
test("home empty state", async () => { /* resetMock 成无实例 → 显示新建按钮 */ });
test("launch without account prompts to add one", async () => {});
test("crash opens log panel with tail", async () => { /* mock 推 instance://exit code 1 → 面板显示最后一行 */ });
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**（布局、间距、动画对照原型）
- [ ] **Step 5: 提交** — `feat(ui): home, instances, new instance wizard`

---

### Task 17: 界面：实例详情、设置、账号登录

**Files:**
- Create: `ui/src/pages/{InstanceDetailPage.tsx,SettingsPage.tsx}`、`ui/src/components/{ModList.tsx,DeviceLoginDialog.tsx,AccountList.tsx,MemorySlider.tsx}`
- Test: 同名 `*.test.tsx`

**Interfaces:**
- Produces:
  - 实例详情：头部（图块、名字、版本 · 加载器 · 模组数、开始游戏、打开文件夹、删除（确认对话框，说明会移到回收区））；标签：概览（游戏版本、加载器、Java、内存）、模组（搜索框、列表、每行一个开关调用 `instanceModToggle`；原版实例显示空状态）、设置（内存滑块 1–系统内存一半，按 512 MB 步进，显示推荐值；Java 自动 / 手动路径；JVM 参数文本框；保存调用 `instanceUpdate`）
  - 设置页：语言（中文 / English，立即生效并保存）、主题（跟随系统 / 浅色 / 深色）、强调色（5 个：`#34D399`、`#7DD3FC`、`#FDBA74`、`#C4B5FD`、`#F9A8D4`）、下载线程（1–64）、新实例默认内存、账号列表（当前使用标记、切换、删除）、"添加微软账号"、"添加离线账号"、关于（版本号、检查更新、GitHub 链接、许可证）
  - `DeviceLoginDialog`：显示 `user_code`（大号等宽字体、复制按钮）和 `verification_uri`（"打开网页"按钮），倒计时 `expires_in`；收到 `auth://done` 关闭并刷新账号；`auth://failed` 显示 `errorText`；`error.auth.not_configured` 时说明"开发者还没配置微软登录，请先用离线账号"
  - 检查更新：用 `@tauri-apps/plugin-updater` 的 `check()`；浏览器模式下按钮隐藏

- [ ] **Step 1: 写失败的测试**

```tsx
test("toggling a mod calls backend and updates row", async () => {});
test("language switch applies immediately and persists", async () => { /* 点 English → 标题变英文；mock 的 settings.language == "en" */ });
test("theme setting sets data-theme", async () => {});
test("offline account name validation message", async () => { /* 输入 "a b" → 显示 error.auth.invalid_name 的文案 */ });
test("device login dialog shows code and closes on done", async () => {});
test("delete instance asks for confirmation", async () => {});
```

- [ ] **Step 2–4: 失败 → 实现 → 通过**
- [ ] **Step 5: 提交** — `feat(ui): instance detail, settings, account login`

---

### Task 18: Playwright 端到端测试

**Files:**
- Create: `ui/playwright.config.ts`、`ui/e2e/{new-instance.spec.ts,launch.spec.ts,settings.spec.ts,a11y.spec.ts}`
- Test: 即上面这些文件

**Interfaces:**
- Consumes: `pnpm -C ui dev`（浏览器里自动用 `mockBackend`）

- [ ] **Step 1: 写测试**
  - `new-instance.spec.ts`：新建 1.20.1 Fabric 实例 → 胶囊出现进度并变成"全部完成" → 实例页有新卡片
  - `launch.spec.ts`：添加离线账号 → 首页点开始游戏 → 按钮变"游戏运行中"；模拟崩溃 → 日志面板出现
  - `settings.spec.ts`：切 English 后刷新页面仍是英文；切浅色主题后 `html[data-theme=light]`
  - `a11y.spec.ts`：用 `@axe-core/playwright` 检查首页、实例页、设置页、向导，零个 serious / critical 问题；只用键盘（Tab / Enter / Space）完成新建实例
  - 两个视口：1280×800 和 390×844（窄屏下侧边栏变成顶部，不出现横向滚动）
- [ ] **Step 2: 运行** — `pnpm -C ui exec playwright test`，预期全部 PASS（云端用预装的 Chromium，不要运行 `playwright install`）
- [ ] **Step 3: 截两个视口下首页和向导的截图给用户看**（`deviceScaleFactor: 2`）
- [ ] **Step 4: 提交** — `test(ui): playwright end-to-end flows`

---

### Task 19: CI、发布、自动更新

**Files:**
- Create: `.github/workflows/{ci.yml,release.yml}`、`scripts/check-i18n-keys.mjs`、`scripts/check-bindings.sh`

**Interfaces:**
- Produces:
  - `ci.yml`（push 和 pull_request）：矩阵 `ubuntu-22.04`、`windows-latest`、`macos-latest`。步骤：装 Linux 依赖（`libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xvfb`）→ Rust stable + `Swatinem/rust-cache` → pnpm + Node 22 → `cargo fmt --all --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace` → `bash scripts/check-bindings.sh`（重新导出类型，`git diff --exit-code ui/src/bindings`）→ `node scripts/check-i18n-keys.mjs` → `pnpm -C ui install --frozen-lockfile && pnpm -C ui lint && pnpm -C ui typecheck && pnpm -C ui test` → 只在 ubuntu：`pnpm -C ui exec playwright install --with-deps chromium && pnpm -C ui exec playwright test`
  - `release.yml`（推 `v*` 标签）：`tauri-apps/tauri-action` 矩阵 `windows-latest`、`ubuntu-22.04`、`macos-latest`（`--target aarch64-apple-darwin` 和 `--target x86_64-apple-darwin`）；环境变量 `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（secrets）、`CAIRN_MSA_CLIENT_ID`（vars）；`args` 里用 `--config` 注入 `{"plugins":{"updater":{"pubkey":"${{ vars.TAURI_UPDATER_PUBKEY }}"}},"bundle":{"createUpdaterArtifacts":true}}`；没有设置 pubkey 变量时跳过 updater 产物（步骤里判断）；产物发布为草稿 Release，`latest.json` 一起上传
  - `check-i18n-keys.mjs`：扫描 `crates/**/*.rs` 里 `AppError::new("...")` 的 key，缺翻译就失败并列出缺的 key
- [ ] **Step 1: 本地跑一遍 CI 里能在 Linux 上跑的命令**，预期全部通过
- [ ] **Step 2: 推送后在 GitHub Actions 看到三个系统都是绿色**（需要仓库已建好，见 Task 20）
- [ ] **Step 3: 提交** — `ci: test matrix, release and updater workflows`

---

### Task 20: 文档和用户操作指南

**Files:**
- Create: `README.md`（英文）、`README.zh-CN.md`、`CONTRIBUTING.md`、`docs/setup/{01-github-repo.md,02-network-allowlist.md,03-azure-app.md,04-curseforge-key.md,05-updater-key.md}`

**Interfaces:**
- Produces:
  - README：一句话介绍、特色（阶段 1 已有的功能；阶段 2、3 标"开发中"）、截图（Task 18 的截图）、下载安装（含 SmartScreen 和 macOS"右键 → 打开"的说明）、从源码构建、许可证、声明"本项目与 Mojang、Microsoft 无关，不分发游戏文件，玩家需要拥有正版账号"
  - CONTRIBUTING：系统依赖、`pnpm -C ui dev`、`cargo tauri dev`、跑测试、加翻译（两个 json 同时改 + CI 检查）、提交规范
  - `docs/setup/` 每篇：要做什么、为什么、一步一步点哪里（中文，适合第一次做的人）、做完后告诉 Claude 什么
  - `05-updater-key.md`：运行 `pnpm -C ui tauri signer generate -w ~/.tauri/cairn.key`，私钥和密码放进 Secrets（`TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`），公钥放进 Variables（`TAURI_UPDATER_PUBKEY`）
- [ ] **Step 1: 写文档**
- [ ] **Step 2: 检查** — 文档里提到的每个命令都在本地跑过一次；每个链接都能打开
- [ ] **Step 3: 提交** — `docs: readme, contributing, setup guides`

---

## 阶段 1 完成的标准

- `cargo test --workspace`、`pnpm -C ui test`、Playwright 全部通过；GitHub Actions 三个系统都是绿色
- Task 13 的冒烟测试在云端启动到了主菜单（需要白名单；没批下来就在交付说明里写明）
- 推一个 `v0.1.0` 标签能打出 5 种安装包的草稿 Release
- 用 superpowers:requesting-code-review 做一次全分支审查，处理完意见
