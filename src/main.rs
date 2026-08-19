use blotter::cli::{Cli, Command};
use blotter::error::AppError;
use clap::Parser;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => match error.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                let _ = error.print();
                std::process::exit(0);
            }
            _ => {
                let app_error = AppError::invalid_argument(
                    error.to_string(),
                    "Run `blotter --help` or `blotter schema` for accepted commands and values.",
                );
                std::process::exit(blotter::output::write_error(&app_error));
            }
        },
    };
    if cli.is_hook_exec() {
        std::process::exit(blotter::commands::run_hook_exec(cli));
    }
    let code = if let Command::Schema { target } = &cli.command {
        blotter::commands::run_schema(*target, cli.pretty)
    } else {
        blotter::commands::validate_args(&cli)
            .and_then(|()| blotter::effective_now())
            .and_then(|now| blotter::commands::run(cli, now))
    }
    .unwrap_or_else(|error| blotter::output::write_error(&error));
    std::process::exit(code);
}
