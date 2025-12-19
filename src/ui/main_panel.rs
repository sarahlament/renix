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
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
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
            let prefix = if idx == app.selected_host_idx {
                "> "
            } else {
                "  "
            };
            let line = if connection.is_configured() {
                format!("{}{} ({})", prefix, name, conn_display)
            } else {
                format!("{}{} {}", prefix, name, conn_display)
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

    let title = format!(
        " hosts - {}{} ",
        app.selected_operation.as_str(),
        if app.use_upgrade { " --upgrade" } else { "" }
    );
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
    // Resize terminal to match output area (minus borders)
    let term_width = area.width.saturating_sub(2) as usize;
    let term_height = area.height.saturating_sub(2) as usize;

    // Convert terminal cells to ratatui Lines
    let scrollback = app.terminal.get_scrollback();
    let screen = app.terminal.get_screen();

    let mut lines: Vec<Line> = Vec::new();

    // Add scrollback
    for row in scrollback {
        lines.push(cells_to_line(row));
    }

    // Add current screen
    for row in screen {
        lines.push(cells_to_line(row));
    }

    // If empty, show placeholder
    if lines.is_empty() {
        if app.is_building {
            lines.push(Line::from("building..."));
        } else {
            lines.push(Line::from(
                "no output yet. select a host and press enter to rebuild.",
            ));
        }
    }

    // Show scroll position in title if scrolled, or building status
    let title = if app.input_mode {
        " output [INPUT MODE - Type password, Esc to exit] ".to_string()
    } else if app.is_building {
        " output [building... | press 'i' for input mode] ".to_string()
    } else if app.output_scroll > 0 {
        format!(
            " output [j/k:scroll | ↑{} lines | End:live] ",
            app.output_scroll
        )
    } else {
        " output [j/k:scroll | h/l:operation | u:upgrade | enter:rebuild] ".to_string()
    };

    let border_color = if app.input_mode {
        Color::Yellow
    } else {
        Color::Gray
    };

    // Render block first
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    frame.render_widget(block, area);

    // Get inner area (inside borders)
    let inner_area = area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 1 });

    // Trim trailing empty lines to avoid showing blank space at bottom
    while let Some(last_line) = lines.last() {
        if last_line.spans.is_empty() ||
           last_line.spans.iter().all(|span| span.content.trim().is_empty()) {
            lines.pop();
        } else {
            break;
        }
    }

    // Calculate which lines to show based on scroll position
    let total_lines = lines.len();
    let visible_height = inner_area.height as usize;

    let visible_lines = if total_lines <= visible_height {
        // All lines fit, show everything
        lines
    } else {
        // Need to scroll - calculate which lines to show
        let max_scroll = total_lines.saturating_sub(visible_height);
        let clamped_scroll = app.output_scroll.min(max_scroll);

        let start_line = if clamped_scroll == 0 {
            // Showing live view (bottom)
            total_lines.saturating_sub(visible_height)
        } else {
            // Scrolled up
            max_scroll.saturating_sub(clamped_scroll)
        };
        let end_line = start_line + visible_height;
        lines[start_line..end_line.min(total_lines)].to_vec()
    };

    // Render paragraph without scroll (we've already sliced the lines)
    let output = Paragraph::new(visible_lines);
    frame.render_widget(output, inner_area);
}

fn cells_to_line(cells: &[crate::terminal::Cell]) -> Line {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();

    for cell in cells {
        let mut new_style = Style::default();

        if let Some(fg) = cell.fg {
            new_style = new_style.fg(ansi_to_color(fg));
        }
        if let Some(bg) = cell.bg {
            new_style = new_style.bg(ansi_to_color(bg));
        }
        if cell.bold {
            new_style = new_style.add_modifier(Modifier::BOLD);
        }

        // If style changed, flush current span
        if new_style != current_style && !current_text.is_empty() {
            spans.push(Span::styled(current_text.clone(), current_style));
            current_text.clear();
        }

        current_style = new_style;
        current_text.push(cell.ch);
    }

    // Flush remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    Line::from(spans)
}

fn ansi_to_color(code: u8) -> Color {
    match code {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        _ => Color::Reset,
    }
}
