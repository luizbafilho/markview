use std::time::{Duration, Instant};

use ratatui::{
    crossterm::event::{MouseButton, MouseEvent, MouseEventKind},
    layout::{Position, Rect},
};

use crate::{
    scrollbar,
    selection::{Point, Row, Selection},
};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);
const WHEEL_LINES: usize = 3;

/// Where things are on screen for the frame the event arrived on.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Presses here start a selection; the padding around the text included.
    pub body: Rect,
    pub text: Rect,
    pub bar: Option<Rect>,
    pub scroll: usize,
    pub max_scroll: usize,
}

#[derive(Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    Scrollbar,
    Select,
}

#[derive(Debug)]
pub struct Outcome {
    pub scroll: usize,
    pub copied: Option<String>,
}

#[derive(Debug, Default)]
pub struct Pointer {
    gesture: Gesture,
    selection: Option<Selection>,
    last_click: Option<(Instant, Point)>,
}

impl Pointer {
    pub const fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn handle(&mut self, m: MouseEvent, view: View, rows: &[Row], now: Instant) -> Outcome {
        let at = Position::new(m.column, m.row);
        let mut scroll = view.scroll;
        let mut copied = None;
        match (m.kind, &self.gesture) {
            (MouseEventKind::ScrollDown, _) => {
                scroll = (scroll + WHEEL_LINES).min(view.max_scroll);
            }
            (MouseEventKind::ScrollUp, _) => scroll = scroll.saturating_sub(WHEEL_LINES),
            (MouseEventKind::Down(MouseButton::Left), _) => {
                if let Some(track) = view.bar.filter(|b| b.contains(at)) {
                    self.gesture = Gesture::Scrollbar;
                    scroll = scrollbar::scroll_at(m.row, track, view.max_scroll);
                } else if view.body.contains(at) {
                    let p = point(view.text, scroll, at);
                    let double = self
                        .last_click
                        .is_some_and(|(t, q)| q == p && now.duration_since(t) < DOUBLE_CLICK);
                    if double {
                        self.selection = Selection::word(rows, p);
                        copied = self.selection.map(|s| s.text(rows));
                        self.gesture = Gesture::Idle;
                        self.last_click = None;
                    } else {
                        self.selection = Some(Selection::at(p));
                        self.gesture = Gesture::Select;
                        self.last_click = Some((now, p));
                    }
                } else {
                    self.clear();
                }
            }
            (MouseEventKind::Drag(MouseButton::Left), Gesture::Scrollbar) => {
                if let Some(track) = view.bar {
                    scroll = scrollbar::scroll_at(m.row, track, view.max_scroll);
                }
            }
            (MouseEventKind::Drag(MouseButton::Left), Gesture::Select) => {
                if m.row < view.text.y {
                    scroll = scroll.saturating_sub(1);
                } else if m.row >= view.text.bottom() {
                    scroll = (scroll + 1).min(view.max_scroll);
                }
                if let Some(s) = &mut self.selection {
                    s.extend(point(view.text, scroll, at));
                }
            }
            (MouseEventKind::Up(MouseButton::Left), Gesture::Select) => {
                self.gesture = Gesture::Idle;
                match self.selection {
                    Some(s) if !s.is_empty() => copied = Some(s.text(rows)),
                    _ => self.selection = None,
                }
            }
            (MouseEventKind::Up(MouseButton::Left), _) => self.gesture = Gesture::Idle,
            _ => {}
        }
        Outcome { scroll, copied }
    }
}

/// The document cell under `at`, clamped into the text area.
fn point(text: Rect, scroll: usize, at: Position) -> Point {
    let y = at.y.min(text.bottom().saturating_sub(1)).max(text.y);
    let x = at.x.min(text.right().saturating_sub(1)).max(text.x);
    Point {
        line: scroll + usize::from(y - text.y),
        col: usize::from(x - text.x),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;

    const VIEW: View = View {
        body: Rect::new(0, 0, 20, 6),
        text: Rect::new(2, 1, 16, 5),
        bar: Some(Rect::new(19, 1, 1, 5)),
        scroll: 0,
        max_scroll: 10,
    };

    fn rows() -> Vec<Row> {
        (0..15)
            .map(|i| Row::Plain(format!("line {i} words")))
            .collect()
    }

    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    const DOWN: MouseEventKind = MouseEventKind::Down(MouseButton::Left);
    const DRAG: MouseEventKind = MouseEventKind::Drag(MouseButton::Left);
    const UP: MouseEventKind = MouseEventKind::Up(MouseButton::Left);

    struct Session {
        pointer: Pointer,
        view: View,
        rows: Vec<Row>,
        now: Instant,
    }

    impl Session {
        fn new() -> Self {
            Self {
                pointer: Pointer::default(),
                view: VIEW,
                rows: rows(),
                now: Instant::now(),
            }
        }

        fn send(&mut self, kind: MouseEventKind, column: u16, row: u16) -> Option<String> {
            let out =
                self.pointer
                    .handle(event(kind, column, row), self.view, &self.rows, self.now);
            self.view.scroll = out.scroll;
            out.copied
        }
    }

    #[test]
    fn dragging_over_text_copies_it_on_release() {
        let mut s = Session::new();
        assert_eq!(s.send(DOWN, 2, 1), None);
        assert_eq!(s.send(DRAG, 7, 2), None);
        assert_eq!(s.send(UP, 7, 2).as_deref(), Some("line 0 words\nline 1"));
        assert!(
            s.pointer.selection().is_some(),
            "highlight stays after release"
        );
    }

    #[test]
    fn a_click_without_a_drag_clears_the_selection_and_copies_nothing() {
        let mut s = Session::new();
        s.send(DOWN, 2, 1);
        s.send(DRAG, 7, 2);
        s.send(UP, 7, 2);
        s.now += Duration::from_secs(1);
        s.send(DOWN, 4, 3);
        assert_eq!(s.send(UP, 4, 3), None);
        assert_eq!(s.pointer.selection(), None);
    }

    #[test]
    fn a_press_on_the_scrollbar_scrolls_instead_of_selecting() {
        let mut s = Session::new();
        s.send(DOWN, 19, 5);
        assert_eq!(s.view.scroll, 10);
        s.send(DRAG, 3, 1);
        assert_eq!(s.view.scroll, 0, "the drag follows the pointer off the bar");
        assert_eq!(s.send(UP, 3, 1), None);
        assert_eq!(s.pointer.selection(), None);
    }

    #[test]
    fn dragging_past_the_bottom_scrolls_and_keeps_extending() {
        let mut s = Session::new();
        s.send(DOWN, 2, 1);
        s.send(DRAG, 12, 9);
        s.send(DRAG, 12, 9);
        assert_eq!(s.view.scroll, 2);
        let copied = s.send(UP, 12, 9).unwrap_or_default();
        assert!(copied.starts_with("line 0 words\n"), "{copied}");
        assert!(copied.ends_with("\nline 6 word"), "{copied}");
    }

    #[test]
    fn double_click_copies_the_word() {
        let mut s = Session::new();
        s.send(DOWN, 10, 2);
        s.send(UP, 10, 2);
        s.now += Duration::from_millis(150);
        assert_eq!(s.send(DOWN, 10, 2).as_deref(), Some("words"));
    }

    #[test]
    fn slow_second_click_is_not_a_double_click() {
        let mut s = Session::new();
        s.send(DOWN, 10, 2);
        s.send(UP, 10, 2);
        s.now += Duration::from_secs(1);
        assert_eq!(s.send(DOWN, 10, 2), None);
    }

    #[test]
    fn the_selection_stays_on_its_lines_while_scrolling() {
        let mut s = Session::new();
        s.send(DOWN, 2, 1);
        s.send(DRAG, 7, 1);
        s.send(UP, 7, 1);
        s.send(MouseEventKind::ScrollDown, 5, 3);
        assert_eq!(s.view.scroll, 3);
        assert_eq!(
            s.pointer.selection().and_then(|sel| sel.cols(0)),
            Some(0..6)
        );
    }
}
