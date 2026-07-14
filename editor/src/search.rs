//! `/` search execution and `n`/`N` repeats (smartcase + accent folding).

use super::*;

impl Editor {
    /// Run the typed `/` search: remember the pattern (a bare `/`+Enter repeats
    /// the last one, like vim) and jump forward once. Literal substring — no
    /// regex on a writing appliance — matched **smartcase** (an all-lowercase
    /// pattern is case-insensitive; one capital makes it exact, vim-style) and
    /// always **accent-folded** (`/ete` finds `été`; see [`fold`]).
    pub(crate) fn execute_search(&mut self) {
        if !self.cmdline.is_empty() {
            self.last_search = self.cmdline.clone();
        }
        self.search_repeat(1, true);
    }

    /// Jump `n` matches of [`last_search`](Self::last_search) forward
    /// (`fwd`, the `/`-Enter and `n` step) or backward (`N`), wrapping around
    /// the buffer with a "wrapped" notice. A missing pattern or an absent
    /// match posts a notice and leaves the caret alone.
    pub(crate) fn search_repeat(&mut self, n: usize, fwd: bool) {
        if self.last_search.is_empty() {
            self.set_notice("no previous search");
            return;
        }
        let pat = self.last_search.clone();
        // Smartcase: any capital in the pattern makes the search exact
        // (`n`/`N` recompute from the remembered pattern, so a repeat behaves
        // like the original search).
        let ci = !pat.chars().any(char::is_uppercase);
        for _ in 0..n {
            // Start strictly past (or before) the caret so a caret already on a
            // match moves to the next one, per vim.
            let hit = if fwd {
                let start = if self.caret >= self.text.len() {
                    self.text.len()
                } else {
                    self.next_char(self.caret)
                };
                match find_fold(&self.text[start..], &pat, ci) {
                    Some(i) => Some((start + i, false)),
                    None => find_fold(&self.text, &pat, ci).map(|i| (i, true)),
                }
            } else {
                match rfind_fold(&self.text[..self.caret], &pat, ci) {
                    Some(i) => Some((i, false)),
                    None => rfind_fold(&self.text, &pat, ci).map(|i| (i, true)),
                }
            };
            match hit {
                Some((pos, wrapped)) => {
                    self.caret = pos;
                    if wrapped {
                        self.set_notice("wrapped");
                    }
                }
                None => {
                    self.set_notice(format!("not found: {pat}"));
                    return; // absent now, absent on every repeat
                }
            }
        }
    }

}
