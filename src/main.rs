mod app;
mod config;
mod nix;
mod ui;

use app::App;
use color_eyre::Result;
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use nix::{discover_configurations, flake::get_hostname};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Handle any errors that occurred during run
    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Poll for output from async rebuild process
        app.poll_output();

        terminal.draw(|f| {
            ui::render(f, app);
        })?;

        // Handle events with timeout
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
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
                } else {
                    // Normal mode input
                    match key.code {
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        KeyCode::Tab => {
                            app.toggle_panel();
                        }
                        KeyCode::Char('v') => {
                            app.toggle_verbose();
                        }
                        KeyCode::Char('f') => {
                            app.start_edit_flake_path();
                        }
                        KeyCode::Char('c') => {
                            app.start_edit_host_connection();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.select_prev_host();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.select_next_host();
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
        }
    }
}
