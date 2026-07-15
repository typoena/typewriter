//! Wizard state: which step we're on, the results each step produces, and
//! step-aware key handling (nav steps, the Configure form, and the SD-card step
//! each behave differently).

use std::sync::mpsc::{Receiver, TryRecvError};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::{Config, Field, keychain_wifi_password};
use crate::preflight::Preflight;
use crate::sdcard::{self, Card, CardInspect, SdEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Preflight,
    Configure,
    SdCard,
    Done,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Preflight, Step::Configure, Step::SdCard, Step::Done];

    pub fn title(self) -> &'static str {
        match self {
            Step::Preflight => "Preflight",
            Step::Configure => "Configure",
            Step::SdCard => "SD card",
            Step::Done => "Done",
        }
    }

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    fn next(self) -> Step {
        Step::ALL[(self.index() + 1).min(Step::ALL.len() - 1)]
    }

    fn prev(self) -> Step {
        Step::ALL[self.index().saturating_sub(1)]
    }
}

pub enum SdState {
    Idle,
    /// The selected card already holds a repo; awaiting an explicit `y` to wipe.
    ConfirmWipe(CardInspect),
    Running,
    Done,
    Failed(String),
}

pub struct App {
    pub step: Step,
    pub preflight: Preflight,
    pub config: Config,
    /// Focused field index within the Configure form.
    pub focus: usize,
    /// Transient one-line feedback (e.g. the Keychain-lookup result).
    pub status: Option<String>,
    // ── SD-card step ──
    pub cards: Vec<Card>,
    pub card_sel: usize,
    pub sd: SdState,
    pub sd_log: Vec<String>,
    /// Latest git-progress tick (phase, 0..=100), driving the SD-step gauge.
    pub sd_progress: Option<(String, u16)>,
    sd_rx: Option<Receiver<SdEvent>>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            step: Step::Preflight,
            preflight: Preflight::run(),
            config: Config::derived(),
            focus: 0,
            status: None,
            cards: Vec::new(),
            card_sel: 0,
            sd: SdState::Idle,
            sd_log: Vec::new(),
            sd_progress: None,
            sd_rx: None,
            should_quit: false,
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // Ctrl-C always quits, on any step (even mid-typing / mid-run).
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        self.status = None;
        match self.step {
            Step::Configure => self.on_key_configure(key),
            Step::SdCard => self.on_key_sdcard(key),
            _ => self.on_key_nav(key),
        }
    }

    /// Non-form steps: single-key navigation. Arrows, Tab/Shift-Tab, and vim
    /// h/j/k/l all move between steps so no pointer or arrow key is required.
    fn on_key_nav(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('r') if self.step == Step::Preflight => {
                self.preflight = Preflight::run();
            }
            KeyCode::Enter
            | KeyCode::Tab
            | KeyCode::Down
            | KeyCode::Right
            | KeyCode::Char('j')
            | KeyCode::Char('l') => self.next(),
            KeyCode::Up
            | KeyCode::BackTab
            | KeyCode::Left
            | KeyCode::Char('k')
            | KeyCode::Char('h') => self.prev(),
            _ => {}
        }
    }

    /// Configure form: typing edits the focused field; field navigation spills
    /// over into step navigation at the ends.
    fn on_key_configure(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let last = Field::ALL.len() - 1;
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            // Shift-Tab / ↑ walk fields up; past the first, back a step.
            KeyCode::Up | KeyCode::BackTab => {
                if self.focus > 0 {
                    self.focus -= 1;
                } else {
                    self.prev();
                }
            }
            KeyCode::Down if self.focus < last => self.focus += 1,
            // Tab walks fields down; past the last, on to the next step — so a
            // pure Tab/Shift-Tab rhythm carries you through the whole wizard.
            KeyCode::Tab => {
                if self.focus < last {
                    self.focus += 1;
                } else {
                    self.next();
                }
            }
            KeyCode::Enter => {
                if self.focus < last {
                    self.focus += 1;
                } else {
                    self.next();
                }
            }
            KeyCode::Char('u') if ctrl => self.config.get_mut(self.focused_field()).clear(),
            KeyCode::Char('k') if ctrl => self.fill_wifi_from_keychain(),
            KeyCode::Backspace => {
                self.config.get_mut(self.focused_field()).pop();
            }
            KeyCode::Char(c) if !ctrl => self.config.get_mut(self.focused_field()).push(c),
            _ => {}
        }
    }

    /// SD-card step: pick a card, then start (or confirm-wipe-then-start) the
    /// provision.
    fn on_key_sdcard(&mut self, key: KeyEvent) {
        match self.sd {
            SdState::Running => return, // input locked while the worker runs
            SdState::ConfirmWipe(_) => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => self.start_provision(true),
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        self.sd = SdState::Idle
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            // ↑/k/Shift-Tab walk the card list up; past the top, back a step.
            KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                if self.card_sel > 0 {
                    self.card_sel -= 1;
                } else {
                    self.prev();
                }
            }
            KeyCode::Down | KeyCode::Char('j') if self.card_sel + 1 < self.cards.len() => {
                self.card_sel += 1;
            }
            KeyCode::Tab if self.card_sel + 1 < self.cards.len() => self.card_sel += 1,
            // ←/h steps back; forward is deliberately gated behind writing a card.
            KeyCode::Left | KeyCode::Char('h') => self.prev(),
            KeyCode::Char('r') => {
                self.sd = SdState::Idle;
                self.sd_log.clear();
                self.sd_progress = None;
                self.refresh_cards();
            }
            KeyCode::Enter => self.attempt_provision(),
            _ => {}
        }
    }

    pub fn focused_field(&self) -> Field {
        Field::ALL[self.focus.min(Field::ALL.len() - 1)]
    }

    /// Whether the current step's forward gate is satisfied, so Enter/Tab may
    /// advance. Drives the sidebar's "ready / finish this step" affordance.
    pub fn forward_open(&self) -> bool {
        match self.step {
            Step::Preflight => true, // advisory — never blocks
            Step::Configure => self.config.missing_required().is_empty(),
            Step::SdCard => matches!(self.sd, SdState::Done),
            Step::Done => false,
        }
    }

    /// The step a forward move would land on, if any (None on the last step).
    pub fn next_step(&self) -> Option<Step> {
        (self.step != Step::Done).then(|| self.step.next())
    }

    fn fill_wifi_from_keychain(&mut self) {
        let ssid = self.config.wifi_ssid.clone();
        self.status = Some(match keychain_wifi_password(&ssid) {
            Some(pw) => {
                self.config.wifi_pass = pw;
                format!("filled Wi-Fi password for “{ssid}” from Keychain")
            }
            None => "no Keychain password found (or the lookup was cancelled)".into(),
        });
    }

    fn refresh_cards(&mut self) {
        self.cards = sdcard::detect_cards();
        self.card_sel = 0;
    }

    fn selected_volume(&self) -> Option<std::path::PathBuf> {
        self.cards
            .get(self.card_sel.min(self.cards.len().saturating_sub(1)))
            .map(|c| c.volume.clone())
    }

    /// Enter on a card: validate, then either start a fresh provision or, if the
    /// card already holds a repo, drop into the wipe-confirm screen.
    fn attempt_provision(&mut self) {
        if self.cards.is_empty() {
            self.status = Some("no card detected — insert one and press r".into());
            return;
        }
        if !self.config.missing_required().is_empty() {
            self.status = Some("fill the required fields on the Configure step first".into());
            return;
        }
        let Some(vol) = self.selected_volume() else {
            return;
        };
        if sdcard::card_has_repo(&vol) {
            self.sd = SdState::ConfirmWipe(sdcard::inspect_card(&vol));
        } else {
            self.start_provision(false);
        }
    }

    fn start_provision(&mut self, wipe: bool) {
        let Some(card_volume) = self.selected_volume() else {
            return;
        };
        let plan = sdcard::Plan {
            remote: self.config.remote_url.clone(),
            pat: self.config.pat.clone(),
            card_volume,
            conf_body: self.config.to_conf(),
            wipe,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.sd_rx = Some(rx);
        self.sd = SdState::Running;
        self.sd_log.clear();
        self.sd_progress = None;
        std::thread::spawn(move || sdcard::run_provision(plan, tx));
    }

    /// Pull worker progress into the log; advance to Done on success.
    pub fn drain_worker(&mut self) {
        let Some(rx) = self.sd_rx.take() else {
            return;
        };
        let mut done = None;
        loop {
            match rx.try_recv() {
                Ok(SdEvent::Log(l)) => self.sd_log.push(l),
                Ok(SdEvent::Progress { phase, pct }) => self.sd_progress = Some((phase, pct)),
                Ok(SdEvent::Done(r)) => {
                    done = Some(r);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    done = Some(Err("worker thread stopped unexpectedly".into()));
                    break;
                }
            }
        }
        match done {
            Some(Ok(())) => {
                self.sd = SdState::Done;
                self.step = Step::Done;
            }
            Some(Err(e)) => self.sd = SdState::Failed(e),
            None => self.sd_rx = Some(rx),
        }
    }

    fn next(&mut self) {
        self.step = self.step.next();
        self.on_enter();
    }

    fn prev(&mut self) {
        self.step = self.step.prev();
        self.on_enter();
    }

    fn on_enter(&mut self) {
        if self.step == Step::SdCard && matches!(self.sd, SdState::Idle) {
            self.refresh_cards();
        }
    }
}
