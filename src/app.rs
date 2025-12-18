use crate::config::{Config, Connection};
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
    pub output_lines: Vec<String>,
    pub is_building: bool,
    pub show_verbose: bool,
    pub output_receiver: Option<mpsc::Receiver<String>>,
    pub edit_mode: EditMode,
    pub edit_buffer: String,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            focused_panel: FocusedPanel::Main,
            selected_host_idx: 0,
            selected_operation: RebuildOperation::Switch,
            output_lines: Vec::new(),
            is_building: false,
            show_verbose: false,
            output_receiver: None,
            edit_mode: EditMode::None,
            edit_buffer: String::new(),
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
    }

    /// Move selection down in host list
    pub fn select_next_host(&mut self) {
        let hosts = self.get_hosts();
        if self.selected_host_idx < hosts.len().saturating_sub(1) {
            self.selected_host_idx += 1;
        }
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
    }

    /// Cycle to previous rebuild operation
    pub fn prev_operation(&mut self) {
        self.selected_operation = self.selected_operation.prev();
    }

    /// Toggle verbose output display
    pub fn toggle_verbose(&mut self) {
        self.show_verbose = !self.show_verbose;
    }

    /// Start a rebuild for the currently selected host (async streaming version)
    pub async fn start_rebuild_async(&mut self) -> Result<()> {
        use crate::nix::RebuildCommand;

        if self.is_building {
            return Ok(()); // Already building
        }

        let (config_name, connection) = match self.get_selected_host() {
            Some(host) => host,
            None => return Ok(()), // No host selected
        };

        if !connection.is_configured() {
            self.output_lines
                .push("Error: Host is not configured".to_string());
            return Ok(());
        }

        self.is_building = true;
        self.output_lines.clear();
        self.output_lines.push(format!(
            "Starting {} for {} ({})...",
            self.selected_operation.as_str(),
            config_name,
            connection.display()
        ));

        let cmd = RebuildCommand::new(
            self.selected_operation,
            self.config.flake_path.clone(),
            config_name,
            connection,
            self.config.extra_args.clone(),
        );

        // Start async streaming
        let rx = cmd.execute_streaming().await?;
        self.output_receiver = Some(rx);

        Ok(())
    }

    /// Poll for new output from the rebuild process
    pub fn poll_output(&mut self) {
        if let Some(ref mut rx) = self.output_receiver {
            // Try to receive all available messages without blocking
            while let Ok(line) = rx.try_recv() {
                self.output_lines.push(line.clone());

                // Check if build finished
                if line.contains("Build completed successfully!")
                    || line.contains("Build failed with exit code")
                {
                    self.is_building = false;
                    self.output_receiver = None;
                    break;
                }
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
                if self.edit_buffer.is_empty() {
                    self.config.flake_path = None;
                } else {
                    self.config.flake_path = Some(self.edit_buffer.clone());
                }
                self.config.save()?;
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
}
