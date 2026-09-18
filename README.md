# redb-view

Browse [redb](https://crates.io/crates/redb) databases from a library or CLI: list tables and page key/value rows.

Values that are valid UTF-8 are shown as text; otherwise as hex. No application schema is assumed.

## Library

```rust
use redb_view::DatabaseView;

let db = DatabaseView::open("path/to/file.redb")?;
for table in db.tables()? {
    println!("{} {:?} len={}", table.name, table.kind, table.len);
}
let rows = db.page("my_table", 0, 50)?;
```

## CLI

```text
redb-view <path.redb> tables
redb-view <path.redb> page <table> [--offset N] [--limit N]
```

## License

MIT OR Apache-2.0

## GUI (optional)

Build and run the desktop browser (egui):

```bash
cargo run --features gui --bin redb-view-gui
```

Open a local `.redb` file, pick a table, then page through rows. The GUI talks only to this crate’s library API (never imports `redb` directly).

The GUI opens a native file picker (`.redb` / `.db`). Theme menu: **System** (default, follows the OS), **Light**, or **Dark**. The theme choice is saved under `~/.config/redb-view/theme`.
