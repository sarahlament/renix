# renix

A terminal user interface (TUI) for managing NixOS rebuilds across multiple hosts.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Multi-host management**: Manage local and remote NixOS systems from one interface
- **Live output streaming**: Watch rebuild progress in real-time with scrollable output
- **Interactive prompts**: PTY-based terminal with input mode for sudo passwords and interactive commands
- **Operation switching**: Easily switch between switch, boot, test, build, dry-build, and dry-activate
- **Flake support**: Automatic discovery of NixOS configurations from flakes
- **Configurable**: Per-host connection settings and extra arguments
- **Safe operations**: Confirmation prompts and build cancellation support
- **Keyboard-driven**: Vim-style navigation and intuitive keybindings

## Installation

### Using NixOS Module (Recommended)

Add renix as a flake input:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    renix.url = "github:sarahlament/renix";
  };

  outputs = { nixpkgs, renix, ... }: {
    nixosConfigurations.your-host = nixpkgs.lib.nixosSystem {
      modules = [
        renix.nixosModules.default
        {
          programs.renix.enable = true;
        }
      ];
    };
  };
}
```

### Using Overlay

```nix
{
  inputs.renix.url = "github:sarahlament/renix";

  outputs = { nixpkgs, renix, ... }: {
    nixosConfigurations.your-host = nixpkgs.lib.nixosSystem {
      modules = [
        {
          nixpkgs.overlays = [ renix.overlays.default ];
          environment.systemPackages = [ pkgs.renix ];
        }
      ];
    };
  };
}
```

### Direct Installation

```bash
nix profile install github:sarahlament/renix
```

Or run without installing:

```bash
nix run github:sarahlament/renix
```

## Quick Start

1. **Launch renix**: Run `renix` in your terminal

2. **Set flake path** (first time):
   - Press `f` to edit flake path
   - Enter the path to your NixOS flake (e.g., `/etc/nixos` or `~/nixos-config`)
   - Press Enter to save

3. **Configure hosts**:
   - Select a host with `↑`/`↓` or `j`/`k`
   - Press `c` to set connection:
     - `localhost` for local system
     - `user@hostname` for SSH remote
   - Press `a` to add extra nixos-rebuild arguments

4. **Rebuild**:
   - Switch operations with `h`/`l` or `←`/`→`
   - Toggle `--upgrade` flag with `u`
   - Press `Enter` to start rebuild
   - Watch live output (scroll with `j`/`k`, `PageUp`/`PageDown`)
   - If prompted for password, press `i` to enter input mode
   - Type password and press `Enter`, then `Esc` to exit input mode

## Configuration

Configuration is stored at `~/.config/renix/config.toml`:

```toml
flake_path = "/etc/nixos"

[hosts.desktop]
connection = "Local"
extra_args = []

[hosts.server]
connection = { Remote = "user@server.example.com" }
extra_args = ["--option", "substitute", "false"]
```

## Keybindings

### Navigation
- `↑`/`↓` or `j`/`k` - Navigate hosts (or scroll output when available)
- `←`/`→` or `h`/`l` - Switch rebuild operation (switch, boot, test, etc.)
- `Tab` - Toggle between main and settings panel
- `PageUp`/`PageDown` - Scroll output by 10 lines
- `Home`/`End` - Jump to top/bottom of output

### Actions
- `Enter` - Start rebuild for selected host
- `u` - Toggle `--upgrade` flag
- `i` - Enter input mode (for typing passwords or interactive input)
- `Esc` - Cancel running build / Exit input mode
- `q` - Quit (press twice during build to cancel and quit)

### Editing
- `f` - Edit flake path
- `c` - Edit host connection
- `a` - Edit extra arguments for selected host

When editing:
- Type to input text
- `Backspace` to delete
- `Enter` to save
- `Esc` to cancel

### Input Mode (for passwords)
When in input mode (press `i` during build):
- Type your password or input
- `Enter` to submit
- `Backspace` to delete characters
- `Esc` to exit input mode
- Input is sent directly to the build process (PTY)

## Development

### Prerequisites

- Nix with flakes enabled
- Rust toolchain (provided by dev shell)

### Setup

```bash
# Clone the repository
git clone https://github.com/sarahlament/renix.git
cd renix

# Enter development shell
nix develop

# Build
cargo build

# Run
cargo run
```

### Project Structure

```
renix/
├── src/
│   ├── main.rs           # Entry point and CLI
│   ├── app.rs            # Application state and logic
│   ├── config/           # Configuration management
│   ├── nix/              # Nix command execution
│   └── ui/               # TUI rendering
├── nix/
│   └── module.nix        # NixOS module
├── flake.nix             # Nix flake with package, module, and overlay
└── Cargo.toml            # Rust dependencies
```

## Troubleshooting

### "Failed to spawn nixos-rebuild"

Ensure `nixos-rebuild` is in your PATH. This should be automatic on NixOS systems.

### Remote builds not working

1. Verify SSH access: `ssh user@host`
2. Ensure the remote user has sudo privileges
3. Check that the target configuration exists in your flake

### Configuration not persisting

Renix creates `~/.config/renix/config.toml` on first run. If changes aren't saving, check file permissions.

## Roadmap

- [ ] Build history viewer
- [ ] Search/filter in output
- [ ] Multiple simultaneous builds
- [ ] Color themes
- [ ] Home Manager support

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Author

**Sarah** ([@sarahlament](https://github.com/sarahlament))
- Email: sarah@lament.gay

## Acknowledgments

Built with:
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI library
- [tokio](https://tokio.rs/) - Async runtime
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation