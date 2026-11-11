# Drawerama

Local single-user desktop app for keeping track of rooms, cabinets, shelves, drawers, and the items in them. A suggestion is advice: nothing is written until you apply it, and you can still place an item by hand.

Linux and Windows. One SQLite file.

## Layout

```
src/main.rs                     eframe window
src/lib.rs                      domain and storage
src/app/                        screens
src/domain/model.rs             rooms, furniture, items
src/domain/space.rs             fit, volume, liters
src/domain/suggest.rs           placement plans
src/storage/                    SQLite open, migration, queries
src/storage/migrations/         schema
tests/use_cases.rs              register a home, suggest, apply
```

## Commands

```
cargo test
cargo run
```

The database is `drawerama/drawerama.db` in the platform data directory. On Linux that is `~/.local/share/drawerama/drawerama.db`. Set `DRAWERAMA_DB` to use another file:

```
DRAWERAMA_DB=/tmp/home.db cargo run
```
