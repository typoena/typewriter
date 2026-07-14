//! Visual mode: charwise/linewise selection with y/d/c over the span.

use super::*;

impl Editor {
    /// True while a Visual selection is active (charwise or linewise).
    pub(crate) fn in_visual(&self) -> bool {
        matches!(self.mode, Mode::Visual | Mode::VisualLine)
    }

    /// Dispatch a key in Visual/VisualLine. Motions extend the selection (the
    /// anchor stays put, the caret moves); `y`/`d`/`c` act on the span and
    /// leave Visual; `v`/`V` switch submode or toggle back to Normal; `Esc`
    /// cancels. Counts and `gg`/`G` work as in Normal.
    pub(crate) fn visual_key(&mut self, key: Key) {
        let c = match key {
            Key::Char(c) => c,
            Key::HalfPageDown => {
                self.count = 0;
                self.move_display_rows(HALF_PAGE as isize);
                return;
            }
            Key::HalfPageUp => {
                self.count = 0;
                self.move_display_rows(-(HALF_PAGE as isize));
                return;
            }
            Key::Escape => {
                self.exit_visual();
                return;
            }
            // Cmd-p works from every mode: drop the selection (as Esc would)
            // and open the palette.
            Key::Palette => {
                self.exit_visual();
                self.open_palette();
                return;
            }
            // Enter/Backspace/etc. carry no Visual meaning; drop any count.
            _ => {
                self.count = 0;
                self.pending_g = false;
                return;
            }
        };

        // `gg` — jump to the top, extending the selection.
        if self.pending_g {
            self.pending_g = false;
            if c == 'g' {
                self.caret = 0;
            }
            self.count = 0;
            return;
        }

        // Count prefix, exactly as in Normal (a leading `0` is the motion).
        if c.is_ascii_digit() && !(c == '0' && self.count == 0) {
            self.count = self.count.saturating_mul(10) + (c as usize - '0' as usize);
            return;
        }
        let n = self.count.max(1);

        match c {
            'g' => {
                self.pending_g = true;
                return;
            }
            // `v` toggles charwise off (or switches VisualLine → charwise);
            // `V` toggles linewise off (or switches charwise → linewise).
            'v' => {
                if self.mode == Mode::Visual {
                    self.exit_visual();
                } else {
                    self.mode = Mode::Visual;
                }
            }
            'V' => {
                if self.mode == Mode::VisualLine {
                    self.exit_visual();
                } else {
                    self.mode = Mode::VisualLine;
                }
            }
            'y' => self.visual_yank(),
            'd' => self.visual_delete(),
            'c' => self.visual_change(),
            // Any other char: a shared motion extends the selection, or is a
            // no-op. Unlike Normal, edit keys (`x`, `p`, …) aren't bound here.
            _ => {
                self.move_by(c, n);
            }
        }
        self.count = 0;
    }

    /// The current selection as `(start, end, linewise)` byte offsets, `start <
    /// end` (or equal on an empty buffer). Charwise is vim-inclusive of the char
    /// under the further caret; linewise always spans whole logical lines from
    /// the anchor's line through the caret's.
    pub(crate) fn visual_span(&self) -> (usize, usize, bool) {
        let anchor = self.visual_anchor.unwrap_or(self.caret);
        let lo = anchor.min(self.caret);
        let hi = anchor.max(self.caret);
        if self.mode == Mode::VisualLine {
            (self.line_start(lo), self.line_end(hi), true)
        } else {
            (lo, self.next_char(hi).min(self.text.len()), false)
        }
    }

    /// Copy the selection into the unnamed register (linewise from `V`, charwise
    /// otherwise), leave the caret at the selection start, and return to Normal.
    pub(crate) fn visual_yank(&mut self) {
        let (s, e, line) = self.visual_span();
        self.register = self.selection_text(s, e, line);
        self.register_linewise = line;
        self.caret = s;
        self.exit_visual();
    }

    /// Delete the selection (filling the register like `visual_yank`), leaving
    /// the caret at the span start, and return to Normal. Linewise removes whole
    /// lines including a bounding newline, mirroring `dd`.
    pub(crate) fn visual_delete(&mut self) {
        let (s, e, line) = self.visual_span();
        self.register = self.selection_text(s, e, line);
        self.register_linewise = line;
        self.checkpoint();
        let (ds, de) = self.delete_bounds(s, e, line);
        self.text.replace_range(ds..de, "");
        self.caret = if line {
            self.line_start(ds.min(self.text.len()))
        } else {
            ds.min(self.text.len())
        };
        self.exit_visual();
    }

    /// Change the selection: delete it (filling the register) and drop into
    /// Insert at the start. A linewise change clears the lines' text but leaves
    /// one empty line to type on (like `cc`), rather than removing the line.
    pub(crate) fn visual_change(&mut self) {
        let (s, e, line) = self.visual_span();
        self.register = self.selection_text(s, e, line);
        self.register_linewise = line;
        self.checkpoint();
        // Linewise: replace only `s..e` (the text, keeping the final newline) so
        // one blank line remains. Charwise: remove the exact span.
        self.text.replace_range(s..e, "");
        self.caret = s.min(self.text.len());
        self.visual_anchor = None;
        self.pending_g = false;
        self.count = 0;
        self.mode = Mode::Insert;
    }

    /// The register contents for a selection span: charwise is the raw slice;
    /// linewise gets a synthesised trailing newline (as `yy`/`dd` store lines).
    pub(crate) fn selection_text(&self, s: usize, e: usize, line: bool) -> String {
        let mut block = self.text[s..e].to_string();
        if line && !block.ends_with('\n') {
            block.push('\n');
        }
        block
    }

    /// Byte range to actually remove for a delete. Charwise is the span as-is;
    /// linewise also eats the trailing newline (or, on the last line, the
    /// preceding one) so no blank line is left behind — matching `dd`.
    pub(crate) fn delete_bounds(&self, s: usize, e: usize, line: bool) -> (usize, usize) {
        if !line {
            return (s, e);
        }
        if e < self.text.len() {
            (s, self.next_char(e)) // eat the trailing '\n' at `e`
        } else if s > 0 {
            (self.prev_char(s), e) // last line: eat the preceding '\n' instead
        } else {
            (s, e) // whole buffer
        }
    }

    /// Leave Visual for Normal, clearing the anchor and any pending state.
    pub(crate) fn exit_visual(&mut self) {
        self.mode = Mode::Normal;
        self.visual_anchor = None;
        self.pending_g = false;
        self.count = 0;
    }

    // --- View mode ---------------------------------------------------------

}
