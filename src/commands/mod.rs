pub mod add;
pub mod archive;
pub mod digest;
pub mod doctor;
pub mod dogear;
pub mod export;
pub mod hook;
pub mod list;
pub mod resolve;
pub mod retrospect;
pub mod schema;
pub mod sweep;
pub mod triage;
pub mod verify;

use crate::cli::{Cli, Command, HookArgs, HookCommand, SchemaTarget};
use crate::error::{AppError, AppResult};
use crate::output;
use jiff::Timestamp;

/// Argument validation that runs before the clock is resolved, so an invalid
/// argument is reported ahead of an unusable environment.
pub fn validate_args(cli: &Cli) -> AppResult<()> {
    match &cli.command {
        Command::Export(args) => export::validate(args),
        _ => Ok(()),
    }
}

pub fn run(cli: Cli, now: Timestamp) -> AppResult<i32> {
    match cli.command {
        Command::Add(args) => add::run(args, cli.file, cli.pretty, now),
        Command::Dogear(args) => dogear::run(args, cli.file, cli.pretty, now),
        Command::List(args) => list::run(args, cli.file, cli.pretty, now),
        Command::Export(args) => export::run(args, cli.file, cli.pretty, now),
        Command::Triage(args) => triage::run(args, cli.file, cli.pretty),
        Command::Verify(args) => verify::run(args, cli.file, cli.pretty),
        Command::Retrospect(args) => retrospect::run(args, cli.file, cli.pretty),
        Command::Digest(args) => digest::run(args, cli.file, cli.pretty, now),
        Command::Sweep(args) => sweep::run(args, cli.file, cli.pretty, now),
        Command::Resolve(args) => resolve::run(args, cli.file, cli.pretty, now),
        Command::Archive(args) => archive::run(args, cli.file, cli.pretty, now),
        Command::Hook(args) => hook::run(args),
        Command::Schema { target } => run_schema(target, cli.pretty),
        Command::Doctor(args) => doctor::run(args, cli.file, cli.pretty, now),
    }
}

pub fn run_schema(target: SchemaTarget, pretty: bool) -> AppResult<i32> {
    output::write_success(schema::contract(target), pretty, output::Meta::new())
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

/// The retired auto-capture receiver (r32). It resolves no clock and touches no log, so no
/// environment fault can make an already-installed harness hook fail into its host session.
pub fn run_hook_exec(cli: Cli) -> i32 {
    let Command::Hook(HookArgs {
        command: HookCommand::Exec(args),
    }) = cli.command
    else {
        unreachable!("run_hook_exec only receives hook exec commands");
    };
    hook::exec(args);
    0
}
