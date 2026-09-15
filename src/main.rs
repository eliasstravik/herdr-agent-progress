mod hooks;
mod publisher;
mod runtime;
mod setup;
mod state;

use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand};
use runtime::{Paths, Runtime};
use serde_json::json;

#[derive(Parser)]
#[command(version, about = "Agent-estimated task progress for Herdr")]
struct Cli {
    #[arg(long, alias = "skill", exclusive = true)]
    instructions: bool,
    /// Administrative endpoint. Reporting always uses the caller's inherited endpoint.
    #[arg(long, global = true)]
    endpoint: Option<String>,
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Args)]
struct Bound {
    #[arg(long)]
    binding: String,
}
#[derive(Subcommand)]
enum Action {
    Context(Bound),
    Status(Bound),
    Begin {
        #[command(flatten)]
        bound: Bound,
        #[arg(long)]
        expected_task: String,
        #[arg(long)]
        title: String,
    },
    Report {
        #[command(flatten)]
        bound: Bound,
        #[arg(long)]
        task: String,
        #[arg(long,required_unless_present="unknown",conflicts_with="unknown",value_parser=clap::value_parser!(u8).range(0..=100))]
        percent: Option<u8>,
        #[arg(long)]
        unknown: bool,
        #[arg(long)]
        activity: String,
    },
    Clear {
        #[command(flatten)]
        bound: Bound,
        #[arg(long)]
        task: String,
    },
    Configure(setup::Configure),
    Unconfigure,
    Doctor,
    /// Human-only fallback: bind a selected live Claude/Codex and print its instructions.
    Activate {
        #[arg(long)]
        pane: Option<String>,
    },
    Start,
    /// Start when configured; a fresh install waits for explicit Configure.
    Startup,
    Stop,
    #[command(hide = true)]
    Serve,
    #[command(hide = true)]
    Hook {
        #[arg(long,value_parser=["claude","codex"])]
        agent: String,
    },
}

fn run(cli: Cli) -> Result<()> {
    if cli.instructions {
        print!("{}", hooks::INSTRUCTIONS);
        return Ok(());
    }
    let action = cli.command.context("Choose a command; see --help")?;
    let paths = Paths::discover()?;
    if matches!(action, Action::Startup) && !paths.enabled() {
        println!(
            "Run herdr plugin action invoke configure --plugin agent-progress to set up progress reporting."
        );
        return Ok(());
    }
    if let Action::Hook { agent } = &action {
        // Context-only hooks never block tools or the agent's work.
        if let Err(e) = hooks::run(agent, &paths) {
            eprintln!("Agent progress unavailable: {e:#}");
        }
        return Ok(());
    }
    let reporting = matches!(
        action,
        Action::Context(_)
            | Action::Status(_)
            | Action::Begin { .. }
            | Action::Report { .. }
            | Action::Clear { .. }
    );
    ensure!(
        !reporting || cli.endpoint.is_none(),
        "Reporting commands cannot override the caller endpoint"
    );
    let rt = Runtime::new(cli.endpoint)?;
    paths.init()?;
    match action {
        Action::Configure(options) => setup::configure(&options, &rt, &paths),
        Action::Unconfigure => setup::unconfigure(&rt, &paths),
        Action::Start | Action::Startup => publisher::start(&rt, &paths),
        Action::Stop => publisher::stop(&rt, &paths),
        Action::Serve => publisher::serve(&rt, &paths),
        Action::Doctor => {
            let snapshot = rt.call(&["api", "snapshot"])?;
            println!(
                "{}",
                json!({"configured":paths.enabled(),"herdr_version":snapshot["snapshot"]["version"],"config_dir":paths.config,"state_dir":paths.state,
                "automatic_adapters":"Claude Code and Codex hooks; native trust review required",
                "platform":"Unix process ancestry and start-time verification; Windows unavailable",
                "other_agents":"unverified/unavailable; --instructions provides the reporting convention but cannot mint a binding",
                "log":paths.state.join("publisher.log")})
            );
            Ok(())
        }
        Action::Activate { pane } => {
            ensure!(paths.enabled(), "Plugin is not configured");
            let pane = pane
                .or_else(|| std::env::var("HERDR_PANE_ID").ok())
                .context("Select a live pane with --pane or a Herdr plugin action")?;
            let live = rt.call(&["pane", "get", &pane])?;
            let identity = rt.identity(&live["pane"], false)?;
            let mut c = state::open(&paths.db())?;
            let tx = state::transaction(&mut c)?;
            let fresh = rt.call(&["pane", "get", &pane])?;
            ensure!(
                rt.identity(&fresh["pane"], false)? == identity,
                "Agent changed while activation waited"
            );
            let s = state::bootstrap(&tx, identity)?;
            tx.commit()?;
            publisher::start(&rt, &paths)?;
            println!("{}", hooks::context(&s, &paths)?);
            Ok(())
        }
        action => {
            ensure!(
                paths.enabled(),
                "Plugin is not configured or its Herdr registration is disabled/missing"
            );
            let pane = rt.current()?;
            let identity = rt.identity(&pane, true)?;
            let mut c = state::open(&paths.db())?;
            let tx = state::transaction(&mut c)?;
            ensure!(
                rt.identity(&rt.current()?, true)? == identity,
                "Agent changed while command waited"
            );
            let binding = match &action {
                Action::Context(b) | Action::Status(b) => &b.binding,
                Action::Begin { bound, .. }
                | Action::Report { bound, .. }
                | Action::Clear { bound, .. } => &bound.binding,
                _ => unreachable!(),
            };
            let mut s = state::bound(&tx, &identity, binding)?;
            match action {
                Action::Context(_) => println!("{}", hooks::context(&s, &paths)?),
                Action::Status(_) => println!(
                    "{}",
                    json!({"task":s.task,"binding":s.binding,"tokens":state::tokens(&s,runtime::now())})
                ),
                Action::Begin {
                    expected_task,
                    title,
                    ..
                } => {
                    state::begin(&mut s, &expected_task, &title)?;
                    state::save(&tx, &s)?;
                    println!("{}", json!({"task":s.task}));
                }
                Action::Report {
                    task,
                    percent,
                    activity,
                    ..
                } => {
                    state::report(&mut s, &task, percent, &activity, runtime::now())?;
                    state::save(&tx, &s)?;
                    println!("{}", json!({"task":s.task}));
                }
                Action::Clear { task, .. } => {
                    state::clear(&mut s, &task)?;
                    state::save(&tx, &s)?;
                    println!("{}", json!({"task":s.task}));
                }
                _ => unreachable!(),
            }
            tx.commit()?;
            publisher::start(&rt, &paths)?;
            Ok(())
        }
    }
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("herdr-progress: {e:#}");
        std::process::exit(1);
    }
}
