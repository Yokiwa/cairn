# Cairn · Minecraft Java 版第三方启动器 · 设计文档

- 日期：2026-11-04
- 状态：已通过用户审阅（2026-11-04）
- 界面原型：https://claude.ai/artifact/HGjG8aTc9U3V1urxufBtBi （示例数据）

## 1. 背景和目标

### 用户说过的

- 做一个**面向海外玩家**的 Minecraft Java 版第三方启动器，好用的程度向国内的 PCL 看齐
- 靠特色功能和其他启动器拉开差距：**模组版本自动适配**（主打）和**存档转移**
- 要一个完整的 GitHub 仓库，功能要做全
- 向 PCL 借鉴三点：**界面好看、动画顺滑**；**新手友好、一键搞定**；**下载快**
- 首发支持 **Windows、macOS、Linux**
- 模组来源：**Modrinth + CurseForge**
- 界面语言：**英文 + 简体中文**，一开始就搭好多语言框架
- 技术栈：**Tauri 2（Rust）+ React + TypeScript**
- 许可证：**GPL-3.0**

### 我的假设（用户可以纠正）

- 公开的开源项目，仓库公开
- 名字定为 **Cairn**（用户 2026-11-04 确认），不用 "PCL"，也不用 Mojang / Minecraft 的 logo
- 不收集任何使用数据（没有遥测）

### 怎样算成功

1. 新手能在三步之内建好实例并进入游戏，包括 Fabric / Forge 等加载器，Java 自动处理
2. 一个装了几十个模组的实例，点"换版本"，启动器给出清楚的适配报告，然后生成一个能直接启动的新实例，原实例不受影响
3. 存档在实例之间转移、导入导出、备份回滚都安全，并且会拦住降级
4. 三个系统上都能安装、自动更新

## 2. 范围

### 分三个阶段，每个阶段做完都是能用的版本

| 阶段 | 内容 |
|---|---|
| 1 能玩 | 微软登录 + 离线账号、版本安装、加载器安装（Fabric / Quilt / Forge / NeoForge）、Java 自动检测和下载、实例管理、启动游戏、下载管理、设置、中英文、浅色 / 深色主题、自动更新、命令行版 |
| 2 特色功能 | 模组自动适配（含"反过来查"）、存档转移 / 导入导出 / 备份回滚、实例内模组管理（启用 / 禁用、检查更新、从文件添加） |
| 3 内容商店 | 在 Modrinth / CurseForge 上搜索并一键安装模组、资源包、光影，自动处理前置 |

### 不做的（这一版）

- 崩溃日志自动分析（用户已确认不放进第一版）
- 代码签名（要付费证书，以后由用户决定）
- 国内下载镜像（目标用户在海外）
- 整合包（`.mrpack` / CurseForge modpack）的导入导出
- 多人服务器列表、皮肤管理
- 低于 1.8 的游戏版本只尽力支持，不做测试保证

## 3. 架构

### 仓库结构

```
crates/core/        Rust 核心库：所有业务逻辑，不依赖 Tauri，可单独测试
crates/cli/         命令行版 `cairn`：不开界面也能完成所有核心操作
src-tauri/          Tauri 2 外壳：把核心库包装成 command，把进度、日志推给界面
ui/                 React + TypeScript 界面（Vite）
docs/               设计文档、实现计划、需要用户本人办的事
.github/workflows/  CI 和发布
```

用 Cargo workspace 管理 `crates/*` 和 `src-tauri`，用 pnpm 管理 `ui/`。

### 核心库模块

每个模块只做一件事，通过公开的函数和类型交互：

| 模块 | 负责 | 主要依赖 |
|---|---|---|
| `paths` | 各系统的数据目录、实例目录、共享缓存目录 | — |
| `http` | 统一的 HTTP 客户端：User-Agent、超时、重试、频率限制、本地响应缓存 | reqwest |
| `download` | 并行下载队列：断点续传、SHA1 / SHA512 校验、进度事件、取消 | `http` |
| `tasks` | 后台任务管理：每个任务有 ID、进度、可取消；界面和命令行共用 | — |
| `versions` | Mojang 版本清单、version JSON 解析（规则、参数、natives）、下载游戏本体、库文件、资源文件 | `download` |
| `java` | 查找本机 Java、读取版本、按 `javaVersion.majorVersion` 选择；缺的从 Adoptium 下载 Temurin JRE | `download` |
| `loaders` | 安装 Fabric、Quilt、Forge、NeoForge，产出合并后的启动配置 | `versions`、`java` |
| `instances` | 实例的创建、复制、删除、读写 `instance.json`、模组文件列表 | `paths` |
| `auth` | 微软账号登录链（设备码 → Xbox Live → XSTS → Minecraft）、续期；离线账号；凭证存系统钥匙串 | keyring |
| `launch` | 拼启动命令、启动进程、收集日志、跟踪正在运行的实例 | `versions`、`java`、`auth` |
| `mods` | Modrinth / CurseForge 接口客户端、jar 元数据解析、哈希（SHA1、CurseForge 指纹） | `http` |
| `resolver` | **主打功能**：模组版本自动适配、反过来查 | `mods`、`instances` |
| `saves` | 读取 `level.dat`、存档列表、转移检查、备份 / 回滚、导入导出、发现其他启动器的存档 | `instances` |
| `settings` | 全局设置读写 | `paths` |

### 界面和后端怎么通信

- 界面调用 Tauri command，比如 `instances_list`、`instance_create`、`resolver_plan`、`saves_transfer_check`
- 耗时操作立即返回一个任务 ID，进度通过事件推送：`task://progress`、`task://done`、`task://failed`
- 游戏日志通过 `instance://log` 事件推送
- 所有 command 返回 `Result<T, AppError>`；`AppError` 带一个翻译 key（界面用它显示中英文说明）和技术细节

### 数据存放

数据目录：Windows `%APPDATA%\Cairn`，macOS `~/Library/Application Support/Cairn`，Linux `~/.local/share/cairn`。

```
settings.json                 全局设置
accounts.json                 账号列表（只有用户名、UUID、类型；凭证在系统钥匙串里）
instances/<id>/instance.json  实例配置
instances/<id>/minecraft/     游戏目录：mods、saves、config、resourcepacks、shaderpacks、options.txt
instances/<id>/logs/          启动器记录的游戏日志
shared/versions/              游戏本体 jar 和 version JSON
shared/libraries/             库文件（所有实例共用）
shared/assets/                资源文件（所有实例共用）
java/<发行版-版本>/            启动器下载的 Java
backups/<存档 ID>/             存档备份 zip 和索引
cache/http/                   接口响应缓存
```

`instance.json` 示例：

```json
{
  "schema": 1,
  "id": "survival-1-20-1",
  "name": "Survival 1.20.1",
  "minecraft": "1.20.1",
  "loader": { "kind": "fabric", "version": "0.16.9" },
  "java": { "mode": "auto" },
  "memory_mb": 6144,
  "jvm_args": [],
  "created_at": "2026-11-04T18:00:00Z",
  "last_played_at": null,
  "mods": { "<文件名>": { "source": "modrinth", "project_id": "...", "version_id": "..." } }
}
```

`mods` 只记录"这个文件是从哪里装的"，方便以后检查更新和换版本；真实的模组列表以 `mods/` 文件夹为准。

## 4. 阶段 1：能玩

### 版本安装

- 版本清单来自 `piston-meta.mojang.com` 的 `version_manifest_v2.json`，默认只显示正式版，快照版要打开开关
- 解析 version JSON：`rules`（按系统、架构、特性过滤）、新旧两种参数格式（`arguments` 和 1.13 以前的 `minecraftArguments`）、`natives`（旧版本要解压到实例的 natives 目录）、`assetIndex`、`logging`
- 资源文件按资源索引下载到 `shared/assets/objects`；1.7.2 及以前的 `legacy` / `pre-1.6` 资源布局也要处理
- 每个文件都用清单里的 SHA1 校验；文件已经存在且校验通过就跳过

### 下载引擎

- 默认 32 个并发，设置里可以调 1–64
- 先写到 `*.part`，校验通过后再改名；服务器支持 Range 时断点续传
- 失败重试 3 次，等待时间 1 秒、2 秒、4 秒；SHA1 不对就删掉重下
- 按文件数和字节数汇报进度，算出实时速度，推给界面右下角的下载胶囊

### Java

- 查找位置：`JAVA_HOME`、`PATH`、各系统常见安装目录（比如 `C:\Program Files\Java`、`/Library/Java/JavaVirtualMachines`、`/usr/lib/jvm`），以及启动器自己的 `java/` 目录
- 读 Java 目录下的 `release` 文件拿到版本号；读不到就运行 `java -version`
- 需要哪个大版本，看 version JSON 的 `javaVersion.majorVersion`；没写就按 8 算
- 缺的时候从 Adoptium API 下载对应系统和架构的 Temurin JRE，解压到 `java/`
- 实例设置里可以手动指定 Java 路径

### 加载器

- **Fabric / Quilt**：从 `meta.fabricmc.net` / `meta.quiltmc.org` 拿启动配置（profile JSON），和原版 version JSON 合并，下载需要的库文件
- **Forge / NeoForge**：下载官方安装器 jar，读里面的 `install_profile.json` 和 `version.json`，下载库文件，再按顺序运行安装器里的 processors（和 Prism Launcher 的做法一样）。Forge 支持 1.12.2 及以上；NeoForge 支持全部版本
- 加载器版本默认选最新的稳定版，实例设置里可以改

### 账号

- **微软账号**：用设备码登录。界面显示一个短代码和网址，用户在浏览器里输入就行；命令行版也能用同一套流程。之后依次换取 Xbox Live、XSTS、Minecraft 的令牌，再查询账号是否拥有游戏和玩家资料
- 需要一个 Azure 应用 ID，并且要通过 Mojang 的审核（见第 10 节）。没批下来之前，微软登录按钮显示"未配置"，离线账号照常能用
- **离线账号**：只用于测试和单机，UUID 按 `OfflinePlayer:<名字>` 生成
- 刷新令牌存在系统钥匙串里（Windows 凭据管理器、macOS 钥匙串、Linux Secret Service）。启动前令牌过期就自动续期，续期失败才让用户重新登录

### 启动

- 拼出 JVM 参数（内存、natives 路径、classpath、日志配置）和游戏参数（用户名、UUID、令牌、游戏目录、资源目录、窗口大小）
- 启动后跟踪进程，日志实时推给界面；`last_played_at` 在启动时更新
- 游戏异常退出时，显示日志的最后 50 行和"打开日志文件夹"按钮
- 默认内存：电脑内存 ≤ 8 GB 推荐 3 GB，≤ 16 GB 推荐 6 GB，更大推荐 8 GB

### 命令行版

```
cairn versions list [--snapshots]
cairn instance create <名字> --mc 1.20.1 [--loader fabric]
cairn instance list
cairn instance launch <实例> [--offline <名字>]
cairn login
cairn mods resolve <实例> --to 1.21.1 [--loader neoforge] [--apply]
cairn saves list | backup <存档> | transfer <存档> --to <实例>
```

## 5. 阶段 2：模组自动适配（主打）

### 入口

实例详情页的"换版本"按钮，首页的"这套模组最高能升到哪个版本"卡片，以及命令行的 `cairn mods resolve`。

### 第 1 步：识别模组

对 `mods/` 下每个 `.jar`（包括 `.jar.disabled`）：
1. `instance.json` 里已经记录了来源，就直接用
2. 否则算 SHA1，批量调用 Modrinth `POST /v2/version_files`
3. Modrinth 没找到的，算 CurseForge 指纹（去掉空白字符后的 MurmurHash2，种子 1），批量调用 CurseForge `POST /v1/fingerprints`
4. 还没找到，就读 jar 里的元数据（`fabric.mod.json`、`quilt.mod.json`、`META-INF/mods.toml`、`META-INF/neoforge.mods.toml`、`mcmod.info`），用模组 ID 和名字去两个平台搜；只有名字和 ID 都对得上才算识别成功
5. 都不行的标为"认不出来"

### 第 2 步：找兼容版本

- Modrinth：`GET /v2/project/{id}/version`，按目标游戏版本和加载器过滤
- CurseForge：`GET /v1/mods/{id}/files`，按 `gameVersion` 和 `modLoaderType` 过滤
- 优先级：正式版 > beta > alpha；同一级别里选最新发布的
- 优先用原来的平台；原平台没有兼容版本，就去另一个平台找同一个模组
- 加载器规则：目标是 Quilt 时，Fabric 版本也算兼容；目标是 NeoForge 时，1.20.1 的 Forge 版本也算兼容（NeoForge 在 1.20.1 上能加载 Forge 模组）；其他组合必须精确匹配

### 第 3 步：补前置

- Modrinth 的 `dependencies` 里 `required`，CurseForge 的 `relationType = 3`，都当作必需前置
- 递归处理，已经在列表里的不重复加；可选前置不自动加

### 第 4 步：查冲突

- 平台标注的不兼容关系（Modrinth `incompatible`，CurseForge `relationType = 5`）
- 同一个模组 ID 出现两次
- 下载完成后、正式启用前，再读一遍每个新 jar 的元数据，检查对游戏版本、加载器、其他模组的版本要求（比如 Fabric 的 `depends` 范围）。不满足的标红

### 第 5 步：适配报告

每个模组一行，状态是下面五种之一：

| 状态 | 含义 | 默认处理 |
|---|---|---|
| ✓ 已适配 | 找到了正式版 | 装上 |
| + 自动补的前置 | 某个模组需要它 | 装上 |
| ! 只有测试版 | 只有 beta / alpha | 装上，报告里标黄 |
| × 目标版本还没有 | 两个平台都没有兼容版本 | 去掉；用户可以改成"先不换版本" |
| ? 认不出来 | 识别失败 | 不复制；用户可以选择"照原样复制" |

### 应用

1. 新建一个实例（名字默认是 "<原名> <目标版本>"），**原实例完全不动**
2. 复制 `config/`、`resourcepacks/`、`shaderpacks/`、`options.txt`
3. 下载报告里要装的模组，原来被禁用的模组在新实例里保持禁用
4. 下载后做第 4 步的二次检查
5. 用户勾选了"同时把存档带过去"，就交给存档模块按第 6 节的流程转移

### 反过来查

- 对每个识别出来的模组，Modrinth 用 `GET /v2/projects?ids=[…]` 一次拿到它支持的全部游戏版本和加载器；CurseForge 用 `latestFilesIndexes`
- 对当前版本及以后的每个正式版，算出"有几个模组能用"，画成条形图
- 这只是快速估算；用户选定版本以后，按第 2–4 步做精确检查

### 平台接口的使用规则

- User-Agent 按 Modrinth 的要求写成 `Cairn/<版本> (github.com/<仓库>)`
- 遵守频率限制：Modrinth 每分钟 300 次，读响应头里的剩余次数；收到 429 就按 `Retry-After` 等
- 响应缓存 1 小时
- 某个平台出错或超时，就只用另一个平台继续，报告顶部注明"CurseForge 暂时连不上，结果可能不全"
- 作者禁止第三方下载的 CurseForge 文件（`downloadUrl` 为空）：报告里提示用户，打开模组页面让用户手动下载，然后启动器监视下载文件夹，用指纹认出下好的文件，自动放进实例

## 6. 阶段 2：存档

### 功能

- 存档页列出所有实例的存档：名字、所在实例、游戏版本、上次游玩时间、备份份数
- 在实例之间复制或移动
- 导出为 `.zip`；从 `.zip` 或文件夹导入
- 自动发现其他启动器的存档：官方启动器的 `.minecraft/saves`、Prism Launcher 的实例、Modrinth App 的 profiles。只读取，不修改它们
- 备份（zip）和回滚：每个存档有一条备份时间线；默认保留最近 10 份，设置里可以改

### 转移前的检查

1. **版本方向**：读 `level.dat`（gzip 压缩的 NBT）里的 `DataVersion`，和目标实例游戏版本的数据版本比较。游戏版本的数据版本从游戏本体 jar 里的 `version.json` 的 `world_version` 字段读取。很老的版本没有这两个字段（存档 1.9 以前、游戏本体 1.14 以前），就改用版本号比较
   - 目标更新：警告"升级是单向的"，并且必须先备份
   - 版本相同：直接复制
   - 目标更旧：**拦住，不提供强行继续的选项**
2. **模组差异**：对比源实例和目标实例的模组 ID，列出目标实例缺少的模组，提醒这些模组的方块和物品会消失
3. **磁盘空间**：剩余空间要大于存档大小的 2 倍（备份 + 复制）
4. **是否在运行**：存档所在实例的游戏正在运行时，拦住

### 规则

- 任何会覆盖或修改存档的操作，执行前都自动备份
- 复制时先写到目标目录下的临时文件夹，完整写完再改名
- 目标实例里已经有同名存档：自动加后缀 `(2)`，不覆盖

## 7. 阶段 3：内容商店

- "发现"页按 模组 / 资源包 / 光影 三类，搜索 Modrinth 和 CurseForge，两个平台的结果合并后去重
- 安装到当前选中的实例：自动筛选兼容这个实例游戏版本和加载器的文件，自动补前置
- 实例里已经装了的显示"已安装"，有新版本的显示"更新"

## 8. 界面

- 原型见本文开头的链接，最终实现以原型的布局和交互为准
- 左边导航：首页、实例、发现、存档、设置；右下角下载胶囊
- 新建实例三步：选版本 → 选加载器 → 命名，Java 和文件下载全部自动
- 实例详情页标签：概览、模组、存档、资源包 / 光影、设置
- 主题：浅色 / 深色，默认跟随系统；强调色可选；所有颜色用 CSS 变量定义
- 动画：页面切换、卡片悬停、弹窗、进度；系统开启"减少动态效果"时关闭
- 多语言：react-i18next，翻译文件在 `ui/src/locales/{en,zh-CN}.json`；默认跟随系统语言，系统语言不是中文时用英文
- 技术：Vite、React、TypeScript、Tailwind CSS、Motion、TanStack Query（后端数据）、Zustand（界面状态）、React Router
- 无障碍：能用键盘操作全部功能，图标按钮有文字标签，文字对比度至少 4.5:1

## 9. 出错处理

- 所有错误统一成 `AppError { key, params, detail }`：`key` 对应翻译文件里的中英文说明，`detail` 是技术细节，界面里可以展开
- 下载：重试和校验见第 4 节
- 没网：已安装的实例照常启动；版本列表和接口结果用缓存；需要联网的按钮显示原因
- 登录：见第 4 节
- 文件写入：重要文件（`instance.json`、`settings.json`、`accounts.json`）都是先写临时文件再改名
- 日志：启动器自己的日志写到数据目录下的 `logs/`，按天滚动，保留 7 天；日志里不写令牌

## 10. 需要用户本人办的事

我会在 `docs/setup/` 里为每一项写好操作步骤：

1. **在 GitHub 上建一个空仓库**，并把它加进这个开发环境
2. **批准网络白名单**：Mojang、Fabric、Quilt、Forge、NeoForge、Modrinth、CurseForge、Adoptium、微软登录相关的域名，用于真实下载测试
3. **Azure 应用注册**：在 Azure 门户注册一个应用，打开"公共客户端流"，拿到应用 ID；再填 Mojang 的表单申请 Minecraft API 权限。审核可能要几天到几周
4. **CurseForge API key**：在 CurseForge 开发者控制台免费申请。key 不写进代码，构建时从 GitHub Secret `CURSEFORGE_API_KEY` 读取；本地开发从环境变量读取；没有 key 时 CurseForge 部分自动隐藏
5. **自动更新签名密钥**：用 Tauri 命令生成一对密钥，私钥放进 GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`

## 11. 测试

- **Rust 单元测试**：每个模块都有。网络请求用 wiremock 回放录好的接口响应，测试不需要联网
- **模组自动适配**测得最细：正常升级、只有 beta、目标版本没有、缺前置、多层前置、不兼容关系、重复模组、认不出的 jar、Modrinth 和 CurseForge 混用、一个平台挂掉、加载器切换
- **存档**：用测试代码生成的 `level.dat` 测版本比较、拦截降级、备份回滚、重名处理
- **真实端到端测试**（需要网络白名单）：用命令行版在云端下载 1.20.1 原版和 Fabric，用软件渲染（Xvfb + Mesa）启动到主菜单；再对一组真实模组跑一次 `mods resolve`
- **界面**：Vitest 组件测试；Playwright 在浏览器里用模拟后端走主要流程（新建实例、换版本、存档转移、切换语言）
- **代码检查**：`cargo fmt`、`cargo clippy -D warnings`、ESLint、`tsc --noEmit`

## 12. CI 和发布

- `ci.yml`：每次推送和 PR，在 Windows、macOS、Linux 上跑代码检查和全部测试
- `release.yml`：推送 `v*` 标签时，用 tauri-action 打包，发布到 GitHub Releases
  - Windows：`.msi`、`.exe`（NSIS）
  - macOS：`.dmg`（Apple 芯片和 Intel 两个版本）
  - Linux：`.AppImage`、`.deb`
- 自动更新：Tauri updater 读 GitHub Releases 里的 `latest.json`，下载后校验签名再安装
- 没有代码签名；README 写清楚 Windows SmartScreen 和 macOS "右键 → 打开"的绕过方法

## 13. 仓库里的文档

- `README.md`（英文）和 `README.zh-CN.md`（中文）：功能介绍、截图、安装、从源码构建
- `CONTRIBUTING.md`：开发环境、目录结构、怎么跑测试、怎么加翻译
- `LICENSE`：GPL-3.0
- `docs/setup/`：第 10 节的操作步骤
- 声明：本项目与 Mojang、Microsoft 无关，不分发游戏文件，玩家需要拥有正版账号

## 14. 风险

| 风险 | 应对 |
|---|---|
| Mojang 不批或很慢才批 Azure 应用 | 离线账号先能用；所有功能都能用离线账号测试 |
| Forge 安装器的 processors 各版本差异大 | 先保证 1.20.1、1.21.x 的 Forge / NeoForge，再逐个补老版本；每个版本都有测试 |
| 平台的版本标注不准（模组说支持实际不行） | 第 4 步下载后再查一次 jar 元数据；报告里给"回到原版本"的退路（原实例没动） |
| CurseForge 第三方下载限制 | 见第 5 节：引导手动下载并自动识别 |
| 云端没有 macOS / Windows | 这两个系统靠 GitHub Actions 的测试和打包来验证 |
