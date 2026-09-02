use crate::cli::{ListArgs, ListKind, OutputFormat, StatusFilter};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{Impact, ItemStatus, ListItem, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListData {
    pub items: Vec<ListItem>,
    pub count: usize,
    pub total: usize,
    pub truncated: bool,
}

pub fn run(args: ListArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    if args.kind != ListKind::Cut && args.impact.is_some() {
        return Err(AppError::invalid_argument(
            "--impact is only available with --kind cut",
            "Remove --impact or use `blotter list --kind cut --impact low|material|blocking`.",
        ));
    }
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;
    let resolved = store::discover(file)?;
    let store::LoadedFold {
        items,
        mut warnings,
    } = store::load_folded(&resolved)?;
    let mut items: Vec<_> = items
        .into_iter()
        .filter(|item| matches_filters(item, &args, since.as_ref()))
        .collect();
    let total = items.len();
    items.truncate(args.limit);
    let data = ListData {
        count: items.len(),
        total,
        truncated: total > items.len(),
        items,
    };
    if total == 0 {
        warnings.push(
            match args.kind {
                ListKind::Cut => "no cuts matched; try --status all or broader filters",
                ListKind::Dogear => "no dogears matched; try --status all or broader filters",
                ListKind::All => "no records matched; try --status all or broader filters",
            }
            .into(),
        );
    }
    if args.format == OutputFormat::Md {
        write_markdown(&data.items, &warnings)?;
    } else {
        let mut meta = Meta::new();
        meta.file = Some(resolved.path.to_string_lossy().into_owned());
        meta.warnings = warnings;
        output::write_success(data, pretty, meta)
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(0)
}

fn matches_filters(item: &ListItem, args: &ListArgs, since: Option<&Timestamp>) -> bool {
    let kind_matches = match args.kind {
        ListKind::Cut => item.kind == "cut",
        ListKind::Dogear => item.kind == "dogear",
        ListKind::All => true,
    };
    let status_matches = match args.status {
        StatusFilter::Open => item.status == ItemStatus::Open,
        StatusFilter::Resolved => item.status == ItemStatus::Resolved,
        StatusFilter::All => true,
    };
    kind_matches
        && status_matches
        && args.agent.as_ref().is_none_or(|agent| &item.agent == agent)
        && args.tag.as_ref().is_none_or(|tag| item.tags.contains(tag))
        && args
            .impact
            .is_none_or(|impact| item.impact == Some(impact))
        && since.is_none_or(|threshold| {
            item.ts
                .parse::<Timestamp>()
                .is_ok_and(|timestamp| timestamp >= *threshold)
        })
}

fn write_markdown(items: &[ListItem], warnings: &[String]) -> AppResult<()> {
    let mut output = output::stdout_writer()
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    for impact in [Impact::Blocking, Impact::Material, Impact::Low] {
        let matching: Vec<_> = items
            .iter()
            .filter(|item| item.impact == Some(impact))
            .collect();
        if matching.is_empty() {
            continue;
        }
        writeln!(
            output,
            "## {}",
            match impact {
                Impact::Blocking => "Blocking",
                Impact::Material => "Material",
                Impact::Low => "Low",
            }
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for item in matching {
            write_markdown_item(&mut output, item)?;
        }
    }
    let dogears: Vec<_> = items.iter().filter(|item| item.kind == "dogear").collect();
    if !dogears.is_empty() {
        writeln!(output, "## Dogears")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for item in dogears {
            write_markdown_item(&mut output, item)?;
        }
    }
    for warning in warnings {
        writeln!(output, "> note: {warning}")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    output
        .flush()
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(())
}

fn write_markdown_item(output: &mut impl Write, item: &ListItem) -> AppResult<()> {
    let id = if item.status == ItemStatus::Resolved {
        format!("~~{}~~", item.id)
    } else {
        item.id.clone()
    };
    let tags = if item.tags.is_empty() {
        String::new()
    } else {
        format!(" ({})", item.tags.join(","))
    };
    // Every interpolated field can carry embedded newlines (resolve only rejects
    // whitespace-only values), so each rendered line is collapsed as a whole.
    let line = format!("- [{id}] {} — {}, {}{tags}", item.text, item.agent, item.ts);
    writeln!(output, "{}", crate::output::collapse_markdown_text(&line))
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    if let Some(resolution) = &item.resolution {
        let mut line = format!("resolved {} by {}", resolution.ts, resolution.agent);
        if let Some(commit) = &resolution.commit {
            line.push_str(&format!(" ({commit})"));
        }
        if let Some(pr) = &resolution.pr {
            line.push_str(&format!(" pr {pr}"));
        }
        if let Some(task) = &resolution.task {
            line.push_str(&format!(" task {task}"));
        }
        if let Some(note) = resolution
            .note
            .as_deref()
            .map(crate::output::collapse_markdown_text)
            .filter(|note| !note.is_empty())
        {
            line.push_str(&format!(": {note}"));
        }
        writeln!(
            output,
            "  - {}",
            crate::output::collapse_markdown_text(&line)
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(())
}
