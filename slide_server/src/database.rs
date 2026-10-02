use std::io;

use include_dir::{Dir, include_dir};
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem};

static MIGRATIONS_DIRECTORY: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../database/migrations");

pub async fn open() -> io::Result<Surreal<Db>> {
    let client = Surreal::new::<Mem>(()).await.map_err(database_error)?;
    client
        .use_ns("slide")
        .use_db("slide")
        .await
        .map_err(database_error)?;

    for migration in migrations()? {
        client
            .query(migration.contents)
            .await
            .map_err(database_error)?;
    }

    Ok(client)
}

struct Migration {
    number: u64,
    contents: &'static str,
}

fn migrations() -> io::Result<Vec<Migration>> {
    let mut migrations = Vec::new();
    for file in MIGRATIONS_DIRECTORY.files() {
        let filename = file
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "migration has an invalid filename",
                )
            })?;
        let stem = filename.strip_suffix(".surql").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("migration does not use the .surql extension: {filename}"),
            )
        })?;
        let (_, number) = stem.rsplit_once('.').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("migration does not use description.number format: {filename}"),
            )
        })?;
        let number = number.parse::<u64>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("migration has an invalid number: {filename}"),
            )
        })?;

        migrations.push(Migration {
            number,
            contents: file.contents_utf8().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("migration is not valid UTF-8: {filename}"),
                )
            })?,
        });
    }

    migrations.sort_by_key(|migration| migration.number);
    for (index, migration) in migrations.iter().enumerate() {
        let expected_number = index as u64 + 1;
        if migration.number != expected_number {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "migration sequence must start at 1 and increase by 1: expected {expected_number}, found {}",
                    migration.number
                ),
            ));
        }
    }

    Ok(migrations)
}

fn database_error(error: impl std::error::Error) -> io::Error {
    io::Error::other(format!("database error: {error}"))
}
