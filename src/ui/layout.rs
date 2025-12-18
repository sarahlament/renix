use crate::app::{App, FocusedPanel};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use super::{main_panel, settings};

pub fn render(frame: &mut Frame, app: &App) {
    // Create 85/15 vertical split
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(85), Constraint::Percentage(15)])
        .split(frame.area());

    // Render main panel (top 85%)
    let main_focused = app.focused_panel == FocusedPanel::Main;
    main_panel::render(frame, app, chunks[0], main_focused);

    // Render settings panel (bottom 15%)
    settings::render(frame, app, chunks[1]);
}
