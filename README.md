# Remindr

A local-only alternative to Notion, written in Rust with [GPUI-kit](https://gpui-kit.com). Pages are trees of blocks; databases hold rows with typed properties and show up in pages through table and board views. Everything stays on your machine in one SQLite file: `~/Library/Application Support/Remindr/workspace.db`.

macOS only for now.

```bash
cargo run -p remindr-desktop --release
```
