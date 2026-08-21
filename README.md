# ludi-pq-stage-8-tool

A native Windows helper tool for **Ludibrium Party Quest Stage 8** (the box-positioning / "JMS" stage) in MapleStory. It renders a transparent, always-on-top **Direct2D overlay** showing which boxes should be occupied, and walks you through the 126-state sequence step by step.

## Download

A **prebuilt executable** is available in the [`artifact/`](artifact/) directory:

```
artifact/ludi-pq-stage-8-tool.exe
```

No installation required — download it and run it directly on Windows.

## Features

- **Control panel** — start/restart a session, step forward/backward through the sequence, toggle positioning mode, show/hide the overlay, and adjust overlay **scale** and **opacity** with sliders.
- **Transparent overlay** — draggable and resizable, highlights the boxes that should be occupied in the current state.
- **Clipboard integration** — each step's instruction (e.g. `step 1:{1,3,4,6,7}->{1,3,6,7,8} (4 goto 8)`) is automatically copied to the clipboard, and a toast notification confirms it.
- **Global hotkeys** — `Page Up` / `Page Down` to move to the previous / next step.
- **Persistent settings** — overlay position, scale, opacity, and visibility are saved to a `settings.json` file between runs.

## Usage

1. Launch `ludi-pq-stage-8-tool.exe`.
2. Drag the overlay over the in-game stage area and resize it to align with the boxes (positioning mode shows the full layout).
3. Click **Start** in the control panel, then use the **Next** / **Previous** buttons (or `Page Down` / `Page Up`) to advance through the steps. Each instruction is copied to the clipboard automatically.

## Building from source

Requires a Rust toolchain on Windows:

```sh
cargo build --release
```

The binary is produced at `target/release/ludi-pq-stage-8-tool.exe`.

A headless smoke test (no visible windows) can be run with:

```sh
cargo test
LUDI_PQ_STAGE_8_TOOL_SMOKE_TEST=1 cargo run
```

## Notes

- Windows only (uses Win32, Direct2D, and DirectWrite APIs).
- The overlay layout image is embedded from `assets/stage8_chairs_layout.png`.

## License

See [LICENSE](LICENSE).
