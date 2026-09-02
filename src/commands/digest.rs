use crate::cli::{DigestArgs, OutputFormat};
use crate::commands::triage::{self, TriageCluster};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{Disposition, ItemStatus, ListItem, format_timestamp, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct DigestData {
    pub chronic: Vec<TriageCluster>,
    pub new_cuts: NewCuts,
    pub open_dogears: OpenDogears,
    pub accepted_cuts: AcceptedCuts,
    pub window: DigestWindow,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptedCuts {
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewCuts {
    pub count: usize,
    pub by_tag: Vec<TagGroup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagGroup {
    pub tag: String,
    pub count: usize,
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenDogears {
    pub count: usize,
    pub items: Vec<OpenDogear>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenDogear {
    pub id: String,
    pub ts: String,
    pub text: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DigestWindow {
    pub since: String,
    pub until: String,
}

pub fn run(
    args: DigestArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let since = parse_since(&args.since, now)?;
    let resolved = store::discover(file)?;
    let store::LoadedFold {
        items, warnings, ..
    } = store::load_folded(&resolved)?;

    let data = digest(items, since, now);
    if args.format == OutputFormat::Md {
        write_markdown(&data, &warnings)?;
    } else {
        let mut meta = Meta::new();
        meta.file = Some(resolved.path.to_string_lossy().into_owned());
        meta.warnings = warnings;
        output::write_success(data, pretty, meta)
            .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    }
    Ok(0)
}

fn digest(items: Vec<ListItem>, since: Timestamp, until: Timestamp) -> DigestData {
    let chronic = triage::triage(items.clone(), 2).clusters;
    let mut tags = BTreeMap::<String, Vec<String>>::new();
    let mut new_cut_count = 0;
    let mut open_dogears = Vec::new();
    let mut accepted_cut_count = 0;

    for item in items {
        if item.status != ItemStatus::Open {
            if item.kind == "cut" {
                // accepted_cuts is judged by disposition_ts alone (r48/r49),
                // never by the cut's ts or the resolution's ts, so a
                // note-only amend cannot move a cut into or out of the
                // window.
                let is_accepted = item.resolution.as_ref().is_some_and(|resolution| {
                    resolution.disposition == Some(Disposition::Accepted)
                });
                if is_accepted {
                    let disposition_timestamp = item
                        .resolution
                        .as_ref()
                        .and_then(|resolution| resolution.disposition_ts.as_deref())
                        .expect("accepted resolutions carry disposition_ts")
                        .parse::<Timestamp>()
                        .expect("folded resolutions have valid RFC3339 disposition_ts");
                    if disposition_timestamp >= since && disposition_timestamp <= until {
                        accepted_cut_count += 1;
                    }
                }
            }
            continue;
        }
        match item.kind.as_str() {
            "cut" => {
                let timestamp = item
                    .ts
                    .parse::<Timestamp>()
                    .expect("folded items have valid RFC3339 timestamps");
                if timestamp >= since && timestamp <= until {
                    new_cut_count += 1;
                    let item_tags = if item.tags.is_empty() {
                        vec![String::new()]
                    } else {
                        item.tags
                    };
                    for tag in item_tags {
                        tags.entry(tag).or_default().push(item.id.clone());
                    }
                }
            }
            "dogear" => open_dogears.push(OpenDogear {
                id: item.id,
                ts: item.ts,
                text: item.text,
                tags: item.tags,
            }),
            _ => unreachable!("folded items are cut or dogear"),
        }
    }

    let mut by_tag: Vec<_> = tags
        .into_iter()
        .map(|(tag, mut ids)| {
            ids.sort();
            TagGroup {
                tag,
                count: ids.len(),
                ids,
            }
        })
        .collect();
    by_tag.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.tag.cmp(&right.tag))
    });
    open_dogears.sort_by(|left, right| {
        right
            .ts
            .parse::<Timestamp>()
            .expect("folded dogears have valid RFC3339 timestamps")
            .cmp(
                &left
                    .ts
                    .parse::<Timestamp>()
                    .expect("folded dogears have valid RFC3339 timestamps"),
            )
            .then_with(|| left.id.cmp(&right.id))
    });

    DigestData {
        chronic,
        new_cuts: NewCuts {
            count: new_cut_count,
            by_tag,
        },
        open_dogears: OpenDogears {
            count: open_dogears.len(),
            items: open_dogears,
        },
        accepted_cuts: AcceptedCuts {
            count: accepted_cut_count,
        },
        window: DigestWindow {
            since: format_timestamp(since),
            until: format_timestamp(until),
        },
    }
}

fn write_markdown(data: &DigestData, warnings: &[String]) -> AppResult<()> {
    let mut output =
        output::stdout_writer().map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    let result: std::io::Result<()> = (|| {
        let mut wrote_section = false;
        if !data.chronic.is_empty() {
            writeln!(output, "## Chronic")?;
            for cluster in &data.chronic {
                writeln!(
                    output,
                    "- {} ({}): {}",
                    crate::output::collapse_markdown_text(&cluster.text),
                    cluster.count,
                    cluster.ids.join(", ")
                )?;
            }
            wrote_section = true;
        }
        if !data.new_cuts.by_tag.is_empty() {
            if wrote_section {
                writeln!(output)?;
            }
            writeln!(output, "## New cuts")?;
            for group in &data.new_cuts.by_tag {
                let tag = if group.tag.is_empty() {
                    "untagged".into()
                } else {
                    crate::output::collapse_markdown_text(&group.tag)
                };
                writeln!(output, "### {tag} ({})", group.count)?;
                for id in &group.ids {
                    writeln!(output, "- {id}")?;
                }
            }
            wrote_section = true;
        }
        if !data.open_dogears.items.is_empty() {
            if wrote_section {
                writeln!(output)?;
            }
            writeln!(output, "## Open dogears")?;
            for dogear in &data.open_dogears.items {
                let tags = if dogear.tags.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", dogear.tags.join(","))
                };
                let line = format!("- [{}] {} — {}{tags}", dogear.id, dogear.text, dogear.ts);
                writeln!(output, "{}", crate::output::collapse_markdown_text(&line))?;
            }
            wrote_section = true;
        }
        if !wrote_section {
            writeln!(output, "No friction in window.")?;
        }
        for warning in warnings {
            writeln!(output, "> note: {warning}")?;
        }
        output.flush()?;
        Ok(())
    })();
    result.map_err(|error| AppError::from_io(error, Path::new("stdout")))
}
