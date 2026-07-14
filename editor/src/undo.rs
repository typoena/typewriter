//! Snapshot undo/redo, bounded to [`UNDO_DEPTH`] checkpoints.

use super::*;

/// Maximum undo depth (change-groups). A full-buffer snapshot per group means
/// worst-case memory is `UNDO_DEPTH × buffer size`; for note-sized files on the
/// 8 MB PSRAM this is negligible, and prose editing rarely nears 100 groups
/// between saves anyway.
pub(crate) const UNDO_DEPTH: usize = 100;


impl Editor {
    /// Record the current `(text, caret)` as an undo baseline, at the *start* of
    /// a change-group, and drop the redo history (a new edit forks the timeline).
    /// Called once per change: on entering Insert (the whole session undoes
    /// together), and before each Normal-mode edit (`x`, `dd`, operators, paste,
    /// `:fmt`). If the buffer is unchanged since the last baseline it is a no-op,
    /// so calling it more than once before a mutation records only one group.
    pub(crate) fn checkpoint(&mut self) {
        // A change-group is about to begin, so the buffer is (or is about to be)
        // modified relative to the last save. See the `dirty` field note on why
        // this is deliberately slightly over-eager.
        self.dirty = true;
        if self.undo.last().is_some_and(|(t, _)| t == &self.text) {
            return; // nothing changed since the last baseline
        }
        self.undo.push((self.text.clone(), self.caret));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0); // drop the oldest group
        }
        self.redo.clear();
    }

    /// `u` — restore the most recent undo baseline, pushing the current state to
    /// the redo stack. Lands in Normal mode with the caret clamped onto a char
    /// boundary. No-op with nothing to undo.
    pub(crate) fn undo(&mut self) {
        if let Some((text, caret)) = self.undo.pop() {
            self.redo.push((self.text.clone(), self.caret));
            self.restore(text, caret);
        }
    }

    /// `Ctrl-r` — reapply the most recently undone state. No-op with nothing to
    /// redo.
    pub(crate) fn redo(&mut self) {
        if let Some((text, caret)) = self.redo.pop() {
            self.undo.push((self.text.clone(), self.caret));
            self.restore(text, caret);
        }
    }

    /// Swap in a snapshot's buffer + caret, landing in Normal on a char boundary.
    pub(crate) fn restore(&mut self, text: String, caret: usize) {
        self.text = text;
        self.caret = caret.min(self.text.len());
        while self.caret > 0 && !self.text.is_char_boundary(self.caret) {
            self.caret -= 1;
        }
        self.mode = Mode::Normal;
        self.reset_pending();
    }

    // --- Buffers (multi-file) ----------------------------------------------

}
