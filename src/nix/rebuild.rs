use color_eyre::Result;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::Write;
use tokio::sync::mpsc;

use crate::app::RebuildOperation;
use crate::config::Connection;

pub struct RebuildCommand {
    pub operation: RebuildOperation,
    pub flake_path: Option<String>,
    pub config_name: String,
    pub connection: Connection,
    pub extra_args: Vec<String>,
    pub pty_cols: u16,
    pub pty_rows: u16,
}

pub struct RebuildChannels {
    pub output_rx: mpsc::Receiver<Vec<u8>>,
    pub input_tx: mpsc::Sender<Vec<u8>>,
}

impl RebuildCommand {
    pub fn new(
        operation: RebuildOperation,
        flake_path: Option<String>,
        config_name: String,
        connection: Connection,
        extra_args: Vec<String>,
        pty_cols: u16,
        pty_rows: u16,
    ) -> Self {
        Self {
            operation,
            flake_path,
            config_name,
            connection,
            extra_args,
            pty_cols,
            pty_rows,
        }
    }

    /// Build the command arguments for nixos-rebuild
    fn build_args(&self) -> Vec<String> {
        let mut args = vec![self.operation.as_str().to_string()];

        // Add flake reference if available
        if let Some(ref flake_path) = self.flake_path {
            args.push("--flake".to_string());
            args.push(format!("{}#{}", flake_path, self.config_name));
        }

        // Add remote target if not local, and use appropriate sudo flag
        match &self.connection {
            Connection::Remote(addr) => {
                args.push("--target-host".to_string());
                args.push(addr.clone());
                args.push("--use-remote-sudo".to_string());
            }
            Connection::Local => {
                // Local rebuild with regular sudo
                args.push("--sudo".to_string());
            }
            Connection::Unconfigured => {
                // This shouldn't happen - unconfigured hosts shouldn't be rebuildable
            }
        }

        // Add extra args
        args.extend(self.extra_args.clone());

        args
    }

    /// Execute the rebuild command asynchronously with PTY support for interactive prompts
    /// Returns channels for both output (receiving) and input (sending)
    pub async fn execute_streaming(self) -> Result<RebuildChannels> {
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(100);
        let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(100);

        tokio::task::spawn_blocking(move || {
            let pty_system = NativePtySystem::default();

            // Create a PTY with the requested size
            let pty_pair = match pty_system.openpty(PtySize {
                rows: self.pty_rows,
                cols: self.pty_cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                Ok(pair) => pair,
                Err(e) => {
                    let msg = format!("Failed to create PTY: {}\n", e);
                    let _ = output_tx.blocking_send(msg.into_bytes());
                    return;
                }
            };

            // Set PTY to raw mode to disable line buffering
            #[cfg(unix)]
            {
                use nix::sys::termios::{self, LocalFlags};
                use std::os::unix::io::BorrowedFd;

                if let Some(raw_fd) = pty_pair.master.as_raw_fd() {
                    // SAFETY: We know the fd is valid as we just created the PTY
                    let fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
                    if let Ok(mut termios) = termios::tcgetattr(fd) {
                        termios.local_flags.remove(LocalFlags::ICANON);
                        termios.local_flags.remove(LocalFlags::ECHO);
                        termios.local_flags.remove(LocalFlags::ISIG);
                        let _ = termios::tcsetattr(fd, termios::SetArg::TCSANOW, &termios);
                    }
                }
            }

            // Build the command
            let args = self.build_args();
            let mut cmd = CommandBuilder::new("nixos-rebuild");
            for arg in args {
                cmd.arg(arg);
            }

            // Set TERM environment variable so programs know they're in a terminal
            cmd.env(
                "TERM",
                std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()),
            );

            // Spawn the command in the PTY
            let mut child = match pty_pair.slave.spawn_command(cmd) {
                Ok(child) => child,
                Err(e) => {
                    let msg = format!("Failed to spawn nixos-rebuild: {}\n", e);
                    let _ = output_tx.blocking_send(msg.into_bytes());
                    return;
                }
            };

            // Drop the slave end - only keep the master
            drop(pty_pair.slave);

            // Get the master reader and writer
            let mut reader = pty_pair.master.try_clone_reader().unwrap();
            let mut writer = pty_pair.master.take_writer().unwrap();

            // Spawn a thread to read from PTY and send to output channel
            let output_tx_clone = output_tx.clone();
            let reader_handle = std::thread::spawn(move || {
                use std::io::Read;
                let mut buffer = [0u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => {
                            if output_tx_clone.blocking_send(buffer[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // Handle input from the input channel and write to PTY
            let writer_handle = std::thread::spawn(move || {
                while let Some(data) = input_rx.blocking_recv() {
                    if writer.write_all(&data).is_err() {
                        break;
                    }
                    if writer.flush().is_err() {
                        break;
                    }
                }
            });

            // Wait for the child process to complete
            let exit_status = match child.wait() {
                Ok(status) => status,
                Err(e) => {
                    let msg = format!("\n✗ Process error: {}\n", e);
                    let _ = output_tx.blocking_send(msg.into_bytes());
                    return;
                }
            };

            // Send completion message
            if exit_status.success() {
                let _ = output_tx
                    .blocking_send(b"\n\xE2\x9C\x93 Build completed successfully!\n".to_vec());
            } else {
                let msg = format!(
                    "\n✗ Build failed with exit code: {:?}\n",
                    exit_status.exit_code()
                );
                let _ = output_tx.blocking_send(msg.into_bytes());
            }

            // Wait for threads to finish
            let _ = reader_handle.join();
            drop(writer_handle); // Input thread will exit when channel closes
        });

        Ok(RebuildChannels {
            output_rx,
            input_tx,
        })
    }
}
