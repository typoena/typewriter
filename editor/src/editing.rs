//! Text mutation: insert/delete/paste, registers, snippets expansion at the
//! caret, operators and text objects.

use super::*;

/// A pending operator awaiting a motion or text object (`d`elete / `c`hange /
/// `y`ank).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Delete,
    Change,
    Yank,
}


impl Editor {
    pub(crate) fn insert_char(&mut self, c: char) {
        self.text.insert(self.caret, c);
        self.caret += c.len_utf8();
    }

    pub(crate) fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.caret, s);
        self.caret += s.len();
    }

    // --- Snippets ----------------------------------------------------------

    /// Expand a snippet `body` at the caret and enter its tab-stop session. Splits
    /// the body into literal text (stops removed) and their [`parse_snippet_body`]
    /// visit order, inserts the literal, lands the caret on `$1` (or the body end
    /// if there are no stops), and enters Insert. The remaining stops queue in
    /// [`snippet_stops`](Self::snippet_stops) for Tab. Shared by inline
    /// Tab-expansion and the `$` palette. **The caller owns the undo boundary** —
    /// it must [`checkpoint`](Self::checkpoint) *before* any related mutation
    /// (the inline path deletes the trigger word first), so one undo group covers
    /// the whole expansion; `insert_snippet` deliberately does not checkpoint.
    pub(crate) fn insert_snippet(&mut self, body: &str) {
        let (literal, stops) = parse_snippet_body(body);
        let base = self.caret;
        self.text.insert_str(base, &literal);
        let mut abs = stops.into_iter().map(|o| base + o);
        match abs.next() {
            Some(first) => {
                self.caret = first;
                self.snippet_stops = abs.collect();
            }
            None => {
                self.caret = base + literal.len();
                self.snippet_stops.clear();
            }
        }
        self.mode = Mode::Insert;
    }

    /// Tab inside a live session: jump the caret to the next pending stop. The
    /// final stop (`$0` / the body end) empties the queue, ending the session with
    /// the caret resting there. Offsets were kept current by the edit-shift in
    /// [`insert_key`](Self::insert_key), so this is a plain move.
    pub(crate) fn snippet_advance(&mut self) {
        if self.snippet_stops.is_empty() {
            return;
        }
        let next = self.snippet_stops.remove(0);
        self.caret = next.min(self.text.len());
    }

    /// The maximal run of non-whitespace ending exactly at the caret — the
    /// candidate inline snippet trigger — or `None` if the caret is at a line/word
    /// start (the char before it is whitespace). Whitespace is ASCII, so scanning
    /// bytes is UTF-8-safe: `start` lands just past an ASCII space/newline (a char
    /// boundary) or at 0.
    pub(crate) fn word_before_caret(&self) -> Option<(usize, &str)> {
        let b = self.text.as_bytes();
        if self.caret == 0 || b[self.caret - 1].is_ascii_whitespace() {
            return None;
        }
        let mut start = self.caret;
        while start > 0 && !b[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        Some((start, &self.text[start..self.caret]))
    }

    /// Inline Tab-expansion: if the word immediately before the caret is exactly a
    /// snippet prefix, replace it with the expansion (as one undo group) and start
    /// the tab-stop session, returning `true`. Otherwise leave the buffer untouched
    /// and return `false`, so Tab falls back to inserting spaces.
    pub(crate) fn try_expand_snippet(&mut self) -> bool {
        let Some((start, word)) = self.word_before_caret() else {
            return false;
        };
        let Some(body) = self.snippets.iter().find(|s| s.prefix == word).map(|s| s.body.clone())
        else {
            return false;
        };
        self.checkpoint(); // baseline includes the trigger word — undo restores it
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
        self.insert_snippet(&body);
        true
    }

    /// Enter in Insert mode, with Markdown list and blockquote continuation. At the
    /// END of a list line (`- `/`* `/`+ ` or `N. `) or a blockquote (`> `, nested
    /// depth preserved), start the next line automatically — same bullet, the next
    /// number, or the same quote depth — preserving indentation. Enter on an
    /// otherwise-empty item/quote strips the marker instead (exits it). Anywhere
    /// else (mid-line, or a plain line) it's a plain newline.
    pub(crate) fn insert_newline(&mut self) {
        let le = self.line_end(self.caret);
        if self.caret == le {
            let ls = self.line_start(self.caret);
            if let Some((next, cur_len, content_empty)) = continuation_marker(&self.text[ls..le]) {
                if content_empty {
                    // Empty item/quote: drop the marker, leaving a blank line.
                    self.text.replace_range(ls..ls + cur_len, "");
                    self.caret = ls;
                } else {
                    self.insert_str(&format!("\n{next}"));
                }
                return;
            }
        }
        self.insert_char('\n');
    }

    pub(crate) fn backspace(&mut self) {
        if self.caret > 0 {
            self.caret = self.prev_char(self.caret);
            self.text.remove(self.caret); // removes the whole char at the caret
        }
    }

    /// `x` — delete the char under the caret (never a newline).
    pub(crate) fn delete_at_caret(&mut self) {
        let b = self.text.as_bytes();
        if self.caret < b.len() && b[self.caret] != b'\n' {
            self.text.remove(self.caret);
            // Keep the caret on a char: if it fell off the line end, step back.
            if self.caret >= self.line_end(self.caret) && self.caret > self.line_start(self.caret) {
                self.caret = self.prev_char(self.caret);
            }
        }
    }

    /// `dd` — delete the current logical line, including its newline (or the
    /// preceding one for the last line, so no blank line is left behind).
    pub(crate) fn delete_current_line(&mut self) {
        let ls = self.line_start(self.caret);
        let le = self.line_end(self.caret);
        let (start, end) = if le < self.text.len() {
            (ls, le + 1) // eat the trailing newline
        } else if ls > 0 {
            (ls - 1, le) // last line: eat the preceding newline instead
        } else {
            (ls, le) // whole buffer
        };
        self.text.replace_range(start..end, "");
        self.caret = self.line_start(start.min(self.text.len()));
    }

    /// `cc` — clear the current line's text and drop into insert.
    pub(crate) fn change_current_line(&mut self) {
        self.checkpoint();
        let ls = self.line_start(self.caret);
        let le = self.line_end(self.caret);
        self.register = format!("{}\n", &self.text[ls..le]); // linewise, like dd
        self.register_linewise = true;
        self.text.replace_range(ls..le, "");
        self.caret = ls;
        self.mode = Mode::Insert;
    }

    /// Yank `n` logical lines from the caret's line into the register, linewise
    /// (each line carries its trailing `\n`, synthesised for a final line that
    /// lacks one). Backs both `yy`/`nyy` and `dd`/`ndd`'s register capture; does
    /// not move the caret or change the buffer.
    pub(crate) fn register_lines(&mut self, n: usize) {
        let ls = self.line_start(self.caret);
        let mut e = ls;
        for _ in 0..n {
            let le = self.line_end(e);
            e = if le < self.text.len() { le + 1 } else { le };
        }
        let mut block = self.text[ls..e].to_string();
        if !block.ends_with('\n') {
            block.push('\n'); // the last line has no trailing newline; add one
        }
        self.register = block;
        self.register_linewise = true;
    }

    /// `p` — paste the register `n` times after the caret. Linewise content
    /// opens new line(s) below the current line (caret to the first pasted
    /// line); charwise content goes in just after the caret char (caret on the
    /// last pasted char). No-op on an empty register.
    pub(crate) fn paste_after(&mut self, n: usize) {
        if self.register.is_empty() {
            return;
        }
        self.checkpoint();
        let content = self.register.repeat(n);
        // `end`: byte offset of the last pasted char, so the viewport can reveal
        // the whole block even when the caret stays on its first line.
        let end = if self.register_linewise {
            let le = self.line_end(self.caret);
            if le < self.text.len() {
                let at = le + 1; // start of the following line
                self.text.insert_str(at, &content);
                self.caret = at;
                at + content.len() - 1
            } else {
                // Last line has no trailing newline: prefix one, drop the
                // block's trailing newline so we don't leave a blank line.
                let block = content.strip_suffix('\n').unwrap_or(&content);
                let inserted = format!("\n{block}");
                let end = le + inserted.len() - 1;
                self.text.insert_str(le, &inserted);
                self.caret = le + 1;
                end
            }
        } else {
            let at = if self.text.is_empty() { 0 } else { self.next_char(self.caret) };
            self.text.insert_str(at, &content);
            self.caret = self.prev_char(at + content.len()); // onto the last char
            self.caret
        };
        self.reveal(end);
    }

    /// `P` — paste the register `n` times before the caret. Linewise content
    /// opens new line(s) above the current line; charwise content goes in at the
    /// caret (caret on the last pasted char). No-op on an empty register.
    pub(crate) fn paste_before(&mut self, n: usize) {
        if self.register.is_empty() {
            return;
        }
        self.checkpoint();
        let content = self.register.repeat(n);
        let end = if self.register_linewise {
            let ls = self.line_start(self.caret);
            self.text.insert_str(ls, &content);
            self.caret = ls;
            ls + content.len() - 1
        } else {
            let at = self.caret;
            self.text.insert_str(at, &content);
            self.caret = self.prev_char(at + content.len()); // onto the last char
            self.caret
        };
        self.reveal(end);
    }

    /// Apply a pending operator over the buffer range `[start, end)` (order
    /// independent). All three fill the unnamed register (charwise) with the
    /// range. Yank copies and leaves the text; Delete removes it; Change removes
    /// it and enters insert. Yank leaves the caret at the range start (vim `yw`),
    /// the others land it there because the text collapses to that point.
    pub(crate) fn apply_op(&mut self, op: Op, start: usize, end: usize) {
        let s = start.min(end);
        let e = start.max(end).min(self.text.len());
        self.register = self.text[s..e].to_string();
        self.register_linewise = false;
        if op == Op::Yank {
            self.caret = s;
            return;
        }
        self.checkpoint(); // Delete/Change mutate — snapshot for undo
        self.text.replace_range(s..e, "");
        self.caret = s.min(self.text.len());
        if op == Op::Change {
            self.mode = Mode::Insert;
        }
    }

    /// Resolve a text object to a buffer range. `around` selects `a` (include
    /// delimiters / trailing space) vs `i` (inner). Returns `None` if there's
    /// no matching object under the caret.
    pub(crate) fn text_object(&self, obj: char, around: bool) -> Option<(usize, usize)> {
        match obj {
            'w' => Some(self.word_object(around)),
            '(' | ')' | 'b' => self.pair_object(b'(', b')', around),
            '{' | '}' | 'B' => self.pair_object(b'{', b'}', around),
            '[' | ']' => self.pair_object(b'[', b']', around),
            '<' | '>' => self.pair_object(b'<', b'>', around),
            '"' => self.quote_object(b'"', around),
            '\'' => self.quote_object(b'\'', around),
            '`' => self.quote_object(b'`', around),
            _ => None,
        }
    }

    /// `iw`/`aw`: the run of same-class chars (word vs space, never crossing a
    /// newline) under the caret. `aw` also takes the trailing run of spaces, or
    /// the leading one if there is no trailing space. Word class is
    /// whitespace-delimited (so this behaves like vim's `iW`/`aW`).
    pub(crate) fn word_object(&self, around: bool) -> (usize, usize) {
        let b = self.text.as_bytes();
        let n = b.len();
        if n == 0 {
            return (0, 0);
        }
        let pos = self.caret.min(n - 1);
        let ws = |c: u8| c == b' ' || c == b'\t';
        let target_ws = ws(b[pos]);
        let same = |c: u8| ws(c) == target_ws && c != b'\n';
        let mut s = pos;
        while s > 0 && same(b[s - 1]) {
            s -= 1;
        }
        let mut e = pos + 1;
        while e < n && same(b[e]) {
            e += 1;
        }
        if around && !target_ws {
            let mut a = e;
            while a < n && ws(b[a]) {
                a += 1;
            }
            if a > e {
                return (s, a);
            }
            let mut ls = s;
            while ls > 0 && ws(b[ls - 1]) {
                ls -= 1;
            }
            return (ls, e);
        }
        (s, e)
    }

    /// `i(`/`a(` and friends: the range between the bracket pair enclosing the
    /// caret, nesting-aware. `around` includes the brackets themselves.
    pub(crate) fn pair_object(&self, open: u8, close: u8, around: bool) -> Option<(usize, usize)> {
        let b = self.text.as_bytes();
        let n = b.len();
        if n == 0 {
            return None;
        }
        let start = self.caret.min(n - 1);
        // Scan left for the enclosing open bracket.
        let mut depth = 0i32;
        let mut i = start;
        let open_idx = loop {
            let ch = b[i];
            if ch == close && i != start {
                depth += 1;
            } else if ch == open {
                if depth == 0 {
                    break Some(i);
                }
                depth -= 1;
            }
            if i == 0 {
                break None;
            }
            i -= 1;
        }?;
        // Scan right for its matching close.
        let mut depth = 0i32;
        let mut j = open_idx + 1;
        let close_idx = loop {
            if j >= n {
                break None;
            }
            let ch = b[j];
            if ch == open {
                depth += 1;
            } else if ch == close {
                if depth == 0 {
                    break Some(j);
                }
                depth -= 1;
            }
            j += 1;
        }?;
        Some(if around {
            (open_idx, close_idx + 1)
        } else {
            (open_idx + 1, close_idx)
        })
    }

    /// `i"`/`a"` and friends: the range between a matching quote pair on the
    /// current line. `around` includes the quotes.
    pub(crate) fn quote_object(&self, q: u8, around: bool) -> Option<(usize, usize)> {
        let b = self.text.as_bytes();
        let ls = self.line_start(self.caret);
        let le = self.line_end(self.caret);
        let quotes: Vec<usize> = (ls..le).filter(|&i| b[i] == q).collect();
        // Pair them left-to-right; take the first pair closing at/after the caret.
        let mut k = 0;
        while k + 1 < quotes.len() {
            let (a, z) = (quotes[k], quotes[k + 1]);
            if self.caret <= z {
                return Some(if around { (a, z + 1) } else { (a + 1, z) });
            }
            k += 2;
        }
        None
    }

    /// Insert-mode Ctrl+W / Ctrl+Backspace: delete the word before the caret.
    pub(crate) fn delete_word_before(&mut self) {
        let b = self.text.as_bytes();
        let mut i = self.caret;
        while i > 0 && (b[i - 1] == b' ' || b[i - 1] == b'\t') {
            i -= 1;
        }
        while i > 0 && !b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        self.text.replace_range(i..self.caret, "");
        self.caret = i;
    }

    /// Insert-mode Cmd+Backspace: delete back to the start of the line, or the
    /// preceding newline if already there.
    pub(crate) fn delete_to_line_start(&mut self) {
        let ls = self.line_start(self.caret);
        if ls == self.caret {
            if self.caret > 0 {
                self.caret -= 1;
                self.text.remove(self.caret);
            }
        } else {
            self.text.replace_range(ls..self.caret, "");
            self.caret = ls;
        }
    }

    // --- Rendering ---------------------------------------------------------

}
