//! `.typoena.snippets.json`: snippet parsing and tab-stop extraction.


/// The git-tracked snippet library, read at boot like [`PREFS_PATH`]. The host
/// reads this file and hands the parsed list to [`Editor::set_snippets`]; a
/// missing or malformed file is non-fatal (no snippets, editor runs). It lives in
/// the Tracked repo so the library syncs across devices. Full format reference:
/// `docs/typoena-snippets.md`.
pub const SNIPPETS_PATH: &str = "/sd/repo/.typoena.snippets.json";

/// One snippet: an inline trigger [`prefix`](Snippet::prefix), a
/// [`body`](Snippet::body) carrying `$1..$n`/`$0` tab stops, and a
/// [`name`](Snippet::name)/[`description`](Snippet::description) the `$` palette
/// shows. Parsed from the Zed-compatible JSON by [`Snippets::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// Display name — the top-level JSON key. What the `$` palette lists.
    pub name: String,
    /// The word that triggers inline Tab-expansion.
    pub prefix: String,
    /// Literal body text with `$1..$n`/`$0` stops. `${n:label}` placeholders are
    /// stripped to bare `$n` at parse time (no completion popup to show a label,
    /// no overtype model to fill it) — see [`strip_stop_labels`].
    pub body: String,
    /// Human description; the `$` palette fuzzy-matches and shows it. Empty if the
    /// JSON entry omits it.
    pub description: String,
}

/// A parsed snippet library. [`parse`](Snippets::parse) reads the Zed JSON shape;
/// [`Editor::set_snippets`] installs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snippets(pub Vec<Snippet>);

/// The Zed JSON value for one snippet: `body` is a string or an array of lines.
#[derive(serde::Deserialize)]
pub(crate) struct RawSnippet {
    prefix: String,
    body: RawBody,
    #[serde(default)]
    description: String,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum RawBody {
    /// `"body": ["line", "line"]` — Zed's multi-line form (joined with `\n`).
    Lines(Vec<String>),
    /// `"body": "one line"`.
    Text(String),
}

impl Snippets {
    /// Parse the Zed-compatible `.typoena.snippets.json`: an object keyed by
    /// display name, each value `{ prefix, body, description? }` where `body` is a
    /// string or an array of lines. Labels in `${n:label}` stops are stripped to
    /// `$n`. Entries come back sorted by name (a `BTreeMap` parse — deterministic,
    /// and the empty-`$` palette order; a query re-ranks by fuzzy score anyway). An
    /// empty (or whitespace-only) file means "no snippets", same as a missing one —
    /// only a malformed file is an `Err` the host logs before booting with none.
    pub fn parse(src: &str) -> Result<Self, serde_json::Error> {
        if src.trim().is_empty() {
            return Ok(Self::default());
        }
        let raw: std::collections::BTreeMap<String, RawSnippet> = serde_json::from_str(src)?;
        let snippets = raw
            .into_iter()
            .map(|(name, r)| {
                let body = match r.body {
                    RawBody::Text(s) => s,
                    RawBody::Lines(v) => v.join("\n"),
                };
                Snippet {
                    name,
                    prefix: r.prefix,
                    body: strip_stop_labels(&body),
                    description: r.description,
                }
            })
            .collect();
        Ok(Self(snippets))
    }
}

/// Rewrite `${n:label}` (and `${n}`) tab stops to a bare `$n`, leaving plain
/// `$n`/`$0` and every other `$` untouched. The editor has no completion popup to
/// surface a label and no selection/overtype model to fill one, so the label is
/// only noise to delete — dropping it is what lets a Zed snippet file with
/// `${1:Titre}` load unchanged. Byte-indexed but UTF-8-safe: it only ever indexes
/// the ASCII `$ { } :` and digits; any multi-byte char is copied whole.
pub(crate) fn strip_stop_labels(body: &str) -> String {
    let b = body.as_bytes();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if b[i] == b'$' && i + 1 < body.len() && b[i + 1] == b'{' {
            // Read the digits right after "${".
            let mut j = i + 2;
            while j < body.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            // A numbered placeholder `${<digits>…}` → `$<digits>`, dropping the rest.
            if j > i + 2 {
                if let Some(close) = body[j..].find('}') {
                    out.push('$');
                    out.push_str(&body[i + 2..j]);
                    i = j + close + 1;
                    continue;
                }
            }
            // Not a numbered placeholder (or unclosed) — emit the `$` literally.
            out.push('$');
            i += 1;
            continue;
        }
        let c = body[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Split a snippet body into its literal text (tab-stop markers removed) and the
/// caret **visit order** of those stops as byte offsets into that literal: `$1 …
/// $n` ascending, then `$0` last. If the body has numbered stops but no explicit
/// `$0`, a final stop at the body end is appended (so the last Tab lands past the
/// text). A body with no stops returns an empty stop list (the caret just lands at
/// the end — no session). `$` not followed by a digit is literal.
pub(crate) fn parse_snippet_body(body: &str) -> (String, Vec<usize>) {
    let b = body.as_bytes();
    let mut literal = String::with_capacity(body.len());
    let mut stops: Vec<(u32, usize)> = Vec::new(); // (stop number, offset in `literal`)
    let mut i = 0;
    while i < body.len() {
        if b[i] == b'$' {
            let mut j = i + 1;
            while j < body.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                let num: u32 = body[i + 1..j].parse().unwrap_or(0);
                stops.push((num, literal.len()));
                i = j;
                continue;
            }
            literal.push('$'); // a lone `$` (e.g. a price) is literal text
            i += 1;
            continue;
        }
        let c = body[i..].chars().next().unwrap();
        literal.push(c);
        i += c.len_utf8();
    }
    // Ensure a final resting stop: `$0` if present, else an implicit one at the end.
    if !stops.is_empty() && !stops.iter().any(|&(n, _)| n == 0) {
        stops.push((0, literal.len()));
    }
    // Visit order: $1..$n ascending, $0 (the final rest) last.
    stops.sort_by_key(|&(n, _)| if n == 0 { u32::MAX } else { n });
    (literal, stops.into_iter().map(|(_, off)| off).collect())
}
