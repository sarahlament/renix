use color_eyre::{eyre::Context, Result};
use std::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

use crate::app::RebuildOperation;
use crate::config::Connection;

pub struct RebuildCommand {
    pub operation: RebuildOperation,
    pub flake_path: Option<String>,
    pub config_name: String,
    pub connection: Connection,
    pub extra_args: Vec<String>,
}

impl RebuildCommand {
    pub fn new(
        operation: RebuildOperation,
        flake_path: Option<String>,
        config_name: String,
        connection: Connection,
        extra_args: Vec<String>,
    ) -> Self {
        Self {
            operation,
            flake_path,
            config_name,
            connection,
            extra_args,
        }
    }

    /// Build the nixos-rebuild command
    fn build_command(&self) -> Command {
        let mut cmd = Command::new("nixos-rebuild");

        // Add operation
        cmd.arg(self.operation.as_str());

        // Add flake reference if available
        if let Some(ref flake_path) = self.flake_path {
            cmd.arg("--flake");
            cmd.arg(format!("{}#{}", flake_path, self.config_name));
        }

        // Add remote target if not local
        match &self.connection {
            Connection::Remote(addr) => {
                cmd.arg("--target-host");
                cmd.arg(addr);
                cmd.arg("--use-remote-sudo");
            }
            Connection::Local => {
                // Local rebuild, no extra args needed
            }
            Connection::Unconfigured => {
                // This shouldn't happen - unconfigured hosts shouldn't be rebuildable
            }
        }

        // Add extra args
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        cmd
    }

    /// Execute the rebuild command synchronously (blocking)
    pub fn execute_blocking(&self) -> Result<String> {
        let mut cmd = self.build_command();

        let output = cmd.output().wrap_err("Failed to execute nixos-rebuild")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let combined = format!("{}\n{}", stdout, stderr);

        if !output.status.success() {
            return Err(color_eyre::eyre::eyre!(
                "nixos-rebuild failed with exit code {:?}\n{}",
                output.status.code(),
                combined
            ));
        }

        Ok(combined)
    }

    /// Build a tokio command for async execution
    fn build_tokio_command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new("nixos-rebuild");

        // Add operation
        cmd.arg(self.operation.as_str());

        // Add flake reference if available
        if let Some(ref flake_path) = self.flake_path {
            cmd.arg("--flake");
            cmd.arg(format!("{}#{}", flake_path, self.config_name));
        }

        // Add remote target if not local
        match &self.connection {
            Connection::Remote(addr) => {
                cmd.arg("--target-host");
                cmd.arg(addr);
                cmd.arg("--use-remote-sudo");
            }
            Connection::Local => {
                // Local rebuild, no extra args needed
            }
            Connection::Unconfigured => {
                // This shouldn't happen
            }
        }

        // Add extra args
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        // Capture stdout and stderr
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        cmd
    }

    /// Execute the rebuild command asynchronously, streaming output to a channel
    /// Returns a channel receiver that will receive output lines
    pub async fn execute_streaming(self) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut cmd = self.build_tokio_command();

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let _ = tx
                        .send(format!("Failed to spawn nixos-rebuild: {}", e))
                        .await;
                    return;
                }
            };

            let stdout = child.stdout.take().expect("Failed to capture stdout");
            let stderr = child.stderr.take().expect("Failed to capture stderr");

            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let tx_stdout = tx.clone();
            let tx_stderr = tx.clone();

            // Spawn tasks to read stdout and stderr concurrently
            let stdout_task = tokio::spawn(async move {
                let mut lines = stdout_reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stdout.send(line).await.is_err() {
                        break;
                    }
                }
            });

            let stderr_task = tokio::spawn(async move {
                let mut lines = stderr_reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stderr.send(format!("stderr: {}", line)).await.is_err() {
                        break;
                    }
                }
            });

            // Wait for both readers to finish
            let _ = tokio::join!(stdout_task, stderr_task);

            // Wait for the process to finish
            match child.wait().await {
                Ok(status) => {
                    if status.success() {
                        let _ = tx.send("Build completed successfully!".to_string()).await;
                    } else {
                        let _ = tx
                            .send(format!("Build failed with exit code: {:?}", status.code()))
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("Failed to wait for process: {}", e)).await;
                }
            }
        });

        Ok(rx)
    }
}
