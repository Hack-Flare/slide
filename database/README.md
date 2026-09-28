# DB

Slide uses SurrealDB as its database. SurrealDB is embedded in the `slided` binary and currently uses its in-memory engine during early development. A persistent storage engine such as RocksDB will be added when Slide needs state to survive restarts. The database is used to store configuration, state, and other metadata for the Slide system.

## Migrations

All migrations are stored in `database/migrations/`.
