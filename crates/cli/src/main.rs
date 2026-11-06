//! `cairn` — the Cairn launcher from the command line.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use cairn_core::auth::{Account, AccountKind};
use cairn_core::download::{Progress, ProgressFn};
use cairn_core::instances::{LoaderKind, LoaderSpec, NewInstance};
use cairn_core::launch::{launch_instance, prepare_instance, LogFn};
use cairn_core::loaders::list_loader_versions;
use cairn_core::settings::{recommended_memory_mb, Settings};
use cairn_core::versions::manifest::{fetch_manifest, VersionKind};
use cairn_core::{AppError, Core, Paths, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use tokio_util::sync::CancellationToken;

const EN: &str = include_str!("../../../ui/src/locales/en.json");

#[derive(Parser)]
#[command(
    name = "cairn",
    version,
    about = "Cairn, a Minecraft: Java Edition launcher"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Minecraft versions
    Versions {
        #[command(subcommand)]
        cmd: VersionsCmd,
    },
    /// Mod loader versions
    Loaders {
        #[command(subcommand)]
        cmd: LoadersCmd,
    },
    /// Create, list and launch instances
    Instance {
        #[command(subcommand)]
        cmd: InstanceCmd,
    },
    /// Sign in with a Microsoft account
    Login,
    /// Accounts
    Accounts {
        #[command(subcommand)]
        cmd: AccountsCmd,
    },
}

#[derive(Subcommand)]
enum VersionsCmd {
    /// List versions (releases only unless --snapshots)
    List {
        #[arg(long)]
        snapshots: bool,
    },
}

#[derive(Subcommand)]
enum LoadersCmd {
    /// List loader versions for a Minecraft version
    List {
        #[arg(value_parser = parse_kind)]
        kind: LoaderKind,
        #[arg(long)]
        mc: String,
    },
}

#[derive(Subcommand)]
enum InstanceCmd {
    /// Create an instance and install everything it needs
    Create {
        name: String,
        #[arg(long)]
        mc: String,
        /// fabric, quilt, forge or neoforge, optionally with @version
        #[arg(long)]
        loader: Option<String>,
        /// Memory in MB
        #[arg(long)]
        memory: Option<u32>,
    },
    /// List instances
    List,
    /// Launch an instance
    Launch {
        id: String,
        /// Play with an offline account of this name
        #[arg(long)]
        offline: Option<String>,
        /// Stop the game and exit 0 as soon as a log line matches this regex
        #[arg(long)]
        smoke_test: Option<String>,
        /// Seconds to wait for --smoke-test before failing
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },
}

#[derive(Subcommand)]
enum AccountsCmd {
    /// List accounts
    List,
    /// Add an offline account
    AddOffline { name: String },
}

fn parse_kind(s: &str) -> std::result::Result<LoaderKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" => Ok(LoaderKind::NeoForge),
        other => Err(format!(
            "unknown loader \"{other}\" (expected fabric, quilt, forge or neoforge)"
        )),
    }
}

/// English message for an error, from the same translation file as the UI.
fn message(e: &AppError) -> String {
    let table: serde_json::Value = serde_json::from_str(EN).unwrap_or_default();
    let mut text = table["errors"][&e.key]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| e.key.clone());
    for (k, v) in &e.params {
        text = text.replace(&format!("{{{{{k}}}}}"), v);
    }
    text
}

fn progress_bar() -> (ProgressBar, ProgressFn) {
    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template("{msg:>8} [{bar:30}] {pos}/{len} files  {prefix}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
    );
    let b = bar.clone();
    let f: ProgressFn = Arc::new(move |p: &Progress| {
        b.set_message(p.stage.clone());
        b.set_length(p.total_files);
        b.set_position(p.done_files);
        if p.bytes_per_sec > 0 {
            b.set_prefix(format!("{:.1} MB/s", p.bytes_per_sec as f64 / 1_048_576.0));
        }
    });
    (bar, f)
}

async fn pick_loader(core: &Core, spec: &str, mc: &str) -> Result<LoaderSpec> {
    let (kind, version) = match spec.split_once('@') {
        Some((k, v)) => (k, Some(v.to_string())),
        None => (spec, None),
    };
    let kind = parse_kind(kind).map_err(|m| {
        AppError::new("error.loader.unsupported")
            .with_param("loader", kind)
            .with_detail(m)
    })?;
    let version = match version {
        Some(v) => v,
        None => {
            let list = list_loader_versions(core, kind, mc).await?;
            let best = list.iter().find(|v| v.stable).or(list.first());
            best.map(|v| v.version.clone()).ok_or_else(|| {
                AppError::new("error.loader.not_found")
                    .with_param("loader", spec)
                    .with_param("mc", mc)
            })?
        }
    };
    Ok(LoaderSpec { kind, version })
}

async fn account_for(core: &Core, offline: Option<&str>) -> Result<Account> {
    match offline {
        Some(name) => core.auth.add_offline(name).await,
        None => core
            .auth
            .active()?
            .ok_or_else(|| AppError::new("error.auth.no_account")),
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    let paths = Paths::new(Paths::default_root()?);
    let _log = cairn_core::logging::init_logging(&paths).ok();
    let core = Core::new(paths)?;
    match cli.cmd {
        Cmd::Versions {
            cmd: VersionsCmd::List { snapshots },
        } => {
            let m = fetch_manifest(
                &core.http,
                &core.endpoints.mojang_meta,
                &core.paths.cache_dir(),
            )
            .await?;
            for v in m.versions.iter().filter(|v| {
                v.kind == VersionKind::Release || (snapshots && v.kind == VersionKind::Snapshot)
            }) {
                println!("{:<24} {:?}", v.id, v.kind);
            }
        }
        Cmd::Loaders {
            cmd: LoadersCmd::List { kind, mc },
        } => {
            for v in list_loader_versions(&core, kind, &mc).await? {
                println!("{}{}", v.version, if v.stable { "  (stable)" } else { "" });
            }
        }
        Cmd::Instance {
            cmd: InstanceCmd::List,
        } => {
            let (list, errors) = core.instances.list()?;
            if list.is_empty() {
                println!(
                    "No instances. Create one with: cairn instance create <name> --mc <version>"
                );
            }
            for i in list {
                let loader = i
                    .loader
                    .map(|l| format!("{:?} {}", l.kind, l.version))
                    .unwrap_or_else(|| "vanilla".into());
                println!("{:<24} {:<28} {} · {}", i.id, i.name, i.minecraft, loader);
            }
            for e in errors {
                eprintln!("warning: {}", message(&e));
            }
        }
        Cmd::Instance {
            cmd:
                InstanceCmd::Create {
                    name,
                    mc,
                    loader,
                    memory,
                },
        } => {
            let loader = match loader {
                Some(spec) => Some(pick_loader(&core, &spec, &mc).await?),
                None => None,
            };
            let settings = Settings::load(&core.paths)?;
            let default_mem = settings
                .default_memory_mb
                .unwrap_or_else(|| recommended_memory_mb(cairn_core::system::total_memory_mb()));
            let inst = core.instances.create(
                NewInstance {
                    name,
                    minecraft: mc,
                    loader,
                    memory_mb: memory,
                },
                default_mem,
            )?;
            println!(
                "Created instance \"{}\" ({}). Installing…",
                inst.name, inst.id
            );
            let _guard = core.running.try_register_install(&inst.id)?;
            let (bar, progress) = progress_bar();
            let res = prepare_instance(&core, &inst, progress, CancellationToken::new()).await;
            bar.finish_and_clear();
            let (v, java) = res?;
            println!(
                "Ready: {} with Java {} ({})",
                v.id,
                java.major,
                java.path.display()
            );
        }
        Cmd::Instance {
            cmd:
                InstanceCmd::Launch {
                    id,
                    offline,
                    smoke_test,
                    timeout,
                },
        } => {
            let account = account_for(&core, offline.as_deref()).await?;
            core.instances.get(&id)?;
            let cancel = CancellationToken::new();
            let matched = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let re = smoke_test
                .as_deref()
                .map(regex::Regex::new)
                .transpose()
                .map_err(|e| AppError::new("error.parse").with_detail(e.to_string()))?;
            let log: LogFn = {
                let (cancel, matched) = (cancel.clone(), matched.clone());
                Arc::new(move |line: &str| {
                    println!("{line}");
                    if re.as_ref().is_some_and(|r| r.is_match(line)) {
                        matched.store(true, std::sync::atomic::Ordering::SeqCst);
                        cancel.cancel();
                    }
                })
            };
            if smoke_test.is_some() {
                let c = cancel.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(timeout)).await;
                    c.cancel();
                });
            }
            let (bar, progress) = progress_bar();
            let exit = launch_instance(&core, &id, &account.id, log, progress, cancel).await;
            bar.finish_and_clear();
            let exit = exit?;
            if smoke_test.is_some() {
                let ok = matched.load(std::sync::atomic::Ordering::SeqCst);
                eprintln!(
                    "smoke test {}",
                    if ok {
                        "passed"
                    } else {
                        "FAILED (pattern not seen)"
                    }
                );
                return Ok(if ok {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                });
            }
            if exit.code != Some(0) {
                eprintln!("The game exited with code {:?}. Last lines:", exit.code);
                for l in exit.tail {
                    eprintln!("  {l}");
                }
                return Ok(ExitCode::from(1));
            }
        }
        Cmd::Login => {
            let (code, pending) = core.auth.start_device_login().await?;
            println!(
                "Open {} and enter the code: {}",
                code.verification_uri, code.user_code
            );
            let acc = pending.finish().await?;
            println!("Signed in as {}", acc.username);
        }
        Cmd::Accounts {
            cmd: AccountsCmd::List,
        } => {
            let list = core.auth.accounts()?;
            if list.is_empty() {
                println!(
                    "No accounts. Add one with: cairn login  or  cairn accounts add-offline <name>"
                );
            }
            for a in list {
                let kind = match a.kind {
                    AccountKind::Microsoft => "microsoft",
                    AccountKind::Offline => "offline",
                };
                println!(
                    "{} {:<16} {:<10} {}",
                    if a.active { "*" } else { " " },
                    a.username,
                    kind,
                    a.id
                );
            }
        }
        Cmd::Accounts {
            cmd: AccountsCmd::AddOffline { name },
        } => {
            let a = core.auth.add_offline(&name).await?;
            println!("Added offline account {} ({})", a.username, a.id);
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {}", message(&e));
            if !e.detail.is_empty() {
                eprintln!("  ({})", e.detail);
            }
            ExitCode::from(1)
        }
    }
}
