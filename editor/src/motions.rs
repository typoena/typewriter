//! Caret motions: char/word/line stepping shared by Normal, Visual and View.

use super::*;

impl Editor {
    /// Apply a plain caret motion shared by Normal and Visual — `h l j k`,
    /// `w b e`, `0 $`, `G` — `n` times, returning whether `c` was a motion (and
    /// so consumed). `gg`/`gr` are handled by their callers' pending-`g` state,
    /// not here.
    pub(crate) fn move_by(&mut self, c: char, n: usize) -> bool {
        match c {
            'h' => (0..n).for_each(|_| self.move_left()),
            'l' => (0..n).for_each(|_| self.move_right()),
            'j' => (0..n).for_each(|_| self.move_down()),
            'k' => (0..n).for_each(|_| self.move_up()),
            'w' => (0..n).for_each(|_| self.caret = self.word_forward_pos(self.caret)),
            'b' => (0..n).for_each(|_| self.caret = self.word_back_pos(self.caret)),
            'e' => (0..n).for_each(|_| self.caret = self.word_end_pos(self.caret)),
            '0' => self.caret = self.line_start(self.caret),
            '$' => self.caret = self.line_end(self.caret),
            'G' => self.caret = self.line_start(self.text.len()),
            // Repeat the last `/` search; a motion here so Visual extends over
            // it for free. Deliberately not an operator target (`dn` is not in
            // scope) — operators resolve their own motion table in `normal_key`.
            'n' => self.search_repeat(n, true),
            'N' => self.search_repeat(n, false),
            _ => return false,
        }
        true
    }

    // --- Command mode (`:`) ------------------------------------------------

    /// Offset of the start of the line containing `pos`.
    pub(crate) fn line_start(&self, pos: usize) -> usize {
        let b = self.text.as_bytes();
        let mut i = pos;
        while i > 0 && b[i - 1] != b'\n' {
            i -= 1;
        }
        i
    }

    /// Offset of the end of the line containing `pos` (the `\n`, or buffer end).
    pub(crate) fn line_end(&self, pos: usize) -> usize {
        let b = self.text.as_bytes();
        let mut i = pos;
        while i < b.len() && b[i] != b'\n' {
            i += 1;
        }
        i
    }

    /// Byte offset one character right of `i`, clamped to the buffer end. `i`
    /// must be a char boundary (every caret position is one).
    pub(crate) fn next_char(&self, i: usize) -> usize {
        self.text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
    }

    /// Byte offset one character left of `i`, clamped to 0.
    pub(crate) fn prev_char(&self, i: usize) -> usize {
        self.text[..i].chars().next_back().map_or(i, |c| i - c.len_utf8())
    }

    /// Byte offset `col` characters into the text starting at `start`, clamped
    /// to `end` (so a shorter target line lands the caret at its end).
    pub(crate) fn advance_chars(&self, start: usize, col: usize, end: usize) -> usize {
        let mut pos = start;
        for _ in 0..col {
            if pos >= end {
                break;
            }
            pos = self.next_char(pos);
        }
        pos.min(end)
    }

    pub(crate) fn move_left(&mut self) {
        if self.caret > self.line_start(self.caret) {
            self.caret = self.prev_char(self.caret);
        }
    }

    pub(crate) fn move_right(&mut self) {
        if self.caret < self.line_end(self.caret) {
            self.caret = self.next_char(self.caret);
        }
    }

    /// Like `l` but allowed to land one past the last char (for `a`).
    pub(crate) fn move_right_append(&mut self) {
        if self.caret < self.line_end(self.caret) {
            self.caret = self.next_char(self.caret);
        }
    }

    pub(crate) fn move_down(&mut self) {
        let ls = self.line_start(self.caret);
        let col = self.text[ls..self.caret].chars().count();
        let le = self.line_end(self.caret);
        if le >= self.text.len() {
            return; // already on the last line
        }
        let next_start = le + 1;
        let next_end = self.line_end(next_start);
        self.caret = self.advance_chars(next_start, col, next_end);
    }

    pub(crate) fn move_up(&mut self) {
        let ls = self.line_start(self.caret);
        if ls == 0 {
            return; // already on the first line
        }
        let col = self.text[ls..self.caret].chars().count();
        let prev_start = self.line_start(ls - 1);
        let prev_end = ls - 1; // the '\n' that ends the previous line
        self.caret = self.advance_chars(prev_start, col, prev_end);
    }

    /// Move the caret by `delta` **display** (soft-wrapped) rows, keeping the
    /// column where the target row is long enough. This is the `Ctrl-d`/`Ctrl-u`
    /// step: unlike `j`/`k` (which move by *logical* line and so jump over
    /// wrapped continuation rows), it walks the rendered layout, so half a page
    /// is half the visible window no matter how the prose wraps. In Normal mode
    /// the caret is always kept on-screen, so moving it *is* the scroll — the
    /// viewport follows via `adjust_scroll` at draw time.
    pub(crate) fn move_display_rows(&mut self, delta: isize) {
        let lay = self.layout();
        if lay.is_empty() {
            return;
        }
        let (row, col) = self.caret_rc(&lay);
        let target = (row as isize + delta).clamp(0, lay.len() as isize - 1) as usize;
        let line = &lay[target];
        let row_end = line.start + line.text.len();
        self.caret = self.advance_chars(line.start, col, row_end);
    }

    /// Start of the next whitespace-delimited word after `from`.
    pub(crate) fn word_forward_pos(&self, from: usize) -> usize {
        let b = self.text.as_bytes();
        let n = b.len();
        let mut i = from;
        while i < n && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        while i < n && b[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    }

    /// Start of the word at or before `from`.
    pub(crate) fn word_back_pos(&self, from: usize) -> usize {
        let b = self.text.as_bytes();
        let mut i = from;
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        while i > 0 && !b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        i
    }

    /// Byte offset of the last character of the current/next word — vim `e`
    /// lands the caret on that char. Skips any leading whitespace, then runs to
    /// the word's end; whitespace includes `\n`, so it can cross lines.
    pub(crate) fn word_end_pos(&self, from: usize) -> usize {
        let start = self.next_char(from);
        if start >= self.text.len() {
            return from;
        }
        let mut last = from;
        let mut in_word = false;
        for (off, c) in self.text[start..].char_indices() {
            if c.is_ascii_whitespace() {
                if in_word {
                    break;
                }
            } else {
                in_word = true;
                last = start + off;
            }
        }
        last
    }

    // --- Edits -------------------------------------------------------------

}
