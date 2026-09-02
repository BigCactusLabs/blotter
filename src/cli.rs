use crate::Severity;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "blotter",
    version,
    about,
    long_about = None,
    arg_required_else_help = true,
    subcommand_required = true,
    rename_all = "kebab-case"
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Override log-file discovery for this invocation"
    )]
    pub file: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        help = "Indent the JSON envelope for human reading"
    )]
    pub pretty: bool,

    #[command(subcommand)]
    pub command: Command,
}

const ADD_AFTER_HELP: &str = "\
Admission: file a cut only when at least one of these holds.
  transferable   another agent or user would plausibly hit the same thing
  consequential  cost real time, produced wrong work, forced retries, or stopped the task
  recurring      the same underlying friction has happened before
  misleading     the error pointed at the wrong cause, hid it, or blamed the wrong file
  systemic       a missing affordance, a doc gap, a brittle interface, a reusable footgun
Skip one-off execution slips unless they recur: typos, shell quoting, a bad first guess,
a patch that missed on stale context, a linter correctly rejecting code you just wrote,
a malformed fixture you authored. Severity records consequence, not admission.";

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(alias = "log", after_help = ADD_AFTER_HELP)]
    Add(AddArgs),
    #[command(alias = "idea")]
    Dogear(DogearArgs),
    List(ListArgs),
    Export(ExportArgs),
    Triage(TriageArgs),
    Verify(VerifyArgs),
    Retrospect(RetrospectArgs),
    Digest(DigestArgs),
    Sweep(SweepArgs),
    Resolve(ResolveArgs),
    Archive(ArchiveArgs),
    Schema {
        #[arg(
            value_enum,
            default_value_t = SchemaTarget::All,
            help = "Contract section to emit"
        )]
        target: SchemaTarget,
    },
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(
        value_name = "TEXT",
        help = "Cut text; omit or use - to read from stdin"
    )]
    pub text: Option<String>,
    #[arg(long, help = "Agent name; overrides BLOTTER_AGENT")]
    pub agent: Option<String>,
    #[arg(long = "tag", help = "Tag the cut; repeatable")]
    pub tags: Vec<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = Severity::Minor,
        help = "Consequence, not admission. blocker: could not proceed; major: lost real time or produced wrong work; minor: limited cost, still worth filing"
    )]
    pub severity: Severity,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "TEXT",
        help = "Command that failed"
    )]
    pub cmd: Option<String>,
    #[arg(long = "exit", value_name = "N", help = "Command exit status")]
    pub exit_code: Option<i32>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Read regular UTF-8 PATH (<=1 MiB); best-effort redaction; store sanitized value <=4096 bytes"
    )]
    pub stderr_file: Option<PathBuf>,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "TEXT",
        help = "Additional evidence or filing note"
    )]
    pub evidence: Option<String>,
    #[arg(long, help = "Validate without appending")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct DogearArgs {
    #[arg(
        value_name = "TEXT",
        help = "Dogear text; omit or use - to read from stdin"
    )]
    pub text: Option<String>,
    #[arg(long, help = "Agent name; overrides BLOTTER_AGENT")]
    pub agent: Option<String>,
    #[arg(long = "tag", help = "Tag the dogear; repeatable")]
    pub tags: Vec<String>,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "TEXT",
        help = "Optional research note; leading hyphens accepted"
    )]
    pub evidence: Option<String>,
    #[arg(long, help = "Validate without appending")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = ListKind::Cut,
        help = "Record kind to list"
    )]
    pub kind: ListKind,
    #[arg(
        long,
        value_enum,
        default_value_t = StatusFilter::Open,
        help = "Filter by lifecycle status"
    )]
    pub status: StatusFilter,
    #[arg(long, help = "Filter by agent")]
    pub agent: Option<String>,
    #[arg(long, help = "Filter by tag")]
    pub tag: Option<String>,
    #[arg(long, value_enum, help = "Filter cuts by severity")]
    pub severity: Option<Severity>,
    #[arg(long, help = "Filter since an RFC3339 timestamp or Nd/Nh duration")]
    pub since: Option<String>,
    #[arg(long, default_value_t = 50, help = "Maximum records to return")]
    pub limit: usize,
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Json,
        help = "Output format"
    )]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(long, value_enum, help = "Output bridge format; required: otlp-json")]
    pub format: Option<ExportFormat>,
    #[arg(long, help = "Filter since an RFC3339 timestamp or Nd/Nh duration")]
    pub since: Option<String>,
}

#[derive(Debug, Args)]
pub struct TriageArgs {
    #[arg(
        long,
        default_value_t = 3,
        value_name = "N",
        help = "Minimum similar open cuts per cluster"
    )]
    pub min_count: usize,
}

#[derive(Debug, Args)]
pub struct VerifyArgs {}

#[derive(Debug, Args)]
pub struct RetrospectArgs {}

#[derive(Debug, Args)]
pub struct DigestArgs {
    #[arg(
        long,
        default_value = "7d",
        help = "Report since an RFC3339 timestamp or Nd/Nh duration"
    )]
    pub since: String,
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Json,
        help = "Output format"
    )]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct ArchiveArgs {
    #[arg(
        long,
        required = true,
        value_name = "VALUE",
        help = "Archive closed groups before an RFC3339 timestamp or Nd/Nh duration"
    )]
    pub before: String,
    #[arg(long, help = "Plan archive retention without writing")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, help = "Repair safe doctor findings")]
    pub fix: bool,
    #[arg(long, requires = "fix", help = "Plan doctor repairs without writing")]
    pub dry_run: bool,
    #[arg(
        long,
        conflicts_with = "fix",
        help = "Scan physical lines for home-path leaks (decoded on a parsing line, raw otherwise)"
    )]
    pub leaks: bool,
    #[arg(
        long,
        value_name = "LITERAL",
        requires = "leaks",
        help = "Flag a literal raw-line leak; repeatable; requires --leaks"
    )]
    pub deny: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SweepArgs {
    #[arg(
        value_name = "PATH",
        help = "Repository directory or direct JSONL log file; repeatable"
    )]
    pub paths: Vec<PathBuf>,
    #[arg(
        long,
        value_name = "FILE",
        help = "User-owned file with one path per line; blank lines and # comments ignored"
    )]
    pub registry: Option<PathBuf>,
    #[arg(long, help = "Filter since an RFC3339 timestamp or Nd/Nh duration")]
    pub since: Option<String>,
    #[arg(
        long,
        value_enum,
        default_value_t = ListKind::Cut,
        help = "Record kind to include in items"
    )]
    pub kind: ListKind,
}

#[derive(Debug, Args)]
pub struct ResolveArgs {
    #[arg(
        value_name = "ID",
        num_args = 1..,
        required = true,
        help = "One or more IDs or unique prefixes"
    )]
    pub ids: Vec<String>,
    #[arg(
        long,
        allow_hyphen_values = true,
        help = "Resolution note; leading hyphens accepted"
    )]
    pub note: Option<String>,
    #[arg(long, help = "Resolving agent; overrides BLOTTER_AGENT")]
    pub agent: Option<String>,
    #[arg(long, value_name = "ID", help = "Graduation task ID")]
    pub task: Option<String>,
    #[arg(long, value_name = "URL", help = "Graduation pull request URL")]
    pub pr: Option<String>,
    #[arg(long, value_name = "SHA", help = "Graduation commit SHA")]
    pub commit: Option<String>,
    #[arg(
        long,
        value_name = "URL",
        conflicts_with = "dropped",
        help = "Published destination (dogear records only)"
    )]
    pub url: Option<String>,
    #[arg(long, help = "Mark dropped (dogear records only)")]
    pub dropped: bool,
    #[arg(long, help = "Append a correction to an existing resolved record")]
    pub amend: bool,
    #[arg(long, help = "Validate without appending a resolution")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StatusFilter {
    Open,
    Resolved,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListKind {
    Cut,
    Dogear,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Md,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    OtlpJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaTarget {
    All,
    Record,
    Error,
    ExitCodes,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn assert_all_arguments_have_help(command: &clap::Command) {
        for argument in command.get_arguments() {
            assert!(
                argument.get_help().is_some() || argument.get_long_help().is_some(),
                "{} argument {:?} is missing help text",
                command.get_name(),
                argument.get_id()
            );
        }
        for subcommand in command.get_subcommands() {
            assert_all_arguments_have_help(subcommand);
        }
    }

    #[test]
    fn parser_covers_defaults_aliases_and_globals() {
        let cli =
            Cli::try_parse_from(["blotter", "--file", "x", "log", "ouch", "--pretty"]).unwrap();
        assert!(cli.pretty);
        assert_eq!(cli.file, Some(PathBuf::from("x")));
        let Command::Add(args) = cli.command else {
            panic!("expected add")
        };
        assert_eq!(args.text.as_deref(), Some("ouch"));
        assert_eq!(args.severity, Severity::Minor);

        let cli = Cli::try_parse_from(["blotter", "list"]).unwrap();
        let Command::List(args) = cli.command else {
            panic!("expected list")
        };
        assert_eq!(args.kind, ListKind::Cut);
        assert_eq!(args.status, StatusFilter::Open);
        assert_eq!(args.limit, 50);
        assert_eq!(args.format, OutputFormat::Json);

        let cli = Cli::try_parse_from(["blotter", "export", "--format", "otlp-json"]).unwrap();
        let Command::Export(args) = cli.command else {
            panic!("expected export")
        };
        assert_eq!(args.format, Some(ExportFormat::OtlpJson));

        let cli = Cli::try_parse_from(["blotter", "triage"]).unwrap();
        let Command::Triage(args) = cli.command else {
            panic!("expected triage")
        };
        assert_eq!(args.min_count, 3);

        let cli = Cli::try_parse_from(["blotter", "verify"]).unwrap();
        assert!(matches!(cli.command, Command::Verify(_)));

        let cli = Cli::try_parse_from(["blotter", "retrospect"]).unwrap();
        assert!(matches!(cli.command, Command::Retrospect(_)));

        let cli = Cli::try_parse_from(["blotter", "digest"]).unwrap();
        let Command::Digest(args) = cli.command else {
            panic!("expected digest")
        };
        assert_eq!(args.since, "7d");
        assert_eq!(args.format, OutputFormat::Json);

        let cli = Cli::try_parse_from([
            "blotter",
            "sweep",
            "repo",
            "--registry",
            "repos.txt",
            "--since",
            "1d",
            "--kind",
            "all",
        ])
        .unwrap();
        let Command::Sweep(args) = cli.command else {
            panic!("expected sweep")
        };
        assert_eq!(args.paths, [PathBuf::from("repo")]);
        assert_eq!(args.registry, Some(PathBuf::from("repos.txt")));
        assert_eq!(args.since.as_deref(), Some("1d"));
        assert_eq!(args.kind, ListKind::All);
    }

    #[test]
    fn parser_rejects_bad_values_and_missing_required_id() {
        assert!(Cli::try_parse_from(["blotter", "list", "--format", "jsonl"]).is_err());
        assert!(Cli::try_parse_from(["blotter", "digest", "--format", "jsonl"]).is_err());
        assert!(Cli::try_parse_from(["blotter", "sweep", "--kind", "other"]).is_err());
        assert!(Cli::try_parse_from(["blotter", "add", "x", "--severity", "critical"]).is_err());
        assert!(Cli::try_parse_from(["blotter", "resolve"]).is_err());
        assert!(Cli::try_parse_from(["blotter"]).is_err());
        for args in [
            vec!["blotter", "list", "--include-auto"],
            vec![
                "blotter",
                "export",
                "--format",
                "otlp-json",
                "--include-auto",
            ],
            vec!["blotter", "triage", "--include-auto"],
            vec!["blotter", "verify", "--include-auto"],
            vec!["blotter", "digest", "--include-auto"],
            vec!["blotter", "sweep", "repo", "--include-auto"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn parser_accepts_every_command_and_stdin_marker() {
        for args in [
            vec!["blotter", "add", "-"],
            vec!["blotter", "idea", "-"],
            vec!["blotter", "list", "--status", "all"],
            vec!["blotter", "list", "--kind", "dogear"],
            vec!["blotter", "export", "--format", "otlp-json"],
            vec!["blotter", "triage", "--min-count", "2"],
            vec!["blotter", "verify"],
            vec!["blotter", "digest"],
            vec!["blotter", "sweep", "repo"],
            vec!["blotter", "resolve", "abcd"],
            vec!["blotter", "archive", "--before", "1d"],
            vec!["blotter", "schema", "record"],
            vec!["blotter", "doctor"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
    }

    #[test]
    fn parser_accepts_leading_hyphen_text_values_without_swallowing_following_options() {
        let cli = Cli::try_parse_from([
            "blotter",
            "add",
            "text",
            "--cmd",
            "-tool arg",
            "--evidence",
            "--detail note",
            "--agent",
            "tester",
        ])
        .unwrap();
        let Command::Add(args) = cli.command else {
            panic!("expected add")
        };
        assert_eq!(args.cmd.as_deref(), Some("-tool arg"));
        assert_eq!(args.evidence.as_deref(), Some("--detail note"));
        assert_eq!(args.agent.as_deref(), Some("tester"));

        let cli = Cli::try_parse_from([
            "blotter",
            "resolve",
            "abcd1234",
            "--note",
            "--retry after timeout",
            "--agent",
            "fixer",
        ])
        .unwrap();
        let Command::Resolve(args) = cli.command else {
            panic!("expected resolve")
        };
        assert_eq!(args.note.as_deref(), Some("--retry after timeout"));
        assert_eq!(args.agent.as_deref(), Some("fixer"));
    }

    #[test]
    fn every_argument_has_help_text() {
        let mut command = Cli::command();
        command.build();
        assert_all_arguments_have_help(&command);
    }
}
