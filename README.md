# ludi-pq-stage-8-tool

A native Windows helper tool for **Ludibrium Party Quest Stage 8** in MapleStory. It renders a transparent, always-on-top **Direct2D overlay** and walks through all 126 five-of-nine box combinations using a **constant-weight Gray code**.

Every pair of consecutive states differs by exactly one occupied box, so every failed attempt requires exactly **one player** to move. The complete route has 126 states, no duplicates, and the theoretical minimum of 125 single-player transitions.

## Download

A **prebuilt executable** is available in the [`artifact/`](artifact/) directory:

```
artifact/ludi-pq-stage-8-tool.exe
```

No installation required — download it and run it directly on Windows.

## Features

- **Constant-weight Gray-code route** — covers all `C(9,5) = 126` answers exactly once and moves only one player per step.
- **Control panel** — start/restart a session, step forward/backward through the sequence, toggle positioning mode, show/hide the overlay, and adjust overlay **scale** and **opacity** with sliders.
- **Transparent overlay** — draggable and resizable; empty boxes are gray, stable occupied boxes are blue, the box to leave is red, and the destination box is green.
- **Clipboard integration** — each instruction puts the resulting state before the action (for example, `State 2/126 · Occupied {1,2,3,5,7} · Move box 4 to box 7`), copies it automatically, and confirms it with a toast notification.
- **Global hotkeys** — `Page Up` / `Page Down` to move to the previous / next step.
- **Persistent settings** — overlay position, scale, opacity, and visibility are saved to a `settings.json` file between runs.

## Usage

1. Launch `ludi-pq-stage-8-tool.exe`.
2. Drag the overlay over the in-game stage area and resize it to align with the boxes (positioning mode shows the full layout).
3. Click **Start** and place the party on the five highlighted boxes.
4. After a wrong result, press **Next** (or `Page Down`). Move the player on the red box to the green box, then check again. Blue boxes stay occupied and gray boxes stay empty.
5. Use **Previous** (or `Page Up`) to undo an accidental advance; the reverse single-player move is shown and copied automatically.

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
- The Gray code starts at boxes `{1,2,3,4,5}`. Its internal bit-to-box permutation is selected for a shorter route on the Stage 8 layout; all instructions continue to use the original in-game box numbers.

## License

See [LICENSE](LICENSE).
