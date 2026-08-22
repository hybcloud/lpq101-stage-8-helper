# lpq101-stage-8-helper

Companion tools for **Ludibrium Party Quest Stage 8** in MapleStory:

- a native Windows **Direct2D overlay** with positioning, scale, opacity, global hotkeys, and optional online sync;
- a responsive web room service for sharing the current step with read-only viewers.

Every pair of consecutive states differs by exactly one occupied box, so every failed attempt requires exactly **one player** to move. The complete route has 126 states, no duplicates, and the theoretical minimum of 125 single-player transitions.

- Web helper: <https://lpq101-stage-8-helper.hvidia.com>
- Source: <https://github.com/hybcloud/lpq101-stage-8-helper>

## Download

A **prebuilt executable** is available in the [`artifact/`](artifact/) directory:

```
artifact/lpq101-stage-8-helper.exe
```

No installation required — download it and run it directly on Windows.

## Native app

- **Constant-weight Gray-code route** — covers all `C(9,5) = 126` answers exactly once and moves only one player per step.
- **Control panel** — start/restart a session, step forward/backward through the sequence, toggle positioning mode, show/hide the overlay, and adjust overlay **scale** and **opacity** with sliders.
- **Transparent overlay** — draggable and resizable; empty boxes are gray, stable occupied boxes are blue, the box to leave is red, and the destination box is green.
- **Clipboard integration** — each instruction puts the resulting state before the action (for example, `State 2/126 · Occupied {1,2,3,5,7} · Move box 4 to box 7`), copies it automatically, and confirms it with a toast notification.
- **Global hotkeys** — `Page Up` / `Page Down` to move to the previous / next step.
- **Persistent settings** — overlay position, scale, opacity, visibility, and the local owner identity are saved under `%APPDATA%\lpq101-stage-8-helper\settings.json`.
- **Optional online sync** — the native tool remains fully offline by default. It can host a room for web/native viewers, or join an existing room as a read-only native viewer.
- **Parchment control panel** — the native panel uses the same warm beige/brown palette as the web interface while preserving native-only overlay fitting controls.

### Native usage

1. Launch `lpq101-stage-8-helper.exe`.
2. Drag the overlay over the in-game stage area and resize it to align with the boxes (positioning mode shows the full layout).
3. Click **Start** and place the party on the five highlighted boxes.
4. After a wrong result, press **Next** (or `Page Down`). Move the player on the red box to the green box, then check again. Blue boxes stay occupied and gray boxes stay empty.
5. Use **Previous** (or `Page Up`) to undo an accidental advance; the reverse single-player move is shown and copied automatically.

### Optional online mode

- Click **Host** to create or restore this installation's room. The four-character room code is shown and the viewer invite is copied automatically.
- Enter a four-character room code and click **Join** to use the native app as a read-only viewer. Step buttons and hotkeys are disabled, while overlay position, scale, opacity, and visibility remain local and adjustable.
- Click **Leave** to return to the fully offline tool. No network thread is started until **Host** or **Join** is selected.

Set `LPQ_SERVICE_URL` before launching the native app to override its service origin at runtime, for example when following a local Worker:

```powershell
$env:LPQ_SERVICE_URL = "http://127.0.0.1:8787"
.\target\release\lpq101-stage-8-helper.exe
```

## Web service

The web interface uses the same Stage 8 layout and colors as the native overlay, without native-only positioning, scale, or opacity controls.

- `/` creates a room or joins one by its four-character `0-9` / `A-Z` code.
- `/host` is the parameter-free owner page. **Previous**, **Next**, and **Reset** update every connected viewer, and **Copy Viewer Invite Link** copies the public invitation.
- `/room/CODE` opens directly in read-only viewer mode, which is also the URL used by invitation links.
- A browser owner receives an HttpOnly GUID cookie. Creating again restores its active room instead of allocating another room code.
- Rooms are synchronized over WebSockets through a Cloudflare Durable Object. Once every WebSocket has disconnected, the room is released after five minutes and the owner receives a new code the next time a room is created.
- Both owner and viewer pages provide a route back to the home page.

## Shared configuration

The production worker name and service origin are defined once in [`project-config.json`](project-config.json). The Worker, web invitation links, Wrangler Custom Domain, and native client consume this configuration.

After changing `workerName` or `serviceOrigin`, run:

```sh
npm run config:sync
```

The build, dev, and deploy scripts run the same synchronization automatically.

## Building the native app

Requires a Rust toolchain on Windows:

```sh
cargo build --release
```

The binary is produced at `target/release/lpq101-stage-8-helper.exe`. To update the checked-in download artifact:

```powershell
Copy-Item .\target\release\lpq101-stage-8-helper.exe .\artifact\lpq101-stage-8-helper.exe -Force
```

## Web development and deployment

Install the locked Node dependencies and start Wrangler's local development server:

```sh
npm ci
npm run dev
```

The local service is available at <http://127.0.0.1:8787>. Tailwind is used only at build time: `npm run build` writes a static `public/styles.css`; no Tailwind runtime is shipped to browsers.

To deploy the Worker, static assets, and Durable Object to the custom domain configured in `wrangler.jsonc`:

```sh
npx wrangler login
npm run deploy
```

Cloudflare credentials stay outside the repository. Local `.env*`, `.dev.vars*`, and `.wrangler/` state are ignored by Git.

After changing Worker bindings, regenerate the checked-in Cloudflare declarations with:

```sh
npm run cf-typegen
```

## Verification

A headless smoke test (no visible windows) can be run with:

```sh
cargo test
LPQ101_STAGE_8_HELPER_SMOKE_TEST=1 cargo run
```

On PowerShell, set the smoke-test variable with `$env:LPQ101_STAGE_8_HELPER_SMOKE_TEST = "1"` before running `cargo run`.

## Notes

- Windows only (uses Win32, Direct2D, and DirectWrite APIs).
- The overlay layout image is embedded from `assets/stage8_chairs_layout.png`.
- The Gray code starts at boxes `{1,2,3,4,5}`. Its internal bit-to-box permutation is selected for a shorter route on the Stage 8 layout; all instructions continue to use the original in-game box numbers.

## License

See [LICENSE](LICENSE).
