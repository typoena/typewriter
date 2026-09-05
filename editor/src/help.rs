//! The `:help` card ([`Mode::Help`]): the on-device command reference, paged.

use keymap::Key;

use crate::{Editor, Mode, ROWS};

/// One line of the card: `(keys, gloss)`. An empty gloss makes the row a section
/// heading; both empty is a spacer.
pub(crate) type HelpRow = (&'static str, &'static str);

/// Content rows a page can hold — the screen's rows less the title and the
/// footer hint.
pub(crate) const HELP_ROWS: usize = ROWS - 2;

/// Column the glosses start in, wide enough for the longest key cell
/// (`:pub  :publish`) plus a two-space gap.
pub(crate) const HELP_GLOSS_COL: usize = 16;

/// The card, one slice per page. The paging is **authored, not computed**, so a
/// section never splits across a page turn; a test holds every page to
/// [`HELP_ROWS`] lines.
pub(crate) const HELP_PAGES: [&[HelpRow]; 3] = [
    &[
        ("NOTES", ""),
        (":w  :wq  :x", "save the note"),
        (":enew <file>", "create a note at a path"),
        (":inbox  :in", "today's fleeting note"),
        (":oldest  :old", "the oldest fleeting note"),
        (":pub  :publish", "rename the note .pub.md"),
        (":delete  :d", "delete the note"),
        (":fmt", "reformat the note"),
        ("", ""),
        ("Cmd+P", "open a note"),
        ("Ctrl+Tab", "walk recently-opened notes"),
    ],
    &[
        ("SYNC", ""),
        (":gs", "format, save and push"),
        (":gl", "pull, folding in unsynced saves"),
        ("", ""),
        ("DEVICE", ""),
        (":focus", "start or stop a focus session"),
        (":settings", "open the settings palette"),
        (":update", "install a firmware update"),
        (":reboot", "restart the device"),
        (":setup", "reopen the setup wizard"),
        (":about", "version and credits"),
    ],
    &[
        ("KEYS", ""),
        ("Cmd+Shift+P", "commands (> actions, $ snippets)"),
        ("Cmd+S", "save"),
        (":", "command line"),
        ("i  a  o", "insert, append, open a line"),
        ("Esc", "back to Normal"),
        ("/  n  N", "search, next match, previous"),
        ("gf", "follow the link under the caret"),
        ("gr", "read-only scroll mode"),
        ("gs  gl", "push, pull"),
    ],
];

impl Editor {
    /// `:help` / `> help` — raise the paged command reference
    /// ([`Mode::Help`]). Always opens on the first page, so the card reads the
    /// same every time. Read-only, like the `:about` splash.
    pub(crate) fn show_help(&mut self) {
        self.mode = Mode::Help;
        self.help_page = 0;
    }

    /// Dispatch a key on the help card. Space / `j` / `n` / Ctrl+N turn the
    /// page, `k` / `p` / Ctrl+P turn back (both clamp at the ends — a wrap on a
    /// three-page card just loses your place), and `Enter` / `q` / `Esc` leave.
    /// Every other key is swallowed: the buffer is behind the card.
    pub(crate) fn help_key(&mut self, key: Key) {
        match key {
            Key::Enter | Key::Escape | Key::Char('q') => self.mode = Mode::Normal,
            Key::Char(' ') | Key::Char('j') | Key::Char('n') | Key::Down => {
                self.help_page = (self.help_page + 1).min(HELP_PAGES.len() - 1);
            }
            Key::Char('k') | Key::Char('p') | Key::Up => {
                self.help_page = self.help_page.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// The rows of the page the card is showing. Clamped, so a page index can
    /// never blank the card.
    pub(crate) fn help_rows(&self) -> &'static [HelpRow] {
        let i = self.help_page.min(HELP_PAGES.len() - 1);
        HELP_PAGES.get(i).copied().unwrap_or(&[])
    }

    /// The card's `page/total` counter, for the title line.
    pub(crate) fn help_page_label(&self) -> String {
        format!("help  ({}/{})", self.help_page + 1, HELP_PAGES.len())
    }
}
