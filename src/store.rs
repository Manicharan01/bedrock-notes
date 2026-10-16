use crate::{markdown, model::*};
use rusqlite::{Connection, TransactionBehavior, params};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Store {
    connection: Connection,
}

fn time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let seed: Vec<Note> = serde_json::from_str(include_str!("../web/seed.json"))
            .map_err(|e| Error::Internal(e.to_string()))?;
        Self::open_with_seed(path, &seed)
    }

    pub fn open_with_seed(path: &Path, seed: &[Note]) -> Result<Self, Error> {
        Self::initialize(Connection::open(path)?, seed)
    }

    pub fn memory() -> Result<Self, Error> {
        Self::initialize(Connection::open_in_memory()?, &[])
    }

    fn initialize(mut connection: Connection, seed: &[Note]) -> Result<Self, Error> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        // Check and initialize inside one write transaction to avoid first-start races.
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i32 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            0 => {
                validate_notes(seed)?;
                transaction.execute_batch("CREATE TABLE notebook (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL CHECK(revision>=0));
                    INSERT INTO notebook VALUES(1,0);
                    CREATE TABLE notes (id TEXT PRIMARY KEY NOT NULL, title TEXT NOT NULL, title_key TEXT UNIQUE NOT NULL, body TEXT NOT NULL, updated INTEGER NOT NULL CHECK(updated>=0), position INTEGER UNIQUE NOT NULL);")?;
                write_notes(&transaction, seed)?;
                transaction.pragma_update(None, "user_version", 1)?;
            }
            1 | 2 => {}
            _ => {
                return Err(Error::Internal(format!(
                    "Unsupported database schema version {version}; the database was not migrated"
                )));
            }
        }
        if version < 2 {
            transaction.execute_batch("CREATE TABLE instance (id INTEGER PRIMARY KEY CHECK(id=1), notebook_id TEXT NOT NULL);")?;
            transaction.execute(
                "INSERT INTO instance VALUES(1,?1)",
                [uuid::Uuid::new_v4().to_string()],
            )?;
            transaction.pragma_update(None, "user_version", 2)?;
        }
        transaction.commit()?;
        let mut store = Self { connection };
        // Refuse corrupt/incompatible stored rows rather than replacing them with samples.
        let snapshot = store.read()?;
        validate_notes(&snapshot.notes)?;
        Ok(store)
    }

    pub fn read(&mut self) -> Result<Snapshot, Error> {
        let transaction = self.connection.transaction()?;
        let notebook_id =
            transaction.query_row("SELECT notebook_id FROM instance WHERE id=1", [], |r| {
                r.get(0)
            })?;
        let revision =
            transaction.query_row("SELECT revision FROM notebook WHERE id=1", [], |r| r.get(0))?;
        let notes = load_notes(&transaction)?;
        transaction.commit()?;
        let mut snapshot = markdown::snapshot(revision, notes);
        snapshot.notebook_id = notebook_id;
        Ok(snapshot)
    }

    pub fn version(&mut self) -> Result<Version, Error> {
        Ok(self.connection.query_row("SELECT notebook_id, revision FROM instance JOIN notebook ON instance.id=notebook.id WHERE instance.id=1", [], |r| Ok(Version {notebook_id:r.get(0)?,revision:r.get(1)?}))?)
    }

    pub fn apply(&mut self, request: CommandRequest) -> Result<CommandResponse, Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let notebook_id =
            transaction.query_row("SELECT notebook_id FROM instance WHERE id=1", [], |r| {
                r.get(0)
            })?;
        let revision: i64 =
            transaction.query_row("SELECT revision FROM notebook WHERE id=1", [], |r| r.get(0))?;
        if request.revision != revision || request.notebook_id != notebook_id {
            return Err(Error::Conflict);
        }
        let mut notes = load_notes(&transaction)?;
        let selected_id = apply_command(&mut notes, request.command)?;
        validate_notes(&notes)?;
        let next_revision = revision
            .checked_add(1)
            .filter(|n| *n <= 9_007_199_254_740_991)
            .ok_or_else(|| Error::Internal("Revision limit reached".into()))?;
        transaction.execute("DELETE FROM notes", [])?;
        write_notes(&transaction, &notes)?;
        transaction.execute(
            "UPDATE notebook SET revision=?1 WHERE id=1",
            [next_revision],
        )?;
        transaction.commit()?;
        let mut notebook = markdown::snapshot(next_revision, notes);
        notebook.notebook_id = notebook_id;
        Ok(CommandResponse {
            notebook,
            selected_id,
        })
    }
}

fn load_notes(connection: &Connection) -> Result<Vec<Note>, Error> {
    let mut statement =
        connection.prepare("SELECT id,title,body,updated FROM notes ORDER BY position")?;
    let rows = statement.query_map([], |r| {
        Ok(Note {
            id: r.get(0)?,
            title: r.get(1)?,
            body: r.get(2)?,
            updated: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn write_notes(connection: &Connection, notes: &[Note]) -> Result<(), Error> {
    let mut insert = connection.prepare(
        "INSERT INTO notes(id,title,title_key,body,updated,position) VALUES(?1,?2,?3,?4,?5,?6)",
    )?;
    for (position, note) in notes.iter().enumerate() {
        insert.execute(params![
            note.id,
            note.title,
            canonical(&note.title),
            note.body,
            note.updated,
            position as i64
        ])?;
    }
    Ok(())
}

fn touch(note: &mut Note) {
    note.updated = time_ms().max(note.updated.saturating_add(1));
}

fn apply_command(notes: &mut Vec<Note>, command: Command) -> Result<Option<String>, Error> {
    match command {
        Command::Create { title, body } => {
            let title = validate_title(&title)?;
            if notes
                .iter()
                .any(|n| canonical(&n.title) == canonical(&title))
            {
                return Err(Error::TitleExists);
            }
            let id = uuid::Uuid::new_v4().to_string();
            notes.insert(
                0,
                Note {
                    id: id.clone(),
                    title,
                    body,
                    updated: time_ms(),
                },
            );
            Ok(Some(id))
        }
        Command::Update { id, body } => {
            let note = notes
                .iter_mut()
                .find(|n| n.id == id)
                .ok_or(Error::NotFound)?;
            note.body = body;
            touch(note);
            Ok(Some(id))
        }
        Command::Rename { id, title } => {
            let title = validate_title(&title)?;
            if !notes.iter().any(|n| n.id == id) {
                return Err(Error::NotFound);
            }
            if notes
                .iter()
                .any(|n| n.id != id && canonical(&n.title) == canonical(&title))
            {
                return Err(Error::TitleExists);
            }
            let original = notes.clone();
            let replacements = BTreeMap::from([(id.clone(), title.clone())]);
            for note in notes.iter_mut() {
                let body = markdown::rewrite_targets(&note.body, &original, &replacements);
                if body != note.body || note.id == id {
                    note.body = body;
                    if note.id == id {
                        note.title.clone_from(&title);
                    }
                    touch(note);
                }
            }
            Ok(Some(id))
        }
        Command::Delete { id } => {
            let index = notes
                .iter()
                .position(|n| n.id == id)
                .ok_or(Error::NotFound)?;
            notes.remove(index);
            Ok(notes.first().map(|n| n.id.clone()))
        }
        Command::Import { notes: incoming } => import(notes, incoming),
    }
}

fn import(notes: &mut Vec<Note>, incoming: Vec<ImportNote>) -> Result<Option<String>, Error> {
    if incoming.is_empty() {
        return Err(Error::Invalid("Select at least one note to import".into()));
    }
    if notes.len().saturating_add(incoming.len()) > MAX_NOTES {
        return Err(Error::Invalid("Notebook limit is 1000 notes".into()));
    }
    let mut used: HashSet<String> = notes.iter().map(|n| canonical(&n.title)).collect();
    let mut originals = Vec::new();
    let mut replacements = BTreeMap::new();
    let mut batch_titles = HashSet::new();
    for entry in incoming {
        let title = validate_title(&entry.title)?;
        if !batch_titles.insert(canonical(&title)) {
            return Err(Error::Invalid(
                "Import contains duplicate note titles; split or rename those files first".into(),
            ));
        }
        let mut planned = title.clone();
        let mut suffix = 2;
        while used.contains(&canonical(&planned)) {
            let ending = format!(" ({suffix})");
            let prefix: String = title
                .chars()
                .take(MAX_TITLE - ending.chars().count())
                .collect();
            planned = format!("{}{ending}", prefix.trim_end_matches([' ', '.']));
            suffix += 1;
        }
        used.insert(canonical(&planned));
        let id = uuid::Uuid::new_v4().to_string();
        replacements.insert(id.clone(), planned);
        originals.push(Note {
            id,
            title,
            body: entry.body,
            updated: time_ms(),
        });
    }
    validate_notes(&originals)?;
    let selected_id = originals.first().map(|n| n.id.clone());
    let imported: Vec<Note> = originals
        .iter()
        .map(|n| Note {
            title: replacements[&n.id].clone(),
            body: markdown::rewrite_targets(&n.body, &originals, &replacements),
            ..n.clone()
        })
        .collect();
    notes.extend(imported);
    Ok(selected_id)
}
