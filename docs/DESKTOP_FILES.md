# Where the desktop app keeps its files

For administrators (V0.4.61): where each system keeps the app's configuration, its saved session and what it writes besides, and how to find,
change and remove each. The tables use the app's identifier, `com.repomemo.desktop`; the places for the data the web view keeps are the
operating system's defaults for that identifier, so look at them on one machine before scripting against them.

## The three things, on each system

| | Windows | macOS | Linux |
| --- | --- | --- | --- |
| **The program** | `%LOCALAPPDATA%\RepoMemo\` (the installer); wherever the portable zip was unpacked | `/Applications/RepoMemo.app` (from the .dmg) | `/usr/bin/repomemo-desktop` (the .deb); wherever the AppImage is |
| **Configuration** (the administrator's file, the saved server, the window's place) | `%APPDATA%\com.repomemo.desktop\` | `~/Library/Application Support/com.repomemo.desktop/` | `~/.config/com.repomemo.desktop/` |
| **The saved session** (the sign-in) | Credential Manager › Windows Credentials, an entry per server named `<server address>.RepoMemo` | Keychain, service `RepoMemo`, account `<server address>` | the Secret Service (GNOME Keyring, KWallet): service `RepoMemo`, username `<server address>` |
| **The web view's own data** (cache, the theme and the choices the page remembers) | `%LOCALAPPDATA%\com.repomemo.desktop\` | `~/Library/WebKit/com.repomemo.desktop/` and `~/Library/Caches/com.repomemo.desktop/` | `~/.local/share/com.repomemo.desktop/` and `~/.cache/com.repomemo.desktop/` |
| **Logs** | none of its own (see below) | none of its own | none of its own |

## What is in the configuration folder

| File | What it is | Who writes it |
| --- | --- | --- |
| `repomemo-desktop.json` | The administrator's file: `server`, `name`, `locked`, `support`, `certificate`, `updates` ([DEVELOPMENT_COMMANDS.md](DEVELOPMENT_COMMANDS.md)). A copy **beside the program** wins over this one. | the administrator |
| `server.json` | The server the person connected to (`{ "server_url": ... }`), used when no file locks one | the app, after a first successful connection |
| `.window-state.json` | The window's size and place | the app, on closing |

A certificate bundle named in `repomemo-desktop.json` (`"certificate": "repomemo-ca.pem"`) is read from beside that file.

## Find, change, remove

- **Find what a machine is set up for.** Open the configuration folder above. `repomemo-desktop.json` there (or beside the program) says
  what the administrator set; if it is wrong the app shows its path and the reason in place of everything else. Without it, `server.json`
  names the server the person chose. The account area shows the server the app is connected to.
- **Change the server.** For a person: *Change server* in the account area, which forgets the server and the session and asks again
  (absent when the server is locked by a file or compiled in). For IT: edit or replace `repomemo-desktop.json`, or delete `server.json`.
- **Remove the saved sign-in without removing the app.** Run the program with `--forget-session` (it opens no window): it deletes the
  session of every server it knows (the compiled-in one, the file's and the saved one) and `server.json`, and leaves the administrator's
  file alone. On Windows the same by hand: `cmdkey /delete:"https://memo.example.com.RepoMemo"`. On macOS: `security delete-generic-password
  -s RepoMemo -a https://memo.example.com`. On Linux: `secret-tool clear service RepoMemo username https://memo.example.com`.
- **Remove the app.** Windows: *Apps* in Settings or `%LOCALAPPDATA%\RepoMemo\uninstall.exe`; it asks whether to forget the server and the
  sign-in (a silent `/S` keeps them; run `--forget-session` first to forget). macOS: drag `RepoMemo.app` to the bin. Linux: `sudo apt remove
  repomemo` or delete the AppImage. None of these removes the configuration folder or the web view's data; delete those too for a clean
  machine, and the sessions as above if `--forget-session` was not run.
- **Reset the page's own choices** (theme, remembered searches, *Check for updates*): delete the web view's data folder above.

## About logs

The app writes **no log file**. A debug build prints to its console; a release build has none. What to look at instead:

- **The server.** *System › Logs* and *System › Overview* on the server record what the app asked, and list the client as `desktop`; the
  audit trail has the sign-ins. This is where "it will not connect" is usually answered.
- **The app's own message.** A bad configuration file, a server it cannot reach, a refused link or a failed update is shown on the page, in
  a sentence, not logged.
- **The system.** On Linux, start the program from a terminal to see what it prints; on Windows and macOS the installer and the system's
  own logs (Event Viewer, Console) hold anything about the installation itself.
