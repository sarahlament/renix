use crate::app::{App, FocusedPanel};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    // Split main panel into left (host list) and right (output)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_host_list(frame, app, chunks[0], focused);
    render_output_area(frame, app, chunks[1]);
}

fn render_host_list(frame: &mut Frame, app: &App, area: Rect, focused: bool) {
    let hosts = app.get_hosts();

    let items: Vec<ListItem> = hosts
        .iter()
        .enumerate()
        .map(|(idx, (name, connection))| {
            let conn_display = connection.display();
            let line = if connection.is_configured() {
                format!("{} ({})", name, conn_display)
            } else {
                format!("{} {}", name, conn_display)
            };

            let style = if idx == app.selected_host_idx && focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if !connection.is_configured() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let border_style = if focused && app.focused_panel == FocusedPanel::Main {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };

    let title = format!(" Hosts - {} ", app.selected_operation.as_str());
    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(list, area);
}

fn render_output_area(frame: &mut Frame, app: &App, area: Rect) {
    let output_text = if app.output_lines.is_empty() {
        if app.is_building {
            "Building...".to_string()
        } else {
            "No output yet. Select a host and press Enter to rebuild.".to_string()
        }
    } else if app.show_verbose {
        app.output_lines.join("\n")
    } else {
        // Show last 10 lines in compact mode
        app.output_lines
            .iter()
            .rev()
            .take(10)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    };

    let title = if app.show_verbose {
        " Output (verbose) [v to toggle] "
    } else {
        " Output (compact) [v to toggle] "
    };

    let output = Paragraph::new(output_text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .scroll((0, 0));

    frame.render_widget(output, area);
}
