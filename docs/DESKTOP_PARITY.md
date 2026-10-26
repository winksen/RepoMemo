# The desktop app against the web client: parity audit

The desktop app is the web client in a window (decision 33, V0.4.43): one bundle, one entry (`index.html` loading `src/main.tsx`),
the same pages, components and styles. `App` chooses the window's start (`DesktopApp` in Tauri, `SharedWebApp` in a browser) and the
desktop's start is `SharedWebApp` once a server is chosen. A difference between the two is a defect to be listed here and either
closed by a release or kept for a stated reason. `apps/desktop/src/test/desktopParity.test.ts` reads this page: every row below must
say which, and the list of files that know about the desktop must be the one in the last section.

Written for V0.4.43; each release that closes a row updates it and says so (*done*).

## The differences

| Area | In the browser | In the desktop app | Closed by or why it stays |
| --- | --- | --- | --- |
| Transport | `fetch` of the page, with the refresh token in an HttpOnly cookie | the HTTP plugin's `fetch` for the chosen server only, no redirects, the refresh token in the operating system's credential store; the client says it is `desktop` | Closed by V0.4.44 (done: `lib/transport.ts` is the one seam, `sharedApi` sends every request through it, the plugin is loaded there and nowhere else, both transports tested) |
| Which server | the page's own host (`/config.json`) | typed on a connect screen, checked by the shell, kept in `server.json` | Closed by V0.4.46 (done: a preset address compiled in, no field, no change) and V0.4.47 (done: `repomemo-desktop.json`, the same executable with two files talks to two servers); typing stays for an unconfigured app |
| First screen | the sign-in page | `DesktopConnect`, a screen of its own with its own copy | Closed by V0.4.45 (done: `AuthFrame` frames both the sign-in page and the connect screen, which use the same field, input, button and toast) |
| Downloads | an anchor with `download`, the browser's own dialog | the same anchor, which a webview treats differently per system (a blob download may not save at all) | Closed by V0.4.52 (done: every download goes through `saveBlob`, which the app replaces with a native save dialog) |
| Open a PDF or an Office file | a new tab, or the application's URL scheme (`ms-word:`) | saved first, then opened from the file manager; no tab, no scheme (`DocumentViewer`) | Stays: a window without tabs cannot hand a link to another program; V0.4.52 makes the saving clean |
| Links to other sites | open in a new tab | `target="_blank"` links go to the system browser (`installExternalLinkHandler`); the window never leaves the bundled client | Stays: the window must not leave the client (it holds the session) |
| Links into the app | a URL | none: a link from a notice or a page of the web client cannot open the app | Closed by V0.4.54 (done: `repomemo://open?server=...&path=...`, the app takes only the client's own paths for its own server; browsers offer *Open in the app*) |
| Notifications | the Notifications page, in the tab | the same page; nothing when the window is in the background | Closed by V0.4.53 (done: `NoticeWatcher` looks at the notices, the shell shows a system notification when the window is in the background and marks the icon with the unread notices and the Review queue) |
| Shortcuts | the page's own (command palette, editor, submit) | the same; no menu bar shortcuts of the platform, no global ones | Stays: nothing in the web client needs a platform menu |
| Window | a tab | a fixed title, a default size and place, any number of copies | Closed by V0.4.51 (done: the title bar follows the page title, which names the workspace; one copy per person; size and place remembered) |
| Version note | tells the person to reload or ask an administrator | tells the person to update the app (`VersionMismatch` takes a `desktop` flag) | Closed by V0.4.44 (done: the flag is `platform.installedApp`, from `lib/platform.ts`, the one place that knows the runtime) |
| Server note | none | the server's name and *Change server* in the account area (`DesktopServerNote`) | Stays: only the app has a server to change; it is the one desktop-only piece of the page and asks `platform.choosesServer` |
| Updates | the next page load | none: the person installs a new version by hand | Closed by V0.4.56 (a signed update feed, opt-in) |
| Private certificate authority | the browser's own store | the system store only | Closed by V0.4.55 (done: a `certificate` bundle in the file, checked by a relay that trusts it for that server only) |
| Installing | none | a Windows installer, then Linux and macOS packages | Closed by V0.4.49 to V0.4.60 |

## Files that know about the desktop

The files below, and only these, test the desktop runtime, load the shell's plugins or hold the desktop session. A component asks `platform` for a capability (`opensTabs`, `choosesServer`, `installedApp`) and never which runtime it is in (V0.4.44); the only components on the list are the entry and the desktop's own start. The list turns into a check that fails the build at V0.4.58.

- `src/App.tsx`
- `src/main.tsx`
- `src/components/DesktopApp.tsx`
- `src/lib/desktop.ts`
- `src/lib/platform.ts`
- `src/lib/sharedApi.ts`
- `src/lib/transport.ts`
