use crate::cli::{ListArgs, ListKind, OutputFormat, StatusFilter};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{Impact, ItemStatus, ListItem, PromotionItem, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

/// The `items` union (r48), discriminated by the existing `kind` field. Serde
/// tells the two arms apart structurally: only a lifecycle record carries
/// `status`, and only a promotion carries `sources`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListEntry {
    Record(Box<ListItem>),
    Promotion(Box<PromotionItem>),
}

impl ListEntry {
    /// The lifecycle arm. Panics on a promotion, so callers that may see either
    /// use `as_record` / `as_promotion`.
    pub fn record(&self) -> &ListItem {
        self.as_record().expect("list entry is a cut or dogear")
    }

    pub fn as_record(&self) -> Option<&ListItem> {
        match self {
            Self::Record(item) => Some(item),
            Self::Promotion(_) => None,
        }
    }

    pub fn as_promotion(&self) -> Option<&PromotionItem> {
        match self {
            Self::Promotion(promotion) => Some(promotion),
            Self::Record(_) => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListData {
    pub items: Vec<ListEntry>,
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
    if args.kind == ListKind::Promotion {
        // A promotion has no status and no tags, so neither filter can select
        // one; `--status all` alone is accepted and is a no-op (r48).
        if matches!(
            args.status,
            Some(StatusFilter::Open | StatusFilter::Resolved)
        ) {
            return Err(AppError::invalid_argument(
                "--status open|resolved is not available with --kind promotion",
                "Promotions have no lifecycle; drop --status or pass --status all.",
            ));
        }
        if args.tag.is_some() {
            return Err(AppError::invalid_argument(
                "--tag is not available with --kind promotion",
                "Promotions carry no tags; drop --tag or list cuts or dogears.",
            ));
        }
    }
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;
    let resolved = store::discover(file)?;
    let store::LoadedFold {
        items,
        promotions,
        mut warnings,
    } = store::load_folded(&resolved)?;
    // Cuts, then dogears, then promotions: the r5 block ordering with a third
    // block appended rather than interleaved (r48).
    let mut items: Vec<_> = items
        .into_iter()
        .filter(|item| matches_filters(item, &args, since.as_ref()))
        .map(|item| ListEntry::Record(Box::new(item)))
        .chain(
            promotions
                .into_iter()
                .filter(|promotion| matches_promotion_filters(promotion, &args, since.as_ref()))
                .map(|promotion| ListEntry::Promotion(Box::new(promotion))),
        )
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
                // No `--status` in the promotion hint: promotions have none.
                ListKind::Promotion => "no promotions matched; try broader filters",
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
        ListKind::Promotion => false,
        ListKind::All => true,
    };
    let status_matches = match args.status.unwrap_or(StatusFilter::Open) {
        StatusFilter::Open => item.status == ItemStatus::Open,
        StatusFilter::Resolved => item.status == ItemStatus::Resolved,
        StatusFilter::All => true,
    };
    kind_matches
        && status_matches
        && args.agent.as_ref().is_none_or(|agent| &item.agent == agent)
        && args.tag.as_ref().is_none_or(|tag| item.tags.contains(tag))
        && args.impact.is_none_or(|impact| item.impact == Some(impact))
        && since.is_none_or(|threshold| {
            item.ts
                .parse::<Timestamp>()
                .is_ok_and(|timestamp| timestamp >= *threshold)
        })
}

/// `--agent` and `--since` apply to a promotion; `--status` never selects one,
/// and an explicitly passed `open`/`resolved` is a request for lifecycle
/// records, so it excludes promotions. `--tag` and `--impact` exclude them
/// under `--kind all` and are rejected outright under `--kind promotion` (r48).
fn matches_promotion_filters(
    promotion: &PromotionItem,
    args: &ListArgs,
    since: Option<&Timestamp>,
) -> bool {
    let kind_matches = matches!(args.kind, ListKind::Promotion | ListKind::All);
    let status_matches = !matches!(
        args.status,
        Some(StatusFilter::Open | StatusFilter::Resolved)
    );
    kind_matches
        && status_matches
        && args.tag.is_none()
        && args.impact.is_none()
        && args
            .agent
            .as_ref()
            .is_none_or(|agent| &promotion.agent == agent)
        && since.is_none_or(|threshold| {
            promotion
                .ts
                .parse::<Timestamp>()
                .is_ok_and(|timestamp| timestamp >= *threshold)
        })
}

fn write_markdown(items: &[ListEntry], warnings: &[String]) -> AppResult<()> {
    let records: Vec<&ListItem> = items
        .iter()
        .filter_map(|entry| match entry {
            ListEntry::Record(item) => Some(item.as_ref()),
            ListEntry::Promotion(_) => None,
        })
        .collect();
    let promotions: Vec<&PromotionItem> = items
        .iter()
        .filter_map(|entry| match entry {
            ListEntry::Promotion(promotion) => Some(promotion.as_ref()),
            ListEntry::Record(_) => None,
        })
        .collect();
    write_markdown_blocks(&records, &promotions, warnings)
}

fn write_markdown_blocks(
    items: &[&ListItem],
    promotions: &[&PromotionItem],
    warnings: &[String],
) -> AppResult<()> {
    let mut output = output::stdout_writer()
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    for impact in [Impact::Blocking, Impact::Material, Impact::Low] {
        let matching: Vec<_> = items
            .iter()
            .copied()
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
    let dogears: Vec<_> = items
        .iter()
        .copied()
        .filter(|item| item.kind == "dogear")
        .collect();
    if !dogears.is_empty() {
        writeln!(output, "## Dogears")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for item in dogears {
            write_markdown_item(&mut output, item)?;
        }
    }
    if !promotions.is_empty() {
        writeln!(output, "## Promotions")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for promotion in promotions {
            write_markdown_promotion(&mut output, promotion)?;
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

fn write_markdown_promotion(output: &mut impl Write, promotion: &PromotionItem) -> AppResult<()> {
    let line = format!(
        "- [{}] {}: {} — {}, {}",
        promotion.id,
        promotion.artifact.kind.as_str(),
        promotion.artifact.r#ref,
        promotion.agent,
        promotion.ts
    );
    writeln!(output, "{}", crate::output::collapse_markdown_text(&line))
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    if let Some(note) = promotion
        .note
        .as_deref()
        .map(crate::output::collapse_markdown_text)
        .filter(|note| !note.is_empty())
    {
        writeln!(output, "  - {note}")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
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
