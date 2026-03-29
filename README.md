# Atlas Observer

A Windows background application that reads Melty Blood Actress Again Current Code (MBAACC) game memory to validate and register ranked matches. Part of a larger ranked system for the MBAACC community.

## Overview

Atlas Observer runs alongside MBAACC and monitors match data in real time. It validates that a match meets the requirements for ranked play and, once complete, sends the result to a remote server for registration.

The application is built in Rust and communicates with the game process via the Windows `ReadProcessMemory` API. It is designed to run silently in the background with minimal user interaction.

## How It Works

Players who want to play a ranked match start ranked mode in the app, generating or entering a shared session code. The app then waits for MBAACC to launch, monitors the match as it plays out, validates it against a set of rules, and submits the result to the ranking server when it ends.

```
Player starts ranked mode
-> App generates or receives a session code
-> App waits for MBAA.exe to open
-> App reads game state every 16ms
-> Match is validated in real time
-> On match end, result is sent to the server
```

## Features

- Reads character, moon style, score, and timer data from game memory
- Tracks game mode transitions (Char Select, In Game, Retry Menu, Replay Menu)
- Validates match flow using a state machine
- Detects and rejects invalid matches (e.g. matches started mid-game or in replay mode)
- Sends match results to a remote server via HTTP POST
- Loads server URL and player ID from a `config.toml` file
- Waits for MBAA.exe to close before attaching, ensuring a clean read state
- Automatically re-attaches if the game is closed and reopened
- Cleans up Windows process handles on exit via `Drop`

## Configuration

Create a `config.toml` file in the same directory as the executable:

```toml
server_url = "https://the-server-url/api/match"
player_id = 1
```

## Usage

```
Commands:
  host <code>   Generate/Insert a session code and wait for opponent, code input is optional
  join <code>   Join a ranked session using a code from your opponent
  stop          Cancel ranked mode
```

Both players must be in ranked mode with matching session codes for a result to be submitted.

## To Fix

- [ ] More than one instance of CCCaster or MBAA causes the client to read the wrong process

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

cccaster is the standard netplay client for MBAACC. The app is designed to run alongside it but does not modify or hook into cccaster in any way in its current form.
