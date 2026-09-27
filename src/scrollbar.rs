use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget},
};
use ratatui_markdown::theme::RichTextTheme;

use crate::theme::Theme;

/// `None` when the whole document fits in one page.
pub fn state(total: usize, page: usize, scroll: usize) -> Option<ScrollbarState> {
    // ratatui puts the thumb at the bottom when position == content_length - 1,
    // and the furthest scroll is total - page.
    (total > page).then(|| {
        ScrollbarState::new(total - page + 1)
            .position(scroll)
            .viewport_content_length(page)
    })
}

pub fn draw(buf: &mut Buffer, area: Rect, state: &mut ScrollbarState, theme: &Theme) {
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some(ratatui::symbols::line::VERTICAL))
        .track_style(Style::new().fg(theme.get_muted_text_color()))
        .thumb_style(Style::new().fg(theme.get_text_color()))
        .render(area, buf, state);
}

/// The scroll offset for a click or drag at `row`: the track's top row is the
/// start of the document and its bottom row the end.
pub fn scroll_at(row: u16, track: Rect, max_scroll: usize) -> usize {
    let last = usize::from(track.height.saturating_sub(1));
    if last == 0 {
        return 0;
    }
    let offset = usize::from(row.saturating_sub(track.y)).min(last);
    offset * max_scroll / last
}

#[cfg(test)]
mod tests {
    use terminal_colorsaurus::ThemeMode;

    use super::*;

    fn thumb_rows(total: usize, page: usize, scroll: usize) -> Vec<u16> {
        let area = Rect::new(0, 0, 1, page as u16);
        let mut buf = Buffer::empty(area);
        let mut state = state(total, page, scroll).unwrap();
        draw(&mut buf, area, &mut state, &Theme::new(ThemeMode::Dark));
        (0..area.height)
            .filter(|&y| buf[(0, y)].symbol() == "█")
            .collect()
    }

    #[test]
    fn thumb_spans_top_to_bottom_of_the_track() {
        let (total, page) = (100, 10);
        assert_eq!(thumb_rows(total, page, 0).first(), Some(&0));
        assert_eq!(
            thumb_rows(total, page, total - page).last(),
            Some(&(page as u16 - 1))
        );
        assert!(!thumb_rows(total, page, total - page).contains(&0));
    }

    #[test]
    fn no_scrollbar_when_the_document_fits() {
        assert!(state(10, 10, 0).is_none());
        assert!(state(3, 10, 0).is_none());
    }

    #[test]
    fn clicking_the_track_maps_its_ends_to_the_ends_of_the_document() {
        let track = Rect::new(170, 1, 1, 41);
        let max = 90;
        assert_eq!(scroll_at(1, track, max), 0);
        assert_eq!(scroll_at(21, track, max), 45);
        assert_eq!(scroll_at(41, track, max), max);
        assert_eq!(scroll_at(0, track, max), 0, "dragged above the track");
        assert_eq!(scroll_at(60, track, max), max, "dragged below the track");
    }
}
