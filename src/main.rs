mod app;
mod config;
mod nix;
mod terminal;
mod ui;

use app::App;
use color_eyre::Result;
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use nix::{discover_configurations, flake::get_hostname};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    // Handle --version and --help flags
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                println!("renix {}", VERSION);
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                eprintln!("Unknown argument: {}", args[1]);
                eprintln!("Try 'renix --help' for more information.");
                std::process::exit(1);
            }
        }
    }

    // Setup color-eyre for better error messages
    color_eyre::install()?;

    // Load config (creates default if missing)
    let mut config = Config::load()?;

    // If flake path is set, discover configurations and merge
    if let Some(ref flake_path) = config.flake_path {
        if let Ok(discovered) = discover_configurations(flake_path) {
            if let Ok(hostname) = get_hostname() {
                config.merge_discovered_configs(discovered, &hostname)?;
                config.save()?;
            }
        }
    }

    // Create app state
    let mut app = App::new(config);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle any errors that occurred during run
    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn print_help() {
    println!("renix {} - NixOS Rebuild Manager TUI", VERSION);
    println!();
    println!("USAGE:");
    println!("    renix [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help       Print help information");
    println!("    -v, --version    Print version information");
    println!();
    println!("KEYBINDINGS:");
    println!("    q                Quit (press twice during build to force)");
    println!("    Tab              Toggle between main and settings panel");
    println!("    ↑/↓, j/k         Navigate hosts / scroll output");
    println!("    ←/→, h/l         Change rebuild operation");
    println!("    u                Toggle --upgrade flag");
    println!("    i                Enter input mode (for passwords)");
    println!("    Enter            Start rebuild");
    println!("    Esc              Cancel running build / Exit input mode");
    println!("    f                Edit flake path");
    println!("    c                Edit host connection");
    println!("    a                Edit extra args for host");
    println!("    PageUp/PageDown  Scroll output (10 lines)");
    println!("    Home/End         Jump to top/bottom of output");
    println!();
    println!("CONFIGURATION:");
    println!("    Config file: ~/.config/renix/config.toml");
    println!();
    println!("For more information, visit: https://github.com/sarahlament/renix");
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Resize virtual terminal to match output area FIRST
        // This ensures terminal_cols and terminal_rows are correct when starting builds
        let term_size = terminal.size()?;
        // Output area is 75% of width, minus 2 for borders
        let output_width = (term_size.width * 75 / 100).saturating_sub(2) as usize;
        let output_height = term_size.height.saturating_sub(2) as usize;
        app.resize_terminal(output_width, output_height);

        // Poll for output from async rebuild process
        app.poll_output();

        terminal.draw(|f| {
            ui::render(f, app);
        })?;

        // Handle events with timeout
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Mouse(mouse) => {
                    // Handle mouse scroll events
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            app.scroll_output_up();
                        }
                        MouseEventKind::ScrollDown => {
                            app.scroll_output_down();
                        }
                        _ => {}
                    }
                }
                Event::Key(key) => {
                    // Handle edit mode input
                    if app.is_editing() {
                        match key.code {
                            KeyCode::Enter => {
                                app.commit_edit()?;
                            }
                            KeyCode::Esc => {
                                app.cancel_edit();
                            }
                            KeyCode::Char(c) => {
                                app.edit_insert_char(c);
                            }
                            KeyCode::Backspace => {
                                app.edit_backspace();
                            }
                            _ => {}
                        }
                    } else if app.input_mode {
                        // Input mode - send keystrokes to PTY
                        match key.code {
                            KeyCode::Esc => {
                                app.input_mode = false;
                            }
                            KeyCode::Char(c) => {
                                app.send_input(vec![c as u8]);
                            }
                            KeyCode::Enter => {
                                app.send_input(vec![b'\n']);
                            }
                            KeyCode::Backspace => {
                                app.send_input(vec![0x7F]); // DEL character
                            }
                            _ => {}
                        }
                    } else {
                        // Normal mode input
                        match key.code {
                            KeyCode::Char('q') => {
                                if app.attempt_quit() {
                                    return Ok(());
                                }
                            }
                            KeyCode::Esc => {
                                app.cancel_build();
                            }
                            KeyCode::Tab => {
                                app.toggle_panel();
                            }
                            KeyCode::Char('f') => {
                                app.start_edit_flake_path();
                            }
                            KeyCode::Char('c') => {
                                app.start_edit_host_connection();
                            }
                            KeyCode::Char('a') => {
                                app.start_edit_extra_args();
                            }
                            KeyCode::Char('u') => {
                                app.toggle_upgrade();
                            }
                            KeyCode::Char('i') => {
                                app.toggle_input_mode();
                            }
                            KeyCode::Up => {
                                app.select_prev_host();
                            }
                            KeyCode::Down => {
                                app.select_next_host();
                            }
                            KeyCode::Char('k') => {
                                app.scroll_output_up();
                            }
                            KeyCode::Char('j') => {
                                app.scroll_output_down();
                            }
                            KeyCode::PageUp => {
                                // Page up - scroll by 10 lines
                                for _ in 0..10 {
                                    app.scroll_output_up();
                                }
                            }
                            KeyCode::PageDown => {
                                // Page down - scroll by 10 lines
                                for _ in 0..10 {
                                    app.scroll_output_down();
                                }
                            }
                            KeyCode::Home => {
                                // Jump to top of output
                                let total_lines = app.terminal.get_scrollback().len()
                                    + app.terminal.get_screen().len();
                                app.output_scroll = total_lines.saturating_sub(1);
                            }
                            KeyCode::End => {
                                // Jump to bottom of output
                                app.output_scroll = 0;
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                app.prev_operation();
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                app.next_operation();
                            }
                            KeyCode::Enter => {
                                app.start_rebuild_async().await?;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
