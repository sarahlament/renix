use crate::config::{Config, Connection};
use crate::terminal::VirtualTerminal;
use color_eyre::Result;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPanel {
    Main,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditMode {
    None,
    FlakePath,
    HostConnection,
    ExtraArgs,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RebuildOperation {
    Switch,
    Boot,
    Test,
    Build,
    DryBuild,
    DryActivate,
}

impl RebuildOperation {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Switch => "switch",
            Self::Boot => "boot",
            Self::Test => "test",
            Self::Build => "build",
            Self::DryBuild => "dry-build",
            Self::DryActivate => "dry-activate",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Switch,
            Self::Boot,
            Self::Test,
            Self::Build,
            Self::DryBuild,
            Self::DryActivate,
        ]
    }

    pub fn next(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|op| op == self).unwrap();
        all[(idx + 1) % all.len()]
    }

    pub fn prev(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|op| op == self).unwrap();
        all[(idx + all.len() - 1) % all.len()]
    }
}

pub struct App {
    pub config: Config,
    pub focused_panel: FocusedPanel,
    pub selected_host_idx: usize,
    pub selected_operation: RebuildOperation,
    pub terminal: VirtualTerminal,
    pub is_building: bool,
    pub output_receiver: Option<mpsc::Receiver<Vec<u8>>>,
    pub input_sender: Option<mpsc::Sender<Vec<u8>>>,
    pub input_mode: bool,
    pub edit_mode: EditMode,
    pub edit_buffer: String,
    pub output_scroll: usize,
    pub use_upgrade: bool,
    pub quit_warned: bool,
    pub terminal_cols: u16,
    pub terminal_rows: u16,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            focused_panel: FocusedPanel::Main,
            selected_host_idx: 0,
            selected_operation: RebuildOperation::Switch,
            terminal: VirtualTerminal::new(200, 100), // Initial size, will be resized on first render
            is_building: false,
            output_receiver: None,
            input_sender: None,
            input_mode: false,
            edit_mode: EditMode::None,
            edit_buffer: String::new(),
            output_scroll: 0,
            use_upgrade: false,
            quit_warned: false,
            terminal_cols: 80,
            terminal_rows: 24,
        }
    }

    /// Resize the terminal to match the output area
    pub fn resize_terminal(&mut self, width: usize, height: usize) {
        self.terminal.resize(width, height);
        self.terminal_cols = width as u16;
        self.terminal_rows = height as u16;
    }

    /// Scroll output up
    pub fn scroll_output_up(&mut self) {
        let total_lines = self.terminal.get_scrollback().len() + self.terminal.get_screen().len();
        let max_scroll = total_lines.saturating_sub(1);
        if self.output_scroll < max_scroll {
            self.output_scroll = self.output_scroll.saturating_add(1);
        }
    }

    /// Scroll output down
    pub fn scroll_output_down(&mut self) {
        if self.output_scroll > 0 {
            self.output_scroll -= 1;
        }
    }

    /// Get list of hosts as (name, connection) tuples, sorted by name
    pub fn get_hosts(&self) -> Vec<(String, Connection)> {
        let mut hosts: Vec<_> = self
            .config
            .hosts
            .iter()
            .map(|(name, config)| (name.clone(), config.connection.clone()))
            .collect();
        hosts.sort_by(|a, b| a.0.cmp(&b.0));
        hosts
    }

    /// Get currently selected host name and connection
    pub fn get_selected_host(&self) -> Option<(String, Connection)> {
        let hosts = self.get_hosts();
        hosts.get(self.selected_host_idx).cloned()
    }

    /// Move selection up in host list
    pub fn select_prev_host(&mut self) {
        if self.selected_host_idx > 0 {
            self.selected_host_idx -= 1;
        }
        self.quit_warned = false;
    }

    /// Move selection down in host list
    pub fn select_next_host(&mut self) {
        let hosts = self.get_hosts();
        if self.selected_host_idx < hosts.len().saturating_sub(1) {
            self.selected_host_idx += 1;
        }
        self.quit_warned = false;
    }

    /// Toggle focus between panels
    pub fn toggle_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            FocusedPanel::Main => FocusedPanel::Settings,
            FocusedPanel::Settings => FocusedPanel::Main,
        };
    }

    /// Cycle to next rebuild operation
    pub fn next_operation(&mut self) {
        self.selected_operation = self.selected_operation.next();
        self.quit_warned = false;
    }

    /// Cycle to previous rebuild operation
    pub fn prev_operation(&mut self) {
        self.selected_operation = self.selected_operation.prev();
        self.quit_warned = false;
    }

    /// Toggle the --upgrade flag
    pub fn toggle_upgrade(&mut self) {
        self.use_upgrade = !self.use_upgrade;
        self.quit_warned = false;
    }

    /// Toggle input mode for PTY
    pub fn toggle_input_mode(&mut self) {
        if self.is_building && self.input_sender.is_some() {
            self.input_mode = !self.input_mode;
        }
    }

    /// Send input to the PTY
    pub fn send_input(&mut self, data: Vec<u8>) {
        if let Some(ref tx) = self.input_sender {
            let _ = tx.try_send(data);
        }
    }

    /// Cancel the current build
    pub fn cancel_build(&mut self) {
        if self.is_building {
            self.is_building = false;
            self.output_receiver = None;
            self.input_sender = None;
            self.input_mode = false;
            let msg = "\n✓ Build cancelled by user\n";
            self.terminal.feed_bytes(msg.as_bytes());
            self.quit_warned = false;
        }
    }

    /// Attempt to quit - returns true if should quit, false if should warn
    pub fn attempt_quit(&mut self) -> bool {
        if self.quit_warned {
            // Second press - quit (and cancel build if running)
            if self.is_building {
                self.cancel_build();
            }
            true
        } else {
            // First press - warn user
            self.quit_warned = true;
            if self.is_building {
                let msg = "\n⚠ Build in progress! Press 'q' again to cancel and quit, or Esc to cancel build.\n";
                self.terminal.feed_bytes(msg.as_bytes());
            }
            false
        }
    }

    /// Start a rebuild for the currently selected host (async streaming version)
    pub async fn start_rebuild_async(&mut self) -> Result<()> {
        self.quit_warned = false;
        use crate::nix::RebuildCommand;

        if self.is_building {
            return Ok(()); // Already building
        }

        let (config_name, connection) = match self.get_selected_host() {
            Some(host) => host,
            None => return Ok(()), // No host selected
        };

        if !connection.is_configured() {
            self.terminal.feed_bytes(b"Error: Host is not configured\n");
            return Ok(());
        }

        self.is_building = true;
        self.output_scroll = 0; // Reset scroll when starting new build
        self.terminal.clear(); // Clear previous build output

        // Write initial message to terminal
        let msg = format!(
            "Starting {} for {} ({}){} ...\n",
            self.selected_operation.as_str(),
            config_name,
            connection.display(),
            if self.use_upgrade {
                " with --upgrade"
            } else {
                ""
            }
        );
        self.terminal.feed_bytes(msg.as_bytes());

        // Get extra args for this host
        let mut extra_args = self
            .config
            .hosts
            .get(&config_name)
            .map(|h| h.extra_args.clone())
            .unwrap_or_default();

        // Add --upgrade if enabled
        if self.use_upgrade {
            extra_args.push("--upgrade".to_string());
        }

        let cmd = RebuildCommand::new(
            self.selected_operation,
            self.config.flake_path.clone(),
            config_name,
            connection,
            extra_args,
            self.terminal_cols,
            self.terminal_rows,
        );

        // Start async streaming with PTY
        let channels = cmd.execute_streaming().await?;
        self.output_receiver = Some(channels.output_rx);
        self.input_sender = Some(channels.input_tx);

        Ok(())
    }

    /// Poll for new output from the rebuild process
    pub fn poll_output(&mut self) {
        if let Some(ref mut rx) = self.output_receiver {
            let mut bytes_received = false;
            // Try to receive all available messages without blocking
            while let Ok(bytes) = rx.try_recv() {
                // Feed bytes to terminal
                self.terminal.feed_bytes(&bytes);
                bytes_received = true;

                // Check if build finished (simple byte pattern matching)
                let text = String::from_utf8_lossy(&bytes);
                if text.contains("Build completed successfully!")
                    || text.contains("Build failed with exit code")
                    || text.contains("Process error:")
                {
                    self.is_building = false;
                    self.output_receiver = None;
                    self.input_sender = None;
                    self.input_mode = false;
                    self.quit_warned = false;
                    break;
                }
            }

            // Terminal handles scrollback internally, scroll position stays relative
            if bytes_received && self.output_scroll > 0 {
                // Keep scroll position stable - terminal manages this internally
            }
        }
    }

    /// Start editing flake path
    pub fn start_edit_flake_path(&mut self) {
        self.edit_mode = EditMode::FlakePath;
        self.edit_buffer = self.config.flake_path.clone().unwrap_or_default();
    }

    /// Start editing host connection
    pub fn start_edit_host_connection(&mut self) {
        if let Some((_, conn)) = self.get_selected_host() {
            self.edit_mode = EditMode::HostConnection;
            self.edit_buffer = match conn {
                Connection::Local => "localhost".to_string(),
                Connection::Remote(addr) => addr,
                Connection::Unconfigured => String::new(),
            };
        }
    }

    /// Start editing extra args for selected host
    pub fn start_edit_extra_args(&mut self) {
        if let Some((host_name, _)) = self.get_selected_host() {
            self.edit_mode = EditMode::ExtraArgs;
            if let Some(host_config) = self.config.hosts.get(&host_name) {
                self.edit_buffer = host_config.extra_args.join(" ");
            }
        }
    }

    /// Handle character input during edit mode
    pub fn edit_insert_char(&mut self, c: char) {
        self.edit_buffer.push(c);
    }

    /// Handle backspace during edit mode
    pub fn edit_backspace(&mut self) {
        self.edit_buffer.pop();
    }

    /// Cancel edit mode
    pub fn cancel_edit(&mut self) {
        self.edit_mode = EditMode::None;
        self.edit_buffer.clear();
    }

    /// Commit the current edit
    pub fn commit_edit(&mut self) -> Result<()> {
        match self.edit_mode {
            EditMode::FlakePath => {
                let flake_changed = self.config.flake_path.as_deref()
                    != Some(self.edit_buffer.as_str());

                if self.edit_buffer.is_empty() {
                    self.config.flake_path = None;
                } else {
                    self.config.flake_path = Some(self.edit_buffer.clone());
                }
                self.config.save()?;

                // Rediscover configs if flake path changed
                if flake_changed {
                    self.refresh_flake_configs()?;
                }
            }
            EditMode::HostConnection => {
                if let Some((host_name, _)) = self.get_selected_host() {
                    let new_connection = if self.edit_buffer.is_empty() {
                        Connection::Unconfigured
                    } else if self.edit_buffer == "localhost" {
                        Connection::Local
                    } else {
                        Connection::Remote(self.edit_buffer.clone())
                    };

                    if let Some(host_config) = self.config.hosts.get_mut(&host_name) {
                        host_config.connection = new_connection;
                    }
                    self.config.save()?;
                }
            }
            EditMode::ExtraArgs => {
                if let Some((host_name, _)) = self.get_selected_host() {
                    let new_args = if self.edit_buffer.trim().is_empty() {
                        Vec::new()
                    } else {
                        self.edit_buffer
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()
                    };

                    if let Some(host_config) = self.config.hosts.get_mut(&host_name) {
                        host_config.extra_args = new_args;
                    }
                    self.config.save()?;
                }
            }
            EditMode::None => {}
        }

        self.edit_mode = EditMode::None;
        self.edit_buffer.clear();
        Ok(())
    }

    /// Check if currently in edit mode
    pub fn is_editing(&self) -> bool {
        self.edit_mode != EditMode::None
    }

    /// Refresh flake configurations (discover and merge with existing config)
    pub fn refresh_flake_configs(&mut self) -> Result<()> {
        use crate::nix::{discover_configurations, flake::get_hostname};

        if let Some(ref flake_path) = self.config.flake_path {
            if let Ok(discovered) = discover_configurations(flake_path) {
                if let Ok(hostname) = get_hostname() {
                    self.config
                        .merge_discovered_configs(discovered, &hostname)?;
                    self.config.save()?;
                }
            }
        }
        Ok(())
    }
}
