//! OTLP 1.11.0 JSON mapping for the OpenTelemetry file-exporter JSONL format.
//!
//! This module owns the outward bridge only. Blotter's event schema remains
//! internal, and no OpenTelemetry crate is required for this compatibility
//! snapshot.

use crate::cli::{ExportArgs, ExportFormat};
use crate::error::{AppError, AppResult};
use crate::output;
use crate::store;
use crate::{ListItem, Severity, parse_since};
use jiff::Timestamp;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

const EVENT_NAME: &str = "blotter.friction.reported";

/// Argument validation that must run before the clock is resolved, so a missing
/// `--format` is reported ahead of any environment error.
pub fn validate(args: &ExportArgs) -> AppResult<()> {
    let Some(ExportFormat::OtlpJson) = args.format else {
        return Err(AppError::invalid_argument(
            "export requires --format otlp-json",
            "Run `blotter export --format otlp-json`.",
        ));
    };
    Ok(())
}

pub fn run(
    args: ExportArgs,
    file: Option<PathBuf>,
    _pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    validate(&args)?;
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;

    let resolved = store::discover(file)?;
    let store::LoadedFold { items, warnings: _ } = store::load_folded(&resolved)?;

    let data = LogsData::from_items(items, since)?;
    write_otlp_json(&data)?;
    Ok(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogsData {
    resource_logs: Vec<ResourceLogs>,
}

impl LogsData {
    fn from_items(items: Vec<ListItem>, since: Option<Timestamp>) -> AppResult<Self> {
        let mut cuts: Vec<_> = items
            .into_iter()
            .filter(|item| {
                item.kind == "cut"
                    && since.is_none_or(|threshold| {
                        item.ts
                            .parse::<Timestamp>()
                            .is_ok_and(|timestamp| timestamp >= threshold)
                    })
            })
            .collect();
        cuts.sort_by(|left, right| {
            left.ts
                .parse::<Timestamp>()
                .expect("folded items have valid RFC3339 timestamps")
                .cmp(
                    &right
                        .ts
                        .parse::<Timestamp>()
                        .expect("folded items have valid RFC3339 timestamps"),
                )
                .then_with(|| left.id.cmp(&right.id))
        });

        let log_records = cuts
            .iter()
            .map(LogRecord::from_item)
            .collect::<AppResult<Vec<_>>>()?;

        Ok(Self {
            resource_logs: vec![ResourceLogs {
                resource: Resource {},
                scope_logs: vec![ScopeLogs {
                    scope: InstrumentationScope {
                        name: "blotter",
                        version: env!("CARGO_PKG_VERSION"),
                    },
                    log_records,
                }],
            }],
        })
    }
}

#[derive(Serialize)]
struct Resource {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceLogs {
    resource: Resource,
    scope_logs: Vec<ScopeLogs>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopeLogs {
    scope: InstrumentationScope,
    log_records: Vec<LogRecord>,
}

#[derive(Serialize)]
struct InstrumentationScope {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogRecord {
    event_name: &'static str,
    time_unix_nano: String,
    severity_number: u8,
    severity_text: &'static str,
    body: AnyValue,
    attributes: Vec<KeyValue>,
}

impl LogRecord {
    fn from_item(item: &ListItem) -> AppResult<Self> {
        let timestamp = item
            .ts
            .parse::<Timestamp>()
            .expect("folded items have valid RFC3339 timestamps");
        // OTLP timeUnixNano is an unsigned fixed64: a record outside that range
        // rejects the whole export rather than emitting a partial or placeholder.
        let time_unix_nano = u64::try_from(timestamp.as_nanosecond()).map_err(|_| {
            AppError::invalid_input(
                format!(
                    "record {} has timestamp {} outside the OTLP unsigned 64-bit nanosecond range",
                    item.id, item.ts
                ),
                "Correct that record's timestamp, or exclude it with --since, then export again.",
            )
        })?;
        let severity = item.severity.expect("cut items have severity");
        let (severity_number, severity_text) = severity_fields(severity);
        let status = export_status(item);
        let mut attributes = vec![
            string_attribute("blotter.friction.id", &item.id),
            string_attribute("blotter.friction.severity", severity.as_str()),
            string_attribute("blotter.friction.status", status),
            string_attribute("blotter.friction.agent", &item.agent),
            tags_attribute(&item.tags),
            string_attribute("blotter.friction.cwd", &item.cwd),
        ];
        if status == "resolved" {
            let resolution = item
                .resolution
                .as_ref()
                .expect("resolved export status has a resolution");
            attributes.push(string_attribute(
                "blotter.friction.resolved_ts",
                &resolution.ts,
            ));
        }
        Ok(Self {
            event_name: EVENT_NAME,
            time_unix_nano: time_unix_nano.to_string(),
            severity_number,
            severity_text,
            body: AnyValue::StringValue(item.text.clone()),
            attributes,
        })
    }
}

fn severity_fields(severity: Severity) -> (u8, &'static str) {
    match severity {
        Severity::Minor => (9, "INFO"),
        Severity::Major => (13, "WARN"),
        Severity::Blocker => (17, "ERROR"),
    }
}

fn export_status(item: &ListItem) -> &'static str {
    match item.resolution.as_ref() {
        Some(resolution) if resolution.dropped => "dropped",
        Some(_) => "resolved",
        None => "open",
    }
}

#[derive(Serialize)]
struct KeyValue {
    key: &'static str,
    value: AnyValue,
}

fn string_attribute(key: &'static str, value: &str) -> KeyValue {
    KeyValue {
        key,
        value: AnyValue::StringValue(value.to_owned()),
    }
}

fn tags_attribute(tags: &[String]) -> KeyValue {
    KeyValue {
        key: "blotter.friction.tags",
        value: AnyValue::ArrayValue(ArrayValue {
            values: tags.iter().cloned().map(AnyValue::StringValue).collect(),
        }),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum AnyValue {
    StringValue(String),
    ArrayValue(ArrayValue),
}

#[derive(Serialize)]
struct ArrayValue {
    values: Vec<AnyValue>,
}

fn write_otlp_json(data: &LogsData) -> AppResult<()> {
    let mut output =
        output::stdout_writer().map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    serde_json::to_writer(&mut output, data)
        .map_err(|error| AppError::from_io(std::io::Error::other(error), Path::new("stdout")))?;
    writeln!(output).map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    output
        .flush()
        .map_err(|error| AppError::from_io(error, Path::new("stdout")))
}
