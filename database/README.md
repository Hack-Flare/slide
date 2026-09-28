# DB

Slide uses SurrealDB as its database. SurrealDB is embedded in the `slided` binary and uses rockdb as its storage engine. The database is used to store configuration, state, and other metadata for the Slide system.

## Migrations

All migrations are stored in `database/migrations/`.

