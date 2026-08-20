//! The retired Claude Code auto-capture lane (design doc r32).
//!
//! `hook exec claude-code` no longer files cuts and `hook install claude-code` no longer
//! exists. The receiver stays because a settings file installed against an older binary keeps
//! firing this exact command line: rejecting it would put a usage error and a non-zero exit
//! into a host session's hook channel, which the lane's fail-open rule forbids. So it drains
//! stdin, writes nothing, and exits 0.

use crate::cli::{HookArgs, HookCommand, HookExecArgs};
use crate::error::AppResult;
use std::io::{Read, Write};

const HOOK_INPUT_LIMIT: u64 = 1024 * 1024;

pub fn run(args: HookArgs) -> AppResult<i32> {
    match args.command {
        HookCommand::Exec(args) => {
            exec(args);
            Ok(0)
        }
    }
}

/// Reads and discards the harness payload, then reports the retirement under
/// `BLOTTER_HOOK_EXPLAIN=1`. Draining matters: a hook process that exits without reading
/// leaves the harness writing to a closed pipe.
pub fn exec(_args: HookExecArgs) {
    let mut sink = std::io::sink();
    let _ = std::io::copy(
        &mut std::io::stdin().lock().take(HOOK_INPUT_LIMIT),
        &mut sink,
    );
    if std::env::var("BLOTTER_HOOK_EXPLAIN").is_ok_and(|value| value == "1") {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "hook exec: the claude-code auto-capture lane is retired; nothing is filed. Remove the hooks.PostToolUseFailure entry naming this command from your Claude Code settings."
        );
    }
}
