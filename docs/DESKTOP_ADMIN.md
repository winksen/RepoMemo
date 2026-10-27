# The desktop app: the administrator's guide

How to give a team a working RepoMemo desktop app, keep it current and take it away (V0.4.63). The app is the web client in a window of
its own: it shows the same pages as the browser against your server, and your data stays on the server. Details of every setting are in
[DEVELOPMENT_COMMANDS.md](DEVELOPMENT_COMMANDS.md); where a machine keeps things is in [DESKTOP_FILES.md](DESKTOP_FILES.md).

## 1. What you hand out

Three files make an app that opens on your server with no address to type:

- **The program**, for the system: a Windows installer (`RepoMemo_<version>_x64-setup.exe`, per person, no administrator rights) and a portable
  `repomemo-desktop.exe`; a macOS universal `.dmg`; a Linux AppImage and `.deb`. CI builds them and keeps them as the artifacts
  `desktop-windows-x64`, `desktop-macos-universal` and `desktop-linux-x64` (the *Desktop Windows*, *Desktop macOS* and *Desktop Linux*
  workflows: run one by hand from the Actions page, or take the one of a release tag). Check a download against the artifact's `SHA256SUMS`.
- **The configuration file**, `repomemo-desktop.json`, which says which server, what the app is called and whether the person can change the
  server (§ 2).
- Optionally, **your authority's certificate** (`repomemo-ca.pem`) if the server's certificate comes from your own CA.

## 2. Package it for your server

On **Windows**, one command makes the whole handout:

```powershell
npm run desktop:package -- --server https://memo.example.com --name "Acme Memo" --support https://intranet.example.com/help/memo
```

It writes `dist-desktop/`: a portable zip (the program, the configuration file and a readme), the per-person installer, the configuration
file for IT, and `SHA256SUMS`. Add `--certificate ca.pem` for a private authority (the file is checked and a private key is refused) and
`--open` to let people change the server. The address, name and help link are checked by the rules the app uses, and a mistake stops the
command before it writes anything.

Two ways, and what each gives:

| | **With Rust and the Tauri CLI** | **Without Rust** (`--from <folder of the CI artifact>`) |
| --- | --- | --- |
| What happens | builds the app with your address compiled in | takes the program CI built and adds your configuration file |
| The program alone talks to | your server and no other | whatever the file beside it says; delete the file and it asks for an address |
| Use it when | people must not reach another server | you want a quick handout, and trust the team with a file |

On **macOS** and **Linux** hand out the `.dmg`, the AppImage or the `.deb` with the configuration file placed where the system looks for it
([DESKTOP_FILES.md](DESKTOP_FILES.md)). Build with `REPOMEMO_PRESET_SERVER=https://memo.example.com` to compile the address in.

## 3. Hand it to a team

1. **Install.** Windows, silently for IT: `RepoMemo_<version>_x64-setup.exe /S` (`/NS` for no shortcuts); put `repomemo-desktop.json` in
   `%APPDATA%\com.repomemo.desktop\` or beside the installed program. The portable zip needs nothing: unpack it and start `RepoMemo.exe`.
   macOS: open the `.dmg`, drag the app to Applications. Linux: `sudo apt install ./RepoMemo_<version>_amd64.deb`, or run the AppImage.
2. **Tell people what to expect.** The app opens on your server's sign-in page; they sign in with the account you gave them. Their sign-in
   is kept in the system's secure store, not in a file.
3. **The warnings of an unsigned app.** Windows SmartScreen and macOS Gatekeeper ask before they run a program they have no reputation for.
   Sign the build (a Windows certificate, an Apple Developer ID) or allow it by publisher or hash: [DEVELOPMENT_COMMANDS.md](DEVELOPMENT_COMMANDS.md),
   *Windows code signing and the SmartScreen warning* and *The desktop app on macOS*.
4. **Notices and links.** People get system notifications for the notices they chose and a badge for what waits in Review; the *Open in the
   app* links of the browser open pages in the app. Nothing for you to set up.

App and server must be the same release line (`0.4.x`): a window of a different line says so and asks to be updated.

## 4. Keep it current

- **Your own feed (managed).** Give the app `"updates": { "feed": "https://updates.example.com/latest.json", "publicKey": "<your key>" }` in the
  configuration file. It looks for a newer version, offers it, and installs it only when the person agrees; every update is checked against
  your key, and an unsigned one, or one signed for another version, is refused. Make `latest.json` with `node scripts/update-feed.mjs` from
  the signed installer and its `.sig`.
- **The project's releases (the person's choice).** In a build that carries the release key, *Check for updates* in the account area is off
  until the person turns it on.
- **Without either**, people install the new version over the old one (the installer upgrades in place and keeps the sign-in), or you push
  it with your software tool.
- **Servers with a new release line:** upgrade the server first ([DEPLOYMENT.md](DEPLOYMENT.md), § 5), then the apps.

## 5. Take it away

- **One person, keeping the app:** *Change server* in the account area forgets the server and the sign-in (absent when you locked the server).
- **Forget the sign-in without removing the app:** start the program with `--forget-session` (it opens no window).
- **Remove the app:** Windows: *Apps* in Settings or `uninstall.exe` (it asks whether to forget the server and the sign-in; `/S` keeps them, so
  run `--forget-session` first to forget); macOS: drag the app to the bin; Linux: `sudo apt remove repomemo` or delete the AppImage.
- **Remove the configuration and data:** delete the folders in [DESKTOP_FILES.md](DESKTOP_FILES.md); that page also has the commands that
  delete a saved sign-in by hand.
- **Stop one account from using the app:** the account lives on the server. Deactivate it or revoke its sessions there (*System › Users*);
  deleting the app from a computer does not end a session.
