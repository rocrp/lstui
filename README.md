# lstui

Lobsters TUI (stories + nested comments) via `lobste.rs` JSON endpoints.

## Run

```bash
cargo run --release
```

Pick feed:

```bash
cargo run --release -- --feed hottest
cargo run --release -- --feed newest
cargo run --release -- --feed active
```

## Keys

Stories:
- `j/k` or `↓/↑`: move
- `gg` / `G`: top / bottom
- `Ctrl+d` / `Ctrl+u`: page down / up
- `Enter` / `Space` / `l` / `→`: open comments
- `o`: open source link in browser
- `O`: open comments page in browser
- `r`: refresh
- `?`: help
- `q` / `Esc`: quit

Comments:
- `j/k` or `↓/↑`: move
- `gg` / `G`: top / bottom
- `Ctrl+d` / `Ctrl+u`: page down / up
- `h` / `←`: collapse selected thread
- `l` / `→`: expand selected thread
- `Enter` / `c`: toggle collapse/expand
- `o`: open comments page in browser
- `O`: open source link in browser
- `r`: refresh
- `?`: help
- `q` / `Esc`: back

