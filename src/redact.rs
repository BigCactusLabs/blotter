//! Home-path rewriting shared by the evidence redactor and the stored cwd.
//! `commands::add` layers secret redaction on top; `store::record_cwd` uses the
//! path scanner alone.

use std::path::Path;

// Keep this token-boundary class and `home_path_delimiter` below mirrored in
// `commands::doctor` for raw leak scans.
// A slash is a path parent, not a delimiter.
const EVIDENCE_DELIMITERS: &str = ",;)]}&#\"'";
// Home-path prefixes, in slash form and in the dash-encoded form that harness
// scratchpad and session slugs embed, such as `-Users-<name>-<repo>`.
const HOME_PREFIXES: [&str; 4] = ["/Users/", "/home/", "-Users-", "-home-"];

pub(crate) fn evidence_delimiter(character: char) -> bool {
    character.is_ascii_whitespace() || EVIDENCE_DELIMITERS.contains(character)
}

// A colon separates entries in Unix path lists such as PATH. Keep it specific
// to home-path scanning: `evidence_delimiter` is also used by the secret-value
// redactor, where treating every colon as a token boundary would change URL and
// assignment parsing.
fn home_path_delimiter(character: char) -> bool {
    evidence_delimiter(character) || character == ':'
}

fn path_prefix_boundary(input: &str, end: usize, separator: char) -> bool {
    input[end..].chars().next().is_none_or(|character| {
        character == '/' || character == separator || home_path_delimiter(character)
    })
}

fn dash_start_boundary(input: &str, start: usize) -> bool {
    start == 0
        || input[..start]
            .chars()
            .next_back()
            .is_some_and(|character| home_path_delimiter(character) || character == '/')
}

fn generic_home_prefix_end(input: &str, start: usize) -> Option<usize> {
    let prefix = HOME_PREFIXES
        .into_iter()
        .find(|prefix| input[start..].starts_with(prefix))?;
    let separator = prefix.chars().next().expect("prefixes are non-empty");
    // Generic aliases only start a token. Unlike exact $HOME matching, a
    // preceding slash makes the slash form a nested path such as
    // /tmp/Users/alice; a dash-encoded slug normally does follow a slash.
    if start != 0
        && !input[..start].chars().next_back().is_some_and(|character| {
            home_path_delimiter(character) || (separator == '-' && character == '/')
        })
    {
        return None;
    }
    let component_start = start + prefix.len();
    let component_end = input[component_start..]
        .char_indices()
        .find_map(|(offset, character)| {
            (character == '/' || character == separator || home_path_delimiter(character))
                .then_some(component_start + offset)
        })
        .unwrap_or(input.len());
    (component_end > component_start && path_prefix_boundary(input, component_end, separator))
        .then_some(component_end)
}

pub(crate) fn rewrite_home_paths(input: &str, home: Option<&Path>) -> String {
    let home = home.and_then(Path::to_str);
    // Exact current home in dash-encoded form. This must win over the generic
    // dash rule: a dash inside the username would otherwise truncate the
    // rewrite after its first dash-separated component.
    let dash_home = home.map(|home| home.replace('/', "-"));
    let mut output = String::with_capacity(input.len());
    let mut copied = 0;
    let mut index = 0;
    while index < input.len() {
        let character = input[index..]
            .chars()
            .next()
            .expect("index stays on a character boundary");
        if character != '/' && character != '-' {
            index += character.len_utf8();
            continue;
        }
        let end = home
            .filter(|home| input[index..].starts_with(home))
            .map(|home| index + home.len())
            .filter(|end| path_prefix_boundary(input, *end, '/'))
            .or_else(|| {
                dash_home
                    .as_deref()
                    .filter(|_| dash_start_boundary(input, index))
                    .filter(|dash| input[index..].starts_with(dash))
                    .map(|dash| index + dash.len())
                    .filter(|end| path_prefix_boundary(input, *end, '-'))
            })
            .or_else(|| generic_home_prefix_end(input, index));
        if let Some(end) = end {
            output.push_str(&input[copied..index]);
            output.push('~');
            copied = end;
            // A match replaces only the home prefix, but scanning resumes right
            // here rather than at the token's end: `doctor --leaks` scans every
            // position, so a home form nested in the tail (an exact home, or a
            // dash-encoded slug after a `/`) must redact or blotter's own write
            // trips its own gate (r38).
            index = end;
        } else {
            index += character.len_utf8();
        }
    }
    if copied == 0 {
        input.into()
    } else {
        output.push_str(&input[copied..]);
        output
    }
}
