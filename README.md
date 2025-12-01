# termail
A terminal mail client inspired by mutt and notmuch, written in Rust. 

# Features
Features
- Dual Interface: Run in TUI mode for interactive browsing or CLI mode for scripting.
- Multiple Backends: Native support for Gmail (OAuth2) and Greenmail (Testing/Local).
- Maildir Synchronization: Syncs emails to local storage using the [Maildir](https://en.wikipedia.org/wiki/Maildir) format for offline access.
- Plugin System: Extensible via [WebAssembly (WASM)](https://github.com/WebAssembly/WASI) modules using [WIT](https://component-model.bytecodealliance.org) bindings.
- External Editor: Composes emails using your preferred editor (e.g., Vim, Neovim, VS Code).

# Installation
Prerequisites:
- Rust toolchain
- Docker (optional, for running local Greenmail test server)

Then, clone and simply run 
```bash
cargo build --release
```

# Configuration
Termail requires a configuration file to run. The application searches for config.toml in the following order:
- Path specified via --config-file argument.
- Current directory (./config.toml).
- User config directory (~/.config/termail/config.toml).
- System config directory (/etc/termail/config.toml).

## Config Structure
Create a config.toml file with the following structure:
```TOML
[termail]
cli = false
default_backend = "gmail" # Options: "greenmail", "gmail"
email_fetch_count = 20
editor = "vim"            # Command to launch your editor
plugins = []              # List of enabled plugin names (case-insensitive)

# Gmail Backend Configuration
[backends.gmail]
host = "imap.gmail.com"
port = 993
ssl = true
oauth2_client_secret_file = "./client_secret.json" # Required for Gmail
maildir_path = "./Maildir/Gmail"
filter_labels = ["CATEGORY_PROMOTIONS", "SPAM"] # Labels to exclude

# Greenmail (Local Test) Configuration
[backends.greenmail]
host = "127.0.0.1"
port = 1993
ssl = true
auth_credentials = { username = "user1@example.com", password = "password123" }
maildir_path = "./Maildir/Greenmail"
```

## Gmail OAuth2 Setup
To use the Gmail backend, you must provide a `client_secret.json` file generated from the Google Cloud Console.
- Create a project in Google Cloud Console.
- Enable the Gmail API.
- Create OAuth 2.0 Client ID credentials (type: Desktop App).
- Download the JSON file, rename it to `client_secret.json`, and place it in the application directory (or the path specified in `config.toml`).

# Usage

## TUI Keybindings
When running in TUI mode (default), the following keys are available:

| Context       | Key          | Action                                  |
|---------------|--------------|-----------------------------------------|
| Global        | `Esc`        | Quit application / go back              |
| Global        | `Tab`        | Cycle between Inbox and Labels panes    |
| Base View     | `c`          | Open Compose view                       |
| Inbox         | `Down / Up`  | Select next/previous email              |
| Inbox         | `Enter`      | Open selected email                     |
| Message View  | `Down / Up`  | Scroll message content                  |
| Compose       | `Down / Up`  | Cycle fields (To, Subject, Body)        |
| Sync/Refresh  | `r`          | Sync form cloud                         |
## CLI Commands
You can execute commands directly without entering the TUI by passing the --cli flag.

Fetch Inbox:
```bash
cargo run -- --cli fetch-inbox --count 5
```

Send Email:
```bash
cargo run -- --cli send-email --to "user@example.com" --subject "Hello" --body "Message body"
```
Note: If subject or body are omitted, the configured external editor will open.

Sync with Cloud: Performs synchronization between the configured backend and the local Maildir.

```bash
cargo run -- --cli sync-from-cloud
```

View downloaded messages in TUI
```bash
cargo run --  --backend Gmail view-mailbox
```

Null: 
```bash 
cargo run -- --cli null
```
This is primarily used for testing if your plugins are being properly loaded.

# Plugins
Termail supports plugins compiled to `.wasm` or `.cwasm` (although it will look for `.cwasm` files first). Plugins must be placed in `.config/termail/plugins` or `./plugins`. Termail will **only** look in one spot for the plugins.

Each plugin directory must contain a `manifest.toml`:

```toml
name = "MyPlugin"
description = "Does something cool"
backends = ["gmail"]
hooks = ["before_send"] 
```
See `plugins/` for more examples.

## Supported hooks:
- `before_send`: Modify email body before sending.
- `after_send`: Trigger actions after sending.
- `before_receive`: Process incoming emails.
- `after_receive`: Post-processing on received emails.

# Testing
We use [Greenmail](https://github.com/greenmail-mail-test/greenmail) to test the
application. You can run Greenmail by

```
docker compose -f test/docker-compose.yml up
```
or detach it with `-d`. 

You can send an email to the Greenmail server via 
```bash 
cargo run -- --cli send-email --to "user1@example.com"
```
The cli args `--to`, `--subject` and `--body` are optional but if they are provided they will be used to preffil the temp pop up editor. 

and test fetching the top email using
```bash 
cargo run -- --cli --backend Greenmail fetch-inbox
```

You can sync from cloud using:
```bash
cargo run -- --cli --backend Gmail sync-from-cloud
```

# Acknowledgement
As part of UCSD's [CSE 291Y](https://plsyssec.github.io/cse291y-fall25/).