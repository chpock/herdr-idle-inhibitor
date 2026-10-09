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
    path::Path,
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
    let status = popup_command(&r.herdr_bin)
        .status()
        .await
        .map_err(|error| anyhow::anyhow!("Cannot execute Herdr popup command: {error}"))?;
    anyhow::ensure!(
        status.success(),
        "Herdr popup command failed ({status}); see Herdr's error output"
    );
    Ok(())
}

fn popup_command(herdr_bin: &Path) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(herdr_bin);
    command.args([
        "plugin",
        "pane",
        "open",
        "--plugin",
        herdr_idle_inhibitor::herdr::discovery::PLUGIN_ID,
        "--entrypoint",
        "status",
    ]);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_command_uses_named_flags_and_the_manifest_entrypoint() {
        let manifest: toml::Value = toml::from_str(include_str!("../herdr-plugin.toml")).unwrap();
        let program = Path::new("herdr");
        let command = popup_command(program);
        let command = command.as_std();
        assert_eq!(command.get_program(), program);
        let arguments: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        assert_eq!(
            arguments,
            [
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-idle-inhibitor",
                "--entrypoint",
                "status"
            ]
        );
        assert_eq!(manifest["id"].as_str(), Some(arguments[4]));
        let pane = manifest["panes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|pane| pane["id"].as_str() == Some(arguments[6]))
            .unwrap();
        assert_eq!(pane["placement"].as_str(), Some("popup"));
        assert_eq!(pane["command"].as_array().unwrap()[1].as_str(), Some("_ui"));
        let action = manifest["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"].as_str() == Some("show"))
            .unwrap();
        assert_eq!(
            action["command"].as_array().unwrap()[1].as_str(),
            Some("_open")
        );
    }
}
