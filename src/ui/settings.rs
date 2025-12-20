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
            .as_deref()
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

    let extra_args = if app.edit_mode == EditMode::ExtraArgs {
        format!("{}_", app.edit_buffer)
    } else {
        app.get_selected_host()
            .and_then(|(name, _)| app.config.hosts.get(&name))
            .map(|h| {
                if h.extra_args.is_empty() {
                    "(none)".to_string()
                } else {
                    h.extra_args.join(" ")
                }
            })
            .unwrap_or_else(|| "(none)".to_string())
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

    let args_style = if app.edit_mode == EditMode::ExtraArgs {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let mut text = vec![
        Line::from(vec![
            Span::raw("flake: "),
            Span::styled(flake_path, flake_style),
            Span::raw(" "),
            Span::styled("[f]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("selected: "),
            Span::styled(selected_host, host_style),
            Span::raw(" "),
            Span::styled("[c]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::raw("extra args: "),
            Span::styled(extra_args, args_style),
            Span::raw(" "),
            Span::styled("[a]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
    ];

    if app.is_editing() {
        text.push(Line::from(Span::styled(
            "[enter] save | [esc] cancel",
            Style::default().fg(Color::Yellow),
        )));
    } else {
        text.push(Line::from(Span::styled(
            "[tab] switch | [f] flake | [c] connection | [a] args",
            Style::default().fg(Color::Gray),
        )));
    }

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .title(" settings ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    frame.render_widget(paragraph, area);
}
