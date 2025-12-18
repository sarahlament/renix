use crate::app::{App, EditMode, FocusedPanel};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::Settings;

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let flake_path = if app.edit_mode == EditMode::FlakePath {
        format!("{}_", app.edit_buffer)
    } else {
        app.config
            .flake_path
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(not set)")
            .to_string()
    };

    let selected_host = if app.edit_mode == EditMode::HostConnection {
        format!("{}_", app.edit_buffer)
    } else {
        app.get_selected_host()
            .map(|(name, conn)| format!("{} → {}", name, conn.display()))
            .unwrap_or_else(|| "(no host selected)".to_string())
    };

    let extra_args = if app.config.extra_args.is_empty() {
        "(none)".to_string()
    } else {
        app.config.extra_args.join(" ")
    };

    let flake_style = if app.edit_mode == EditMode::FlakePath {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let host_style = if app.edit_mode == EditMode::HostConnection {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let mut text = vec![
        Line::from(vec![
            Span::raw("Flake: "),
            Span::styled(flake_path, flake_style),
            Span::raw(" "),
            Span::styled("[f]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("Selected: "),
            Span::styled(selected_host, host_style),
            Span::raw(" "),
            Span::styled("[c]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("Extra args: "),
            Span::styled(extra_args, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
    ];

    if app.is_editing() {
        text.push(Line::from(Span::styled(
            "[Enter] save | [Esc] cancel",
            Style::default().fg(Color::Yellow),
        )));
    } else {
        text.push(Line::from(Span::styled(
            "[Tab] switch panels | [f] edit flake | [c] edit connection",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    frame.render_widget(paragraph, area);
}
