//! Markdown formatting: `:fmt` reflow, list markers, and table alignment.

use super::*;

/// The text of the first ATX heading in `text` (`# Title` → `Title`), any
/// level, or `None`. The `add local link` command uses it as the link title —
/// the `# <title>` line title-typed files are seeded with.
pub(crate) fn first_heading(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let hashes = line.len() - line.trim_start_matches('#').len();
        if hashes == 0 {
            return None;
        }
        let title = substr(line, hashes..).strip_prefix(' ')?.trim();
        (!title.is_empty()).then_some(title)
    })
}

/// Parse an auto-continued Markdown line prefix at the start of `line` — a list
/// item (`- `/`* `/`+ ` or `N. `) or a blockquote (`> `, possibly nested). Returns
/// `(next_marker, current_marker_len, content_empty)` where `next_marker` is what
/// the following line should start with (same bullet, the incremented number, or
/// the same quote depth, preserving indentation), `current_marker_len` is the byte
/// length of this line's marker prefix, and `content_empty` is whether anything
/// follows it. Returns `None` when the line has no such prefix. ASCII throughout
/// (leading spaces, bullets, digits, `. `, `> ` are all single-byte).
pub(crate) fn continuation_marker(line: &str) -> Option<(String, usize, bool)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = substr(line, indent..);
    for bullet in ["- ", "* ", "+ "] {
        if rest.starts_with(bullet) {
            let cur_len = indent + bullet.len();
            let content_empty = substr(line, cur_len..).trim().is_empty();
            return Some((format!("{}{bullet}", substr(line, ..indent)), cur_len, content_empty));
        }
    }
    // Ordered: <digits>`. ` → continue as the next number.
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && substr(rest, digits..).starts_with(". ") {
        let cur_len = indent + digits + 2;
        let content_empty = substr(line, cur_len..).trim().is_empty();
        let n: usize = substr(rest, ..digits).parse().unwrap_or(0);
        return Some((format!("{}{}. ", substr(line, ..indent), n + 1), cur_len, content_empty));
    }
    // Blockquote: a run of `>` markers, each with an optional trailing space,
    // continued at the same depth (`> > text` → `> > `). A bare `>` normalizes to
    // `> `. A nested list inside the quote isn't preserved — it degrades to `> `.
    if rest.starts_with('>') {
        let bytes = rest.as_bytes();
        let mut j = 0;
        let mut depth = 0;
        while bytes.get(j) == Some(&b'>') {
            depth += 1;
            j += 1;
            if bytes.get(j) == Some(&b' ') {
                j += 1;
            }
        }
        let cur_len = indent + j;
        let content_empty = substr(line, cur_len..).trim().is_empty();
        let next = format!("{}{}", substr(line, ..indent), "> ".repeat(depth));
        return Some((next, cur_len, content_empty));
    }
    None
}

/// Every `[title](target)` markdown link on `line`, left to right, as
/// `(open, target_start, close)` — byte offsets of the opening `[`, the first
/// target byte, and the closing `)`. The first `]` closes the title (nested
/// brackets aren't supported) and must be immediately followed by `(`.
/// Byte-indexed and ASCII-delimited throughout, so multibyte titles/targets
/// pass through untouched.
fn link_spans(line: &str) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    let bytes = line.as_bytes();
    let mut i = 0;
    core::iter::from_fn(move || {
        while i < bytes.len() {
            if bytes.get(i) != Some(&b'[') {
                i += 1;
                continue;
            }
            let Some(title_end) = (i + 1..bytes.len()).find(|&j| bytes.get(j) == Some(&b']'))
            else {
                return None; // no `]` left — no further link can start either
            };
            if bytes.get(title_end + 1) != Some(&b'(') {
                i += 1;
                continue;
            }
            let close = (title_end + 2..bytes.len()).find(|&k| bytes.get(k) == Some(&b')'))?;
            let open = i;
            i = close + 1;
            return Some((open, title_end + 2, close));
        }
        None
    })
}

/// The target of the `[title](target)` markdown link whose span contains byte
/// `col` of `line` — the caret can sit anywhere from the opening `[` through
/// the closing `)`, inclusive. Returns the raw text between the parens
/// (`<>` unwrapping and `#fragment` stripping are the follower's job). `None`
/// when no link spans `col`.
pub(crate) fn link_target_at(line: &str, col: usize) -> Option<&str> {
    link_spans(line)
        .find(|&(open, _, close)| (open..=close).contains(&col))
        .map(|(_, ts, close)| substr(line, ts..close))
}

/// `:pub` — retarget every markdown link in `text` (the contents of
/// `own_path`) that points at `from` (the absolute pre-publish card path) so
/// it follows the file to its `.pub.md` name. Matching mirrors `gf` exactly —
/// same parse ([`link_spans`]), `<>` unwrap, `#fragment` strip, and directory
/// resolution ([`resolve_link_target`]) — so a link is rewritten iff `gf` on
/// it would have opened the old file. Publishing only changes the name's
/// tail, so each hit keeps the writer's own spelling (`notes.md`,
/// `./notes.md`, `../repo/notes.md`) and just grows a `.pub`: the rewrite is
/// a 4-byte insertion before the target's final `.md`. Returns the new text
/// plus each insertion offset **in the old text** (callers shift carets past
/// them), or `None` when nothing links to `from`. Pure: the host runs it over
/// on-disk files, the editor over resident buffers.
pub fn publish_retarget_links(
    own_path: &str,
    text: &str,
    from: &str,
) -> Option<(String, Vec<usize>)> {
    // A file's scope is its directory; it only disambiguates label/absolute
    // target forms (`repo/…`, `/sd/…`), which name their own scope anyway.
    let scope = if own_path.strip_prefix(LOCAL_DIR).is_some_and(|r| r.starts_with('/')) {
        Scope::Local
    } else {
        Scope::Tracked
    };
    let mut sites: Vec<usize> = Vec::new();
    let mut line_start = 0;
    for line in text.split('\n') {
        for (_, ts, close) in link_spans(line) {
            if let Some(site) = retarget_site(own_path, scope, line, ts, close, from) {
                sites.push(line_start + site);
            }
        }
        line_start += line.len() + 1;
    }
    if sites.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 4 * sites.len());
    let mut prev = 0;
    for &s in &sites {
        out.push_str(substr(text, prev..s));
        out.push_str(".pub");
        prev = s;
    }
    out.push_str(substr(text, prev..));
    Some((out, sites))
}

/// One link's `.pub` insertion offset within `line` — the start of the
/// target's final `.md` — or `None` when the `[title](target)` at `ts..close`
/// doesn't resolve to `from`. The normalization (trim, `<>` unwrap, external
/// skip, `#fragment` strip) mirrors
/// [`follow_link_at_caret`](Editor::follow_link_at_caret), but tracks byte
/// offsets so the splice lands inside the original spelling.
fn retarget_site(
    own_path: &str,
    scope: Scope,
    line: &str,
    ts: usize,
    close: usize,
    from: &str,
) -> Option<usize> {
    let raw = substr(line, ts..close);
    let mut start = ts + (raw.len() - raw.trim_start().len());
    let mut end = start + raw.trim().len();
    let t = substr(line, start..end);
    if t.len() >= 2 && t.starts_with('<') && t.ends_with('>') {
        let inner = substr(line, start + 1..end - 1);
        start = start + 1 + (inner.len() - inner.trim_start().len());
        end = start + inner.trim().len();
    }
    let target = substr(line, start..end);
    if target.contains("://") || target.starts_with("mailto:") {
        return None;
    }
    let path = substr(target, ..target.find('#').unwrap_or(target.len()));
    // Requiring the spelled path to end in `.md` also rejects `.`/`..`-tailed
    // spellings whose *resolved* form ends in `.md` — there is no `.md` tail
    // to splice into.
    if path.is_empty() || !path.ends_with(".md") {
        return None;
    }
    let (abs, _) = resolve_link_target(own_path, scope, path)?;
    (abs == from).then(|| start + path.len() - 3)
}

/// Column alignment parsed from a table's `|:--:|` separator row.
#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Right,
    Center,
    None,
}

/// Normalize a Markdown buffer for `:fmt`: strip trailing whitespace, align
/// pipe tables, collapse runs of blank lines to a single blank, and terminate
/// the buffer with a newline. Deliberately does NOT reflow paragraphs — the
/// buffer's logical line breaks are the writer's, and display wrapping is soft
/// (see `layout`). ASCII throughout (widths are char counts).
pub(crate) fn format_markdown(text: &str) -> String {
    // 1. Trailing-whitespace strip, per line.
    let stripped: Vec<String> = text.split('\n').map(|l| l.trim_end().to_string()).collect();

    // 2. Reformat pipe-table blocks in place; pass everything else through.
    let mut piped: Vec<String> = Vec::with_capacity(stripped.len());
    let mut i = 0;
    while let Some(tail) = stripped.get(i..).filter(|t| !t.is_empty()) {
        if let Some(len) = table_block_len(tail) {
            piped.extend(format_table(tail.get(..len).unwrap_or_default()));
            i += len.max(1);
        } else {
            piped.extend(tail.first().cloned());
            i += 1;
        }
    }

    // 3. Collapse 2+ consecutive blank lines to one. A trailing blank run
    //    collapses the same way, so at most one trailing blank line survives — and
    //    we deliberately keep that one rather than dropping it. A writer often
    //    presses Enter to open the next line before pausing; yanking that line
    //    (and the caret) out from under them on every format-on-save is jarring.
    let mut out: Vec<String> = Vec::with_capacity(piped.len());
    let mut blank_run = 0;
    for line in piped {
        if line.is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                out.push(String::new());
            }
        } else {
            blank_run = 0;
            out.push(line);
        }
    }
    let mut text = out.join("\n");
    // 4. POSIX terminator, in the buffer rather than only on disk. `save_path`
    //    already guards the file byte, but a buffer that ended mid-line stayed a
    //    byte out of step with its file — the writer saw no empty last row and
    //    the terminator only appeared on the next reload. An empty buffer stays
    //    empty: a blank scratch has no line to terminate.
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Split a table row into trimmed cells, dropping the empty cells that leading /
/// trailing `|` produce (`| a | b |` → `["a", "b"]`).
pub(crate) fn table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// A separator row: every cell is dashes with optional edge colons (`:--`, `-:`,
/// `:-:`, `---`) and at least one dash.
pub(crate) fn is_separator_row(line: &str) -> bool {
    if !line.contains('|') {
        return false;
    }
    let cells = table_cells(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            !c.is_empty() && c.contains('-') && c.chars().all(|ch| ch == '-' || ch == ':')
        })
}

/// If `lines[0..]` starts a pipe table (header row + separator row + data rows),
/// return its length in lines; else `None`.
pub(crate) fn table_block_len(lines: &[String]) -> Option<usize> {
    let (Some(header), Some(sep)) = (lines.first(), lines.get(1)) else {
        return None;
    };
    if !header.contains('|') || !is_separator_row(sep) {
        return None;
    }
    let mut n = 2;
    while lines.get(n).is_some_and(|l| !l.is_empty() && l.contains('|')) {
        n += 1;
    }
    Some(n)
}

/// Reformat one detected table block: pad every cell to its column's width and
/// rebuild the separator row, honoring per-column alignment colons.
pub(crate) fn format_table(block: &[String]) -> Vec<String> {
    let rows: Vec<Vec<String>> = block.iter().map(|l| table_cells(l)).collect();
    let aligns: Vec<Align> = rows
        .get(1)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|c| match (c.starts_with(':'), c.ends_with(':')) {
            (true, true) => Align::Center,
            (true, false) => Align::Left,
            (false, true) => Align::Right,
            (false, false) => Align::None,
        })
        .collect();
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(aligns.len());

    // Column widths from content rows (min 3 so the separator stays readable).
    let mut width = vec![3usize; ncols];
    for (ri, row) in rows.iter().enumerate() {
        if ri == 1 {
            continue; // the separator's own width doesn't constrain the column
        }
        for (ci, cell) in row.iter().enumerate() {
            if let Some(w) = width.get_mut(ci) {
                *w = (*w).max(cell.chars().count());
            }
        }
    }
    let align_of = |ci: usize| aligns.get(ci).copied().unwrap_or(Align::None);

    rows.iter()
        .enumerate()
        .map(|(ri, row)| {
            let cells: Vec<String> = (0..ncols)
                .map(|ci| {
                    let w = width.get(ci).copied().unwrap_or(3);
                    if ri == 1 {
                        match align_of(ci) {
                            Align::Left => format!(":{}", "-".repeat(w - 1)),
                            Align::Right => format!("{}:", "-".repeat(w - 1)),
                            Align::Center => format!(":{}:", "-".repeat(w - 2)),
                            Align::None => "-".repeat(w),
                        }
                    } else {
                        pad_cell(row.get(ci).map(String::as_str).unwrap_or(""), w, align_of(ci))
                    }
                })
                .collect();
            format!("| {} |", cells.join(" | "))
        })
        .collect()
}

/// Pad `cell` to `w` columns per `align` (left/none pad right, right pads left,
/// center splits). Over-wide cells are returned unchanged.
pub(crate) fn pad_cell(cell: &str, w: usize, align: Align) -> String {
    let len = cell.chars().count();
    if len >= w {
        return cell.to_string();
    }
    let pad = w - len;
    match align {
        Align::Right => format!("{}{cell}", " ".repeat(pad)),
        Align::Center => {
            let l = pad / 2;
            format!("{}{cell}{}", " ".repeat(l), " ".repeat(pad - l))
        }
        _ => format!("{cell}{}", " ".repeat(pad)),
    }
}


impl Editor {
    /// The lint pass's caret-safe subset, for a Cmd+S that lands mid-Insert:
    /// terminate the buffer without reflowing the line under the caret. The
    /// append is at the very end, past every caret offset, so the caret does not
    /// move — including the common case of a caret sitting at the old end.
    ///
    /// Deliberately no [`checkpoint`](Self::checkpoint): the Insert session is
    /// one undo group (taken on entering Insert), and splitting it on a save
    /// would make `u` mid-typing undo the terminator instead of the sentence.
    pub(crate) fn terminate_buffer(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
    }

    /// `:fmt` — normalize the buffer (align tables, collapse duplicate blank
    /// lines, strip trailing whitespace, terminate with a newline) and keep the
    /// caret on roughly the same line (buffer length changes, so exact
    /// restoration isn't possible).
    pub(crate) fn format_buffer(&mut self) {
        self.checkpoint(); // `:fmt` (and format-on-save) is undoable
        let row = substr(&self.text, ..self.caret).bytes().filter(|&b| b == b'\n').count();
        let col = self.caret - self.line_start(self.caret); // byte offset within the line
        self.text = format_markdown(&self.text);
        // Land the caret on the same logical line, at the same column when it
        // still fits. Formatting keeps the writer's line breaks (no paragraph
        // reflow), so for ordinary prose the caret lands exactly where it was; a
        // line that was rewritten (table padding, list-marker normalization)
        // clamps to its new end.
        let total = self.text.bytes().filter(|&b| b == b'\n').count() + 1;
        let target = row.min(total - 1);
        let line_start = if target == 0 {
            0
        } else {
            // Byte after the `target`-th newline; end of buffer if there are
            // fewer newlines than that.
            self.text
                .bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .nth(target - 1)
                .map(|(i, _)| i + 1)
                .unwrap_or(self.text.len())
        };
        let mut caret = (line_start + col).min(self.line_end(line_start));
        // A rewritten line's byte layout can shift; snap back to a char boundary.
        while caret > line_start && !self.text.is_char_boundary(caret) {
            caret -= 1;
        }
        self.caret = caret;
    }

}
