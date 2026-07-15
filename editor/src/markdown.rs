//! Markdown formatting: `:fmt` reflow, list markers, and table alignment.

use super::*;

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
    let rest = &line[indent..];
    for bullet in ["- ", "* ", "+ "] {
        if rest.starts_with(bullet) {
            let cur_len = indent + bullet.len();
            let content_empty = line[cur_len..].trim().is_empty();
            return Some((format!("{}{bullet}", &line[..indent]), cur_len, content_empty));
        }
    }
    // Ordered: <digits>`. ` → continue as the next number.
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && rest[digits..].starts_with(". ") {
        let cur_len = indent + digits + 2;
        let content_empty = line[cur_len..].trim().is_empty();
        let n: usize = rest[..digits].parse().unwrap_or(0);
        return Some((format!("{}{}. ", &line[..indent], n + 1), cur_len, content_empty));
    }
    // Blockquote: a run of `>` markers, each with an optional trailing space,
    // continued at the same depth (`> > text` → `> > `). A bare `>` normalizes to
    // `> `. A nested list inside the quote isn't preserved — it degrades to `> `.
    if rest.starts_with('>') {
        let bytes = rest.as_bytes();
        let mut j = 0;
        let mut depth = 0;
        while j < bytes.len() && bytes[j] == b'>' {
            depth += 1;
            j += 1;
            if j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
        }
        let cur_len = indent + j;
        let content_empty = line[cur_len..].trim().is_empty();
        let next = format!("{}{}", &line[..indent], "> ".repeat(depth));
        return Some((next, cur_len, content_empty));
    }
    None
}

// --- `:fmt` Markdown normalizer ----------------------------------------------

/// Column alignment parsed from a table's `|:--:|` separator row.
#[derive(Clone, Copy)]
pub(crate) enum Align {
    Left,
    Right,
    Center,
    None,
}

/// Normalize a Markdown buffer for `:fmt`: strip trailing whitespace, align
/// pipe tables, and collapse runs of blank lines to a single blank (dropping
/// trailing blanks). Deliberately does NOT reflow paragraphs — the buffer's
/// logical line breaks are the writer's, and display wrapping is soft (see
/// `layout`). ASCII throughout (widths are char counts).
pub(crate) fn format_markdown(text: &str) -> String {
    // 1. Trailing-whitespace strip, per line.
    let stripped: Vec<String> = text.split('\n').map(|l| l.trim_end().to_string()).collect();

    // 2. Reformat pipe-table blocks in place; pass everything else through.
    let mut piped: Vec<String> = Vec::with_capacity(stripped.len());
    let mut i = 0;
    while i < stripped.len() {
        if let Some(len) = table_block_len(&stripped[i..]) {
            piped.extend(format_table(&stripped[i..i + len]));
            i += len;
        } else {
            piped.push(stripped[i].clone());
            i += 1;
        }
    }

    // 3. Collapse 2+ consecutive blank lines to one. A trailing blank run
    //    collapses the same way, so at most one trailing blank line survives — and
    //    we deliberately keep that one rather than dropping it. A writer often
    //    presses Enter to open the next line before pausing; yanking that line
    //    (and the caret) out from under them on every format-on-save is jarring.
    //    The file's POSIX terminator is `save_path`'s job, not this pass's, so
    //    keeping the blank line here is purely about not disturbing the buffer.
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
    out.join("\n")
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
    if lines.len() < 2 || !lines[0].contains('|') || !is_separator_row(&lines[1]) {
        return None;
    }
    let mut n = 2;
    while n < lines.len() && !lines[n].is_empty() && lines[n].contains('|') {
        n += 1;
    }
    Some(n)
}

/// Reformat one detected table block: pad every cell to its column's width and
/// rebuild the separator row, honoring per-column alignment colons.
pub(crate) fn format_table(block: &[String]) -> Vec<String> {
    let rows: Vec<Vec<String>> = block.iter().map(|l| table_cells(l)).collect();
    let aligns: Vec<Align> = rows[1]
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
            width[ci] = width[ci].max(cell.chars().count());
        }
    }
    let align_of = |ci: usize| aligns.get(ci).copied().unwrap_or(Align::None);

    rows.iter()
        .enumerate()
        .map(|(ri, row)| {
            let cells: Vec<String> = (0..ncols)
                .map(|ci| {
                    let w = width[ci];
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
    /// `:fmt` — normalize the buffer (align tables, collapse duplicate blank
    /// lines, strip trailing whitespace) and keep the caret on roughly the same
    /// line (buffer length changes, so exact restoration isn't possible).
    pub(crate) fn format_buffer(&mut self) {
        self.checkpoint(); // `:fmt` (and format-on-save) is undoable
        let row = self.text[..self.caret].bytes().filter(|&b| b == b'\n').count();
        self.text = format_markdown(&self.text);
        // Land the caret at the start of the same logical line, clamped.
        let total = self.text.bytes().filter(|&b| b == b'\n').count() + 1;
        let target = row.min(total - 1);
        self.caret = if target == 0 {
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
    }

}
