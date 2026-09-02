use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

pub const CONTRACT: u8 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub contract: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Meta {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            contract: CONTRACT,
            file: None,
            agent_source: None,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessEnvelope<T> {
    pub ok: bool,
    pub data: T,
    pub meta: Meta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorBody,
    pub meta: Meta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub details: Value,
    pub retryable: bool,
    pub suggested_fix: String,
}

#[cfg(unix)]
pub fn stdout_writer() -> io::Result<Box<dyn Write>> {
    use std::os::fd::AsFd;

    let stdout = io::stdout();
    let stdout = std::fs::File::from(stdout.as_fd().try_clone_to_owned()?);
    Ok(Box::new(io::BufWriter::new(stdout)))
}

#[cfg(windows)]
pub fn stdout_writer() -> io::Result<Box<dyn Write>> {
    use std::io::IsTerminal;
    use std::os::windows::io::AsHandle;

    let stdout = io::stdout();
    // An interactive console needs Rust's console-aware writer (UTF-8 →
    // WriteConsoleW); a raw duplicated handle goes through WriteFile and the
    // active code page, producing mojibake for non-ASCII text. The handle dup
    // is only for redirected stdout, where write errors must not be suppressed.
    if stdout.is_terminal() {
        return Ok(Box::new(io::BufWriter::new(stdout)));
    }
    let stdout = std::fs::File::from(stdout.as_handle().try_clone_to_owned()?);
    Ok(Box::new(io::BufWriter::new(stdout)))
}

#[cfg(not(any(unix, windows)))]
pub fn stdout_writer() -> io::Result<Box<dyn Write>> {
    Ok(Box::new(io::BufWriter::new(io::stdout())))
}

pub fn write_success<T: Serialize>(data: T, pretty: bool, meta: Meta) -> io::Result<()> {
    let envelope = SuccessEnvelope {
        ok: true,
        data,
        meta,
    };
    let mut output = stdout_writer()?;
    if pretty {
        serde_json::to_writer_pretty(&mut output, &envelope)?;
    } else {
        serde_json::to_writer(&mut output, &envelope)?;
    }
    writeln!(output)?;
    output.flush()
}

pub fn collapse_markdown_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn write_error(error: &AppError) -> i32 {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: error.code.into(),
            message: error.message.clone(),
            details: error.details.clone(),
            retryable: error.retryable,
            suggested_fix: error.suggested_fix.clone(),
        },
        meta: Meta::new(),
    };
    let mut output = io::BufWriter::new(io::stderr().lock());
    // There is no way to report a failure to report a failure.
    let _ = serde_json::to_writer(&mut output, &envelope);
    let _ = writeln!(output);
    error.exit_code
}
