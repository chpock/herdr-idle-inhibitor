use clap::{Parser, Subcommand};
use herdr_idle_inhibitor::{
    runtime::{
        bootstrap,
        ipc::{self, Operation},
        paths::Paths,
    },
    status::Status,
};
use std::{
    io::{self, Write},
    process::ExitCode,
};
#[derive(Parser)]
#[command(version, about = "Idle-only sleep inhibition for working Herdr agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Status {
        #[arg(long, required = true)]
        json: bool,
    },
    #[command(name = "_ensure", hide = true)]
    Ensure,
    #[command(name = "_serve", hide = true)]
    Serve,
    #[command(name = "_open", hide = true)]
    Open,
    #[command(name = "_ui", hide = true)]
    Ui,
}
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Commands::Status { .. } = cli.command {
        let result = match Paths::get() {
            Ok(paths) => ipc::query(&paths.endpoint, Operation::GetStatus { details: false }).await,
            Err(_) => Err(ipc::QueryError {
                code: "query_io_error",
                exit: 5,
            }),
        };
        let (value, exit) = match result {
            Ok(r) => (r.status, 0),
            Err(e) => (
                serde_json::to_value(Status::unavailable(
                    e.code,
                    "Monitor query failed; work and native ownership are unavailable",
                ))
                .unwrap(),
                e.exit,
            ),
        };
        let mut stdout = io::stdout().lock();
        return if serde_json::to_writer(&mut stdout, &value).is_ok()
            && stdout.write_all(b"\n").is_ok()
        {
            ExitCode::from(exit)
        } else {
            ExitCode::FAILURE
        };
    }
    let result = match cli.command {
        Commands::Ensure => {
            async {
                let r = herdr_idle_inhibitor::herdr::discovery::Registration::from_env()?;
                bootstrap::ensure(r).await?;
                Ok(())
            }
            .await
        }
        Commands::Serve => {
            async { herdr_idle_inhibitor::controller::serve(Paths::get()?).await }.await
        }
        Commands::Open => open().await,
        Commands::Ui => herdr_idle_inhibitor::ui::run().await,
        Commands::Status { .. } => unreachable!(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "{}",
                herdr_idle_inhibitor::diagnostics::sanitize(&e.to_string())
            );
            ExitCode::FAILURE
        }
    }
}
async fn open() -> anyhow::Result<()> {
    let r = herdr_idle_inhibitor::herdr::discovery::Registration::from_env()?;
    bootstrap::ensure(r.clone()).await?;
    let status = tokio::process::Command::new(&r.herdr_bin)
        .args(["plugin", "pane", "open", "herdr-idle-inhibitor", "status"])
        .status()
        .await?;
    anyhow::ensure!(
        status.success(),
        "Cannot open popup: another popup may be open (ui_busy), or no Herdr UI is attached"
    );
    Ok(())
}
