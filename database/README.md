# DB

Slide uses SurrealDB as its database. SurrealDB is embedded in the `slided` binary and currently uses its in-memory engine during early development. A persistent storage engine such as RocksDB will be added when Slide needs state to survive restarts. The database is used to store configuration, state, and other metadata for the Slide system.

## Migrations

All migrations are stored in `database/migrations/`.

Migration files use the following format:
```
<description>.<number>.surql
```

For example:
```
node_identity.1.surql
cluster_members.2.surql
deployment_state.3.surql
```

Migrations are embedded into `slided`, discovered at startup, sorted by number, and applied in order. Numbers must start at `1` and increase by `1` without gaps.
