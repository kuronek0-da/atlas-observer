# Atlas Observer

## Overview

Atlas Observer runs alongside MBAACC and monitors match data in real time. It validates that a match meets the requirements for ranked play and, once complete, sends the result to a remote server for registration.
It currently only works for CCCaster v3.1.008;

The application is built in Rust and communicates with the game process via the Windows `ReadProcessMemory` API. It is designed to run silently in the background with minimal user interaction.

## How It Works

Players who want to play a ranked match start ranked mode in the app, generating or entering a shared session code. The app then waits for MBAACC to launch, monitors the match as it plays out, validates it against a set of rules, and submits the result to the ranking server when it ends.

```
Player starts ranked mode
-> App generates or receives a session code
-> App waits for MBAA.exe and CCCaster to open
-> App reads game state every 16ms
-> Match is validated in real time
-> On match end, result is sent to the server
```

## Configuration

A config file named `config.toml` should be automatically created in the executable directory on the first run.

```toml
token=" ... "
```

## Usage

```
Commands:
  host <opt code>   Generate/Insert a session code and wait for opponent, code input is optional
  join <code>   Join a ranked session using a code from your opponent
  stop          Cancel ranked mode
  exit          Exits the app; you can also do CTRL-X
```

Both players must be in ranked mode with matching session codes for a result to be submitted.

## How to Build

### Windows
```bash
cargo build --release
```

The binary will be at `target/release/atlas-observer.exe`.

### Linux (cross-compiling for Windows)

Since Atlas Observer reads game memory from `MBAA.exe`, it must run as a Windows binary. The TUI is also currently visually broken when running via Wine, so cross-compiling is the recommended approach if you want to run on linux.

**1. Install the Windows target:**
```bash
rustup target add i686-pc-windows-gnu
```

**2. Install the MinGW cross-compiler:**
```bash
# Debian/Ubuntu
sudo apt install gcc-mingw-w64-i686

# Arch
sudo pacman -S mingw-w64-gcc
```

**3. Build:**
```bash
cargo build --release --target i686-pc-windows-gnu
```

The binary will be at `target/i686-pc-windows-gnu/release/atlas-observer.exe`.

### Custom Server URL

If you want to point the app at your own server, set `SERVER_URL` at compile time:
```bash
# Windows
SERVER_URL=https://your-server.com cargo build --release

# Linux
SERVER_URL=https://your-server.com cargo build --release --target i686-pc-windows-gnu
```

If `SERVER_URL` is not set, the app will use the default Atlas Index server.

## To Fix

- [x] More than one instance of CCCaster or MBAA causes the client to read the wrong process

## To Implement

- [ ] Better validation: more checks for game states
- [ ] Automatic opponent detection — currently requires manual session code sharing
- [ ] Session ID reading from cccaster memory or file (without modding the caster code)
- [ ] IP-based ranked session validation as an alternative handshake method
- [ ] Player authentication: Discord OAuth or create my own

## Related Projects

This application is one component of a larger system:

- **Atlas Observer** (this repo) — game client, match reader and validator
- **Atlas Index**
    - **[Server](https://github.com/kuronek0-da/atlas-index-server)** - receives match results, manages player rankings
    - **Website or Discord bot** (not started yet) - frontend to registration and checking leaderboards and statistics

## Notes

MBAACC is a 32-bit Windows application. All memory addresses are specific to the community version of the game. The app uses static offsets and does not require a memory scanner at runtime.

CCCaster is the standard netplay client for MBAACC. The app is designed to run alongside it but does not modify or hook into cccaster in any way in its current form.
