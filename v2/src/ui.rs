//! Browser front end: builds the board, handles mouse / touch / keyboard
//! input, runs the timer and renders state changes from the engine.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, Event, EventTarget, HtmlDialogElement, HtmlElement, HtmlInputElement,
    KeyboardEvent, MouseEvent, PointerEvent, Window,
};

use crate::engine::{Board, Config, Mark, Status, MAX_SIDE, MIN_SIDE};
use crate::storage::{self, format_ms, Stats};

const FLAG_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path class="pole" d="M6 3v18"/><path class="cloth" d="M7 4h11l-3 4 3 4H7z"/></svg>"#;
const MINE_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><g class="spikes"><path d="M12 2v20M2 12h20M4.9 4.9l14.2 14.2M19.1 4.9 4.9 19.1"/></g><circle cx="12" cy="12" r="6.5"/><circle class="shine" cx="9.8" cy="9.8" r="1.8"/></svg>"#;
const SUN_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4.5"/><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22M4.9 4.9l1.8 1.8M17.3 17.3l1.8 1.8M4.9 19.1l1.8-1.8M17.3 6.7l1.8-1.8"/></svg>"#;
const MOON_SVG: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>"#;

const LONG_PRESS_MS: i32 = 380;
const MOVE_TOLERANCE: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    Easy,
    Medium,
    Hard,
    Custom,
}

impl Level {
    const ALL: [Level; 4] = [Level::Easy, Level::Medium, Level::Hard, Level::Custom];

    fn key(self) -> &'static str {
        match self {
            Level::Easy => "easy",
            Level::Medium => "medium",
            Level::Hard => "hard",
            Level::Custom => "custom",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Level::Easy => "Easy",
            Level::Medium => "Medium",
            Level::Hard => "Hard",
            Level::Custom => "Custom",
        }
    }

    fn from_key(key: &str) -> Option<Level> {
        Level::ALL.into_iter().find(|l| l.key() == key)
    }

    /// Hard is 16 × 30. On portrait screens it is flipped to 30 × 16 so it
    /// fits a phone without sideways scrolling; the game is identical.
    fn config(self, portrait: bool, custom: Config) -> Config {
        match self {
            Level::Easy => Config {
                rows: 9,
                cols: 9,
                mines: 10,
            },
            Level::Medium => Config {
                rows: 16,
                cols: 16,
                mines: 40,
            },
            Level::Hard if portrait => Config {
                rows: 30,
                cols: 16,
                mines: 99,
            },
            Level::Hard => Config {
                rows: 16,
                cols: 30,
                mines: 99,
            },
            Level::Custom => custom,
        }
    }
}

/// How a cell is currently drawn. Used to skip DOM writes for unchanged cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Hidden,
    Flagged,
    Open(u8),
    Mine,
    Exploded,
    WrongFlag,
}

impl View {
    fn is_open(self) -> bool {
        matches!(self, View::Open(_) | View::Mine | View::Exploded)
    }
}

struct Press {
    cell: usize,
    x: i32,
    y: i32,
    timeout: i32,
}

struct App {
    window: Window,
    doc: Document,
    board_el: HtmlElement,
    board: Board,
    level: Level,
    custom: Config,
    cells: Vec<HtmlElement>,
    views: Vec<View>,
    focus: usize,
    flag_mode: bool,
    portrait: bool,
    /// Bumped on every new game so stale timeouts can tell they are stale.
    game_id: u32,
    started_at: Option<f64>,
    elapsed_ms: u32,
    timer: Option<i32>,
    tick: js_sys::Function,
    press: Option<Press>,
    suppress_click: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` against the app state. Silently skips re-entrant calls instead of
/// panicking, which keeps a stray nested event from killing the game.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut app| app.as_mut().map(f))
    })
}

fn listen<F: FnMut(Event) + 'static>(target: &EventTarget, name: &str, f: F) {
    let closure = Closure::<dyn FnMut(Event)>::new(f);
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}

fn set_timeout(window: &Window, ms: i32, f: impl FnOnce() + 'static) -> i32 {
    let cb = Closure::once_into_js(f);
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms)
        .unwrap_or(0)
}

fn by_id(doc: &Document, id: &str) -> Element {
    doc.get_element_by_id(id)
        .unwrap_or_else(|| panic!("missing #{id}"))
}

fn dialog(doc: &Document, id: &str) -> HtmlDialogElement {
    by_id(doc, id).unchecked_into()
}

fn input(doc: &Document, id: &str) -> HtmlInputElement {
    by_id(doc, id).unchecked_into()
}

fn set_text(doc: &Document, id: &str, text: &str) {
    by_id(doc, id).set_text_content(Some(text));
}

fn now(window: &Window) -> f64 {
    window.performance().map(|p| p.now()).unwrap_or(0.0)
}

fn is_portrait(window: &Window) -> bool {
    let w = window
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let h = window
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    h > w
}

fn media(window: &Window, query: &str) -> bool {
    window
        .match_media(query)
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}

fn vibrate(window: &Window, pattern: &[u32]) {
    let nav = window.navigator();
    if !js_sys::Reflect::has(&nav, &"vibrate".into()).unwrap_or(false) {
        return;
    }
    let arr: js_sys::Array = pattern.iter().map(|&n| JsValue::from(n)).collect();
    let _ = nav.vibrate_with_pattern(&arr);
}

fn any_dialog_open(doc: &Document) -> bool {
    doc.query_selector("dialog[open]").ok().flatten().is_some()
}

/// Index of the cell an event happened on, if any.
fn cell_index(event: &Event) -> Option<usize> {
    let target: Element = event.target()?.dyn_into().ok()?;
    let cell = target.closest(".cell").ok().flatten()?;
    cell.get_attribute("data-i")?.parse().ok()
}

impl App {
    fn new_game(&mut self, level: Level) {
        self.stop_timer();
        self.cancel_press();
        self.game_id = self.game_id.wrapping_add(1);
        self.level = level;
        self.portrait = is_portrait(&self.window);
        self.started_at = None;
        self.elapsed_ms = 0;
        storage::set("level", level.key());

        let config = level.config(self.portrait, self.custom);
        let seed =
            (js_sys::Math::random() * 9_007_199_254_740_991.0) as u64 ^ now(&self.window).to_bits();
        self.board = Board::new(config, seed);
        self.build_cells();

        for l in Level::ALL {
            if let Some(btn) = self.doc.get_element_by_id(&format!("level-{}", l.key())) {
                let _ =
                    btn.set_attribute("aria-checked", if l == level { "true" } else { "false" });
            }
        }
        let size = format!("{} × {} · {} mines", config.rows, config.cols, config.mines);
        set_text(&self.doc, "level-detail", &size);
        self.set_face("🙂");
        self.update_hud();
        self.announce(&format!("New {} game. {size}.", level.label()));
        dialog(&self.doc, "dlg-result").close();
    }

    fn build_cells(&mut self) {
        let Config { rows, cols, .. } = self.board.config;
        self.board_el.set_inner_html("");
        let style = self.board_el.style();
        let _ = style.set_property("--rows", &rows.to_string());
        let _ = style.set_property("--cols", &cols.to_string());

        self.cells.clear();
        self.views = vec![View::Hidden; rows * cols];
        self.focus = self.board.index(rows / 2, cols / 2);

        for r in 0..rows {
            let row = self.doc.create_element("div").unwrap();
            row.set_class_name("row");
            let _ = row.set_attribute("role", "row");
            for c in 0..cols {
                let i = self.board.index(r, c);
                let btn: HtmlElement = self.doc.create_element("button").unwrap().unchecked_into();
                btn.set_class_name("cell");
                let _ = btn.set_attribute("type", "button");
                let _ = btn.set_attribute("role", "gridcell");
                let _ = btn.set_attribute("data-i", &i.to_string());
                let _ = btn.set_attribute("tabindex", if i == self.focus { "0" } else { "-1" });
                let _ = btn.set_attribute(
                    "aria-label",
                    &format!("Hidden, row {}, column {}", r + 1, c + 1),
                );
                let _ = row.append_child(&btn);
                self.cells.push(btn);
            }
            let _ = self.board_el.append_child(&row);
        }
    }

    fn view_of(&self, i: usize) -> View {
        let cell = self.board.cell(i);
        let lost = self.board.status() == Status::Lost;
        match cell.mark {
            Mark::Revealed if cell.mine => View::Exploded,
            Mark::Revealed => View::Open(cell.adjacent),
            Mark::Flagged if lost && !cell.mine => View::WrongFlag,
            Mark::Flagged => View::Flagged,
            Mark::Hidden if lost && cell.mine => View::Mine,
            Mark::Hidden => View::Hidden,
        }
    }

    /// Re-draws every cell whose view changed. Newly opened cells get an
    /// animation delay that grows with their distance from `origin`.
    fn paint(&mut self, origin: Option<usize>, step_ms: usize) {
        for i in 0..self.cells.len() {
            let view = self.view_of(i);
            let old = self.views[i];
            if view == old {
                continue;
            }
            self.views[i] = view;
            let el = &self.cells[i];
            let (r, c) = self.board.row_col(i);

            let delay = match origin {
                Some(o) if view.is_open() && !old.is_open() => {
                    let (or, oc) = self.board.row_col(o);
                    (r.abs_diff(or).max(c.abs_diff(oc)) * step_ms).min(900)
                }
                _ => 0,
            };
            let _ = el.style().set_property("--d", &format!("{delay}ms"));

            let (class, html, label): (String, &str, String) = match view {
                View::Hidden => ("cell".into(), "", "Hidden".into()),
                View::Flagged => ("cell flagged".into(), FLAG_SVG, "Flagged".into()),
                View::Open(0) => ("cell open n0".into(), "", "Empty".into()),
                View::Open(n) => {
                    let label = if n == 1 {
                        "1 mine nearby".into()
                    } else {
                        format!("{n} mines nearby")
                    };
                    (format!("cell open n{n}"), "", label)
                }
                View::Mine => ("cell open mine".into(), MINE_SVG, "Mine".into()),
                View::Exploded => (
                    "cell open mine exploded".into(),
                    MINE_SVG,
                    "Exploded mine".into(),
                ),
                View::WrongFlag => (
                    "cell wrong".into(),
                    FLAG_SVG,
                    "Wrong flag, no mine here".into(),
                ),
            };
            el.set_class_name(&class);
            match view {
                View::Open(n) if n > 0 => el.set_text_content(Some(&n.to_string())),
                _ => el.set_inner_html(html),
            }
            let _ = el.set_attribute(
                "aria-label",
                &format!("{label}, row {}, column {}", r + 1, c + 1),
            );
        }
    }

    fn update_hud(&self) {
        set_text(
            &self.doc,
            "mines-left",
            &self.board.mines_left().to_string(),
        );
        let secs = (self.elapsed_ms / 1000).min(999);
        set_text(&self.doc, "timer", &secs.to_string());
    }

    fn set_face(&self, face: &str) {
        set_text(&self.doc, "face", face);
    }

    fn announce(&self, msg: &str) {
        set_text(&self.doc, "live", msg);
    }

    // ----- timer -----

    fn start_timer(&mut self) {
        self.started_at = Some(now(&self.window));
        self.timer = self
            .window
            .set_interval_with_callback_and_timeout_and_arguments_0(&self.tick, 250)
            .ok();
    }

    fn stop_timer(&mut self) {
        if let Some(id) = self.timer.take() {
            self.window.clear_interval_with_handle(id);
        }
        self.sync_elapsed();
    }

    fn sync_elapsed(&mut self) {
        if let Some(start) = self.started_at {
            self.elapsed_ms = (now(&self.window) - start).max(0.0) as u32;
        }
    }

    fn on_tick(&mut self) {
        self.sync_elapsed();
        self.update_hud();
    }

    // ----- moves -----

    fn dig(&mut self, i: usize) {
        if self.board.is_over() {
            return;
        }
        let was_ready = self.board.status() == Status::Ready;
        let opened = self.board.dig(i);
        if opened.is_empty() {
            return;
        }
        if was_ready {
            self.start_timer();
        }
        self.after_move(i);
    }

    fn flag(&mut self, i: usize) {
        if self.board.toggle_flag(i) {
            vibrate(&self.window, &[12]);
            self.paint(None, 0);
            self.update_hud();
        }
    }

    /// Tap / left click / Enter.
    fn primary(&mut self, i: usize) {
        if self.flag_mode && self.board.cell(i).mark != Mark::Revealed {
            self.flag(i);
        } else {
            self.dig(i);
        }
    }

    /// Long-press / right click / F key: the opposite of the current mode.
    fn secondary(&mut self, i: usize) {
        if self.flag_mode || self.board.cell(i).mark == Mark::Revealed {
            self.dig(i);
        } else {
            self.flag(i);
        }
    }

    fn after_move(&mut self, origin: usize) {
        match self.board.status() {
            Status::Lost => {
                self.stop_timer();
                let exploded = self.board.exploded().unwrap_or(origin);
                self.paint(Some(exploded), 45);
                self.set_face("😵");
                vibrate(&self.window, &[70, 50, 140]);
                self.announce("Boom! You hit a mine. Game over.");
                self.finish(false);
            }
            Status::Won => {
                self.stop_timer();
                self.paint(Some(origin), 18);
                self.set_face("😎");
                vibrate(&self.window, &[30, 40, 30]);
                self.announce(&format!("You won in {}!", format_ms(self.elapsed_ms)));
                self.confetti();
                self.finish(true);
            }
            _ => self.paint(Some(origin), 18),
        }
        self.update_hud();
    }

    fn finish(&mut self, won: bool) {
        let ms = self.elapsed_ms;
        let (stats, new_best) = if self.level == Level::Custom {
            (None, false)
        } else {
            let mut stats = Stats::load(self.level.key());
            let new_best = stats.record(won, ms);
            stats.save(self.level.key());
            (Some(stats), new_best)
        };

        let doc = &self.doc;
        set_text(doc, "res-emoji", if won { "🏆" } else { "💥" });
        set_text(
            doc,
            "res-title",
            if won {
                "You cleared the field!"
            } else {
                "Boom! Game over"
            },
        );
        set_text(
            doc,
            "res-sub",
            if won {
                "Every mine found. Nicely done."
            } else {
                "That one was a mine. Shake it off and try again."
            },
        );
        set_text(doc, "res-time", &format_ms(ms));
        set_text(doc, "res-level", self.level.label());
        let best = stats
            .and_then(|s| s.best_ms)
            .map(format_ms)
            .unwrap_or_else(|| "—".into());
        set_text(doc, "res-best", &best);
        let streak = stats
            .map(|s| s.streak.to_string())
            .unwrap_or_else(|| "—".into());
        set_text(doc, "res-streak", &streak);
        let _ = by_id(doc, "res-newbest")
            .class_list()
            .toggle_with_force("hidden", !new_best);
        let _ = by_id(doc, "dlg-result")
            .class_list()
            .toggle_with_force("won", won);

        let game_id = self.game_id;
        set_timeout(&self.window, if won { 1100 } else { 1300 }, move || {
            with_app(|app| {
                if app.game_id == game_id && app.board.is_over() && !any_dialog_open(&app.doc) {
                    let _ = dialog(&app.doc, "dlg-result").show_modal();
                    let _ = by_id(&app.doc, "res-again")
                        .unchecked_into::<HtmlElement>()
                        .focus();
                }
            });
        });
    }

    fn confetti(&self) {
        if media(&self.window, "(prefers-reduced-motion: reduce)") {
            return;
        }
        let layer = by_id(&self.doc, "confetti");
        let mut html = String::new();
        for k in 0..90 {
            let rnd = |m: f64| (js_sys::Math::random() * m).floor() as i32;
            html.push_str(&format!(
                r#"<i style="--x:{}vw;--dx:{}px;--h:{};--r:{}deg;--dl:{}ms;--s:{}"></i>"#,
                rnd(100.0),
                rnd(240.0) - 120,
                (k * 37 + rnd(40.0)) % 360,
                rnd(720.0),
                rnd(700.0),
                0.6 + js_sys::Math::random() * 0.8,
            ));
        }
        layer.set_inner_html(&html);
        let game_id = self.game_id;
        set_timeout(&self.window, 4200, move || {
            with_app(|app| {
                if app.game_id == game_id {
                    by_id(&app.doc, "confetti").set_inner_html("");
                }
            });
        });
    }

    // ----- focus & keyboard -----

    fn move_focus(&mut self, dr: isize, dc: isize) {
        let Config { rows, cols, .. } = self.board.config;
        let (r, c) = self.board.row_col(self.focus);
        let nr = (r as isize + dr).clamp(0, rows as isize - 1) as usize;
        let nc = (c as isize + dc).clamp(0, cols as isize - 1) as usize;
        self.set_focus(self.board.index(nr, nc));
    }

    fn set_focus(&mut self, i: usize) {
        if let Some(old) = self.cells.get(self.focus) {
            let _ = old.set_attribute("tabindex", "-1");
        }
        self.focus = i;
        if let Some(el) = self.cells.get(i) {
            let _ = el.set_attribute("tabindex", "0");
            let _ = el.focus();
        }
    }

    // ----- long press -----

    fn cancel_press(&mut self) {
        if let Some(p) = self.press.take() {
            self.window.clear_timeout_with_handle(p.timeout);
        }
    }

    fn toggle_flag_mode(&mut self) {
        self.flag_mode = !self.flag_mode;
        let btn = by_id(&self.doc, "btn-mode");
        let _ = btn.set_attribute(
            "aria-pressed",
            if self.flag_mode { "true" } else { "false" },
        );
        if let Some(body) = self.doc.body() {
            let _ = body
                .class_list()
                .toggle_with_force("flag-mode", self.flag_mode);
        }
        self.update_hint();
        self.announce(if self.flag_mode {
            "Flag mode on"
        } else {
            "Dig mode on"
        });
    }

    fn update_hint(&self) {
        let touch = media(&self.window, "(pointer: coarse)");
        let hint = match (touch, self.flag_mode) {
            (true, false) => "Tap to dig · long-press to flag",
            (true, true) => "Tap to flag · long-press to dig",
            (false, false) => "Click to dig · right-click to flag",
            (false, true) => "Click to flag · right-click to dig",
        };
        set_text(&self.doc, "hint", hint);
    }

    // ----- theme, stats, custom -----

    fn toggle_theme(&self) {
        let root = self.doc.document_element().unwrap();
        let dark = match root.get_attribute("data-theme").as_deref() {
            Some("dark") => true,
            Some("light") => false,
            _ => media(&self.window, "(prefers-color-scheme: dark)"),
        };
        let next = if dark { "light" } else { "dark" };
        let _ = root.set_attribute("data-theme", next);
        storage::set("theme", next);
        self.sync_theme_icon();
    }

    fn sync_theme_icon(&self) {
        let root = self.doc.document_element().unwrap();
        let dark = match root.get_attribute("data-theme").as_deref() {
            Some("dark") => true,
            Some("light") => false,
            _ => media(&self.window, "(prefers-color-scheme: dark)"),
        };
        let btn = by_id(&self.doc, "btn-theme");
        btn.set_inner_html(if dark { SUN_SVG } else { MOON_SVG });
        let _ = btn.set_attribute(
            "aria-label",
            if dark {
                "Switch to light theme"
            } else {
                "Switch to dark theme"
            },
        );
        let color = if dark { "#0b1020" } else { "#eef2ff" };
        if let Ok(Some(meta)) = self.doc.query_selector("meta[name=theme-color]") {
            let _ = meta.set_attribute("content", color);
        }
    }

    fn render_stats(&self) {
        let mut rows = String::new();
        for level in [Level::Easy, Level::Medium, Level::Hard] {
            let s = Stats::load(level.key());
            let best = s.best_ms.map(format_ms).unwrap_or_else(|| "—".into());
            rows.push_str(&format!(
                "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}%</td><td>{}</td><td>{}</td></tr>",
                level.label(),
                s.played,
                s.win_rate(),
                best,
                s.best_streak
            ));
        }
        by_id(&self.doc, "stats-body").set_inner_html(&rows);
    }

    fn open_custom(&self) {
        input(&self.doc, "cust-rows").set_value(&self.custom.rows.to_string());
        input(&self.doc, "cust-cols").set_value(&self.custom.cols.to_string());
        input(&self.doc, "cust-mines").set_value(&self.custom.mines.to_string());
        set_text(&self.doc, "cust-error", "");
        self.sync_custom_max();
        let _ = dialog(&self.doc, "dlg-custom").show_modal();
    }

    fn read_custom(&self) -> Result<Config, String> {
        let num = |id: &str| -> Result<usize, String> {
            let v = input(&self.doc, id).value_as_number();
            if v.is_finite() && v >= 0.0 && v.fract() == 0.0 {
                Ok(v as usize)
            } else {
                Err("Please enter whole numbers.".into())
            }
        };
        Config::custom(num("cust-rows")?, num("cust-cols")?, num("cust-mines")?)
    }

    fn sync_custom_max(&self) {
        let rows = input(&self.doc, "cust-rows").value_as_number();
        let cols = input(&self.doc, "cust-cols").value_as_number();
        let clamp = |v: f64| {
            if v.is_finite() {
                (v as usize).clamp(MIN_SIDE, MAX_SIDE)
            } else {
                MIN_SIDE
            }
        };
        let max = Config::max_mines(clamp(rows), clamp(cols));
        let _ = by_id(&self.doc, "cust-mines").set_attribute("max", &max.to_string());
        set_text(&self.doc, "cust-mines-hint", &format!("1 – {max}"));
    }
}

pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let doc = window.document().ok_or("no document")?;
    let board_el: HtmlElement = by_id(&doc, "board").unchecked_into();

    let tick = Closure::<dyn FnMut()>::new(|| {
        with_app(App::on_tick);
    });
    let tick_fn: js_sys::Function = tick.as_ref().unchecked_ref::<js_sys::Function>().clone();
    tick.forget();

    let custom = storage::get("custom")
        .and_then(|s| {
            let v: Vec<usize> = s.split(',').filter_map(|p| p.parse().ok()).collect();
            match v[..] {
                [r, c, m] => Config::custom(r, c, m).ok(),
                _ => None,
            }
        })
        .unwrap_or(Config {
            rows: 12,
            cols: 12,
            mines: 24,
        });
    let level = storage::get("level")
        .and_then(|k| Level::from_key(&k))
        .unwrap_or(Level::Easy);

    let app = App {
        window: window.clone(),
        doc: doc.clone(),
        board_el: board_el.clone(),
        board: Board::new(
            Config {
                rows: 9,
                cols: 9,
                mines: 10,
            },
            1,
        ),
        level,
        custom,
        cells: Vec::new(),
        views: Vec::new(),
        focus: 0,
        flag_mode: false,
        portrait: is_portrait(&window),
        game_id: 0,
        started_at: None,
        elapsed_ms: 0,
        timer: None,
        tick: tick_fn,
        press: None,
        suppress_click: false,
    };
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    wire_board(&board_el);
    wire_controls(&window, &doc);

    with_app(|app| {
        if let Some(theme) = storage::get("theme") {
            let _ = app
                .doc
                .document_element()
                .unwrap()
                .set_attribute("data-theme", &theme);
        }
        app.sync_theme_icon();
        app.update_hint();
        app.new_game(level);
    });
    doc.document_element()
        .unwrap()
        .class_list()
        .add_1("ready")?;
    Ok(())
}

fn wire_board(board_el: &HtmlElement) {
    // Clicks: mouse left button, finger tap, and Enter/Space on a focused cell.
    listen(board_el, "click", |e| {
        let Some(i) = cell_index(&e) else { return };
        let keyboard = e.dyn_ref::<MouseEvent>().is_some_and(|m| m.detail() == 0);
        with_app(|app| {
            if app.suppress_click && !keyboard {
                app.suppress_click = false;
                return;
            }
            if app.focus != i {
                app.set_focus(i);
            }
            app.primary(i);
        });
    });

    // Right click (desktop) and the browser's own long-press menu (Android).
    listen(board_el, "contextmenu", |e| {
        e.prevent_default();
        let Some(i) = cell_index(&e) else { return };
        with_app(|app| {
            if app.suppress_click {
                return; // our long-press timer already handled this press
            }
            app.cancel_press();
            app.suppress_click = true;
            app.secondary(i);
        });
    });

    listen(board_el, "pointerdown", |e| {
        let Some(p) = e.dyn_ref::<PointerEvent>() else {
            return;
        };
        let Some(i) = cell_index(&e) else { return };
        let (x, y, button) = (p.client_x(), p.client_y(), p.button());
        // Long-press is for touch and pens; mice use right click.
        let long_press = p.pointer_type() != "mouse";
        with_app(|app| {
            app.suppress_click = false;
            app.cancel_press();
            if app.board.is_over() || button != 0 {
                return;
            }
            if app.board.cell(i).mark != Mark::Revealed {
                app.set_face("😮");
            }
            if !long_press {
                return;
            }
            let timeout = set_timeout(&app.window, LONG_PRESS_MS, move || {
                with_app(|app| {
                    if app.press.as_ref().is_some_and(|p| p.cell == i) {
                        app.press = None;
                        app.suppress_click = true;
                        app.secondary(i);
                    }
                });
            });
            app.press = Some(Press {
                cell: i,
                x,
                y,
                timeout,
            });
        });
    });

    listen(board_el, "pointermove", |e| {
        let Some(p) = e.dyn_ref::<PointerEvent>() else {
            return;
        };
        let (x, y) = (p.client_x(), p.client_y());
        with_app(|app| {
            let moved = app.press.as_ref().is_some_and(|p| {
                (p.x - x).abs() > MOVE_TOLERANCE || (p.y - y).abs() > MOVE_TOLERANCE
            });
            if moved {
                app.cancel_press();
            }
        });
    });

    let release = |_e: Event| {
        with_app(|app| {
            app.cancel_press();
            if !app.board.is_over() {
                app.set_face("🙂");
            }
        });
    };
    for name in ["pointerup", "pointercancel", "pointerleave"] {
        listen(board_el, name, release);
    }

    listen(board_el, "keydown", |e| {
        let Some(k) = e.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if k.ctrl_key() || k.meta_key() || k.alt_key() {
            return;
        }
        let key = k.key();
        let handled = with_app(|app| {
            match key.as_str() {
                "ArrowUp" => app.move_focus(-1, 0),
                "ArrowDown" => app.move_focus(1, 0),
                "ArrowLeft" => app.move_focus(0, -1),
                "ArrowRight" => app.move_focus(0, 1),
                "f" | "F" => app.secondary(app.focus),
                _ => return false,
            }
            true
        });
        if handled == Some(true) {
            e.prevent_default();
        }
    });
}

fn on_click(doc: &Document, id: &str, f: impl FnMut(Event) + 'static) {
    listen(&by_id(doc, id), "click", f);
}

fn wire_controls(window: &Window, doc: &Document) {
    for level in Level::ALL {
        on_click(doc, &format!("level-{}", level.key()), move |_| {
            with_app(|app| {
                if level == Level::Custom {
                    app.open_custom();
                } else {
                    app.new_game(level);
                }
            });
        });
    }

    on_click(doc, "btn-new", |_| {
        with_app(|app| app.new_game(app.level));
    });
    on_click(doc, "btn-mode", |_| {
        with_app(App::toggle_flag_mode);
    });
    on_click(doc, "btn-theme", |_| {
        with_app(|app| app.toggle_theme());
    });
    on_click(doc, "btn-help", |_| {
        with_app(|app| {
            let _ = dialog(&app.doc, "dlg-help").show_modal();
        });
    });
    on_click(doc, "btn-stats", |_| {
        with_app(|app| {
            app.render_stats();
            let _ = dialog(&app.doc, "dlg-stats").show_modal();
        });
    });
    on_click(doc, "stats-reset", |_| {
        with_app(|app| {
            if app
                .window
                .confirm_with_message("Reset all statistics and best times?")
                .unwrap_or(false)
            {
                for level in [Level::Easy, Level::Medium, Level::Hard] {
                    storage::remove(&format!("stats.{}", level.key()));
                }
                app.render_stats();
            }
        });
    });
    on_click(doc, "res-again", |_| {
        with_app(|app| app.new_game(app.level));
    });

    // Custom board form.
    for id in ["cust-rows", "cust-cols"] {
        listen(&by_id(doc, id), "input", |_| {
            with_app(|app| app.sync_custom_max());
        });
    }
    on_click(doc, "cust-cancel", |_| {
        with_app(|app| dialog(&app.doc, "dlg-custom").close());
    });
    listen(&by_id(doc, "cust-form"), "submit", |e| {
        e.prevent_default();
        with_app(|app| match app.read_custom() {
            Ok(config) => {
                app.custom = config;
                storage::set(
                    "custom",
                    &format!("{},{},{}", config.rows, config.cols, config.mines),
                );
                dialog(&app.doc, "dlg-custom").close();
                app.new_game(Level::Custom);
            }
            Err(msg) => set_text(&app.doc, "cust-error", &msg),
        });
    });

    // Clicking a dialog's backdrop closes it.
    if let Ok(dialogs) = doc.query_selector_all("dialog") {
        for k in 0..dialogs.length() {
            let Some(node) = dialogs.item(k) else {
                continue;
            };
            let dlg: HtmlDialogElement = node.unchecked_into();
            let target = dlg.clone();
            listen(&dlg, "click", move |e| {
                let on_backdrop = e
                    .target()
                    .is_some_and(|t| JsValue::from(t) == JsValue::from(target.clone()));
                if on_backdrop {
                    target.close();
                }
            });
        }
    }

    // Global shortcuts.
    listen(doc, "keydown", |e| {
        let Some(k) = e.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if k.ctrl_key() || k.meta_key() || k.alt_key() {
            return;
        }
        let typing = e
            .target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"));
        let key = k.key();
        with_app(|app| {
            if typing || any_dialog_open(&app.doc) {
                return;
            }
            match key.as_str() {
                "n" | "N" => app.new_game(app.level),
                "m" | "M" => app.toggle_flag_mode(),
                "?" => {
                    let _ = dialog(&app.doc, "dlg-help").show_modal();
                }
                _ => {}
            }
        });
    });

    // Hard flips orientation with the screen, but only before the first move.
    listen(window, "resize", |_| {
        with_app(|app| {
            let portrait = is_portrait(&app.window);
            if portrait != app.portrait {
                app.portrait = portrait;
                if app.level == Level::Hard && app.board.status() == Status::Ready {
                    app.new_game(Level::Hard);
                }
            }
        });
    });

    // Follow the OS theme live when the user has not picked one.
    if let Ok(Some(mq)) = window.match_media("(prefers-color-scheme: dark)") {
        listen(&mq, "change", |_| {
            with_app(|app| app.sync_theme_icon());
        });
    }
}
