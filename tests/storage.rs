use lattice::{model::*, store::Store};
use std::{
    path::Path,
    sync::{Arc, Barrier},
};

fn command(store: &mut Store, revision: i64, command: Command) -> Result<CommandResponse, Error> {
    let notebook_id = store.version()?.notebook_id;
    store.apply(CommandRequest {
        notebook_id,
        revision,
        command,
    })
}
fn create(store: &mut Store, revision: i64, title: &str, body: &str) -> CommandResponse {
    command(
        store,
        revision,
        Command::Create {
            title: title.into(),
            body: body.into(),
        },
    )
    .unwrap()
}

#[test]
fn create_update_delete_and_reopen_persist() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite3");
    let mut store = Store::open_with_seed(&path, &[]).unwrap();
    let added = create(&mut store, 0, "Alpha", "first");
    let id = added.selected_id.unwrap();
    drop(store);
    let mut reopened = Store::open_with_seed(&path, &[]).unwrap();
    let s = reopened.read().unwrap();
    assert_eq!(s.revision, 1);
    assert_eq!(s.notes[0].body, "first");
    let updated = command(
        &mut reopened,
        1,
        Command::Update {
            id: id.clone(),
            body: "second".into(),
        },
    )
    .unwrap();
    assert_eq!(updated.notebook.notes[0].body, "second");
    let deleted = command(&mut reopened, 2, Command::Delete { id }).unwrap();
    assert!(deleted.notebook.notes.is_empty());
    assert_eq!(deleted.notebook.revision, 3);
}

#[test]
fn stale_writes_do_not_overwrite_data() {
    let mut store = Store::memory().unwrap();
    create(&mut store, 0, "Alpha", "kept");
    assert!(matches!(
        command(
            &mut store,
            0,
            Command::Create {
                title: "Lost".into(),
                body: "".into()
            }
        ),
        Err(Error::Conflict)
    ));
    let s = store.read().unwrap();
    assert_eq!(s.revision, 1);
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.notes[0].body, "kept");
}

#[test]
fn simultaneous_connections_allow_exactly_one_revision_winner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite3");
    drop(Store::open_with_seed(&path, &[]).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|i| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = Store::open_with_seed(&path, &[]).unwrap();
                barrier.wait();
                command(
                    &mut store,
                    0,
                    Command::Create {
                        title: format!("note{i}"),
                        body: "".into(),
                    },
                )
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Error::Conflict)))
            .count(),
        1
    );
    assert_eq!(
        Store::open_with_seed(&path, &[])
            .unwrap()
            .read()
            .unwrap()
            .notes
            .len(),
        1
    );
}

#[test]
fn rename_updates_unique_basename_links_and_preserves_code() {
    let mut store = Store::memory().unwrap();
    let id = create(&mut store, 0, "Folder/Beta", "")
        .selected_id
        .unwrap();
    create(&mut store, 1, "Alpha", "[[Beta#Part|label]] `[[Beta]]`");
    let response = command(
        &mut store,
        2,
        Command::Rename {
            id,
            title: "新しい😀".into(),
        },
    )
    .unwrap();
    let source = response
        .notebook
        .notes
        .iter()
        .find(|n| n.title == "Alpha")
        .unwrap();
    assert_eq!(source.body, "[[新しい😀#Part|label]] `[[Beta]]`");
}

#[test]
fn rename_collision_rolls_back_every_note_and_revision() {
    let mut store = Store::memory().unwrap();
    let id = create(&mut store, 0, "Alpha", "[[Beta]]")
        .selected_id
        .unwrap();
    create(&mut store, 1, "Beta", "");
    assert!(matches!(
        command(
            &mut store,
            2,
            Command::Rename {
                id,
                title: "BETA.md".into()
            }
        ),
        Err(Error::TitleExists)
    ));
    let s = store.read().unwrap();
    assert_eq!(s.revision, 2);
    assert!(
        s.notes
            .iter()
            .any(|n| n.title == "Alpha" && n.body == "[[Beta]]")
    );
}

#[test]
fn import_collision_rewrites_links_between_imported_notes() {
    let mut store = Store::memory().unwrap();
    create(&mut store, 0, "Beta", "original");
    let response = command(
        &mut store,
        1,
        Command::Import {
            notes: vec![
                ImportNote {
                    title: "Beta".into(),
                    body: "imported".into(),
                },
                ImportNote {
                    title: "Alpha".into(),
                    body: "[[Beta#Part|alias]]".into(),
                },
            ],
        },
    )
    .unwrap();
    assert_eq!(
        response
            .notebook
            .notes
            .iter()
            .find(|n| n.title == "Beta")
            .unwrap()
            .body,
        "original"
    );
    assert!(
        response
            .notebook
            .notes
            .iter()
            .any(|n| n.title == "Beta (2)")
    );
    assert_eq!(
        response
            .notebook
            .notes
            .iter()
            .find(|n| n.title == "Alpha")
            .unwrap()
            .body,
        "[[Beta (2)#Part|alias]]"
    );
}

#[test]
fn invalid_late_import_item_does_not_partially_import() {
    let mut store = Store::memory().unwrap();
    create(&mut store, 0, "Existing", "");
    let bad = Command::Import {
        notes: vec![
            ImportNote {
                title: "Valid".into(),
                body: "ok".into(),
            },
            ImportNote {
                title: "../Bad".into(),
                body: "bad".into(),
            },
        ],
    };
    assert!(command(&mut store, 1, bad).is_err());
    let s = store.read().unwrap();
    assert_eq!(s.notes.len(), 1);
    assert_eq!(s.revision, 1);
}

#[test]
fn duplicate_import_titles_are_rejected_atomically() {
    let mut store = Store::memory().unwrap();
    assert!(
        command(
            &mut store,
            0,
            Command::Import {
                notes: vec![
                    ImportNote {
                        title: "Café".into(),
                        body: "".into()
                    },
                    ImportNote {
                        title: "Cafe\u{301}".into(),
                        body: "".into()
                    }
                ]
            }
        )
        .is_err()
    );
    assert!(store.read().unwrap().notes.is_empty());
}

#[test]
fn oversized_body_and_missing_id_do_not_mutate() {
    let mut store = Store::memory().unwrap();
    let id = create(&mut store, 0, "Alpha", "keep").selected_id.unwrap();
    assert!(
        command(
            &mut store,
            1,
            Command::Update {
                id,
                body: "x".repeat(MAX_BODY + 1)
            }
        )
        .is_err()
    );
    assert!(matches!(
        command(
            &mut store,
            1,
            Command::Delete {
                id: "missing".into()
            }
        ),
        Err(Error::NotFound)
    ));
    let s = store.read().unwrap();
    assert_eq!(s.revision, 1);
    assert_eq!(s.notes[0].body, "keep");
}

#[test]
fn imported_long_unicode_titles_get_valid_unique_suffixes() {
    let mut store = Store::memory().unwrap();
    let title = "😀".repeat(180);
    create(&mut store, 0, &title, "");
    let r = command(
        &mut store,
        1,
        Command::Import {
            notes: vec![ImportNote {
                title,
                body: "".into(),
            }],
        },
    )
    .unwrap();
    assert_eq!(r.notebook.notes.len(), 2);
    for n in &r.notebook.notes {
        validate_title(&n.title).unwrap();
    }
}

#[test]
fn sql_metacharacters_are_data() {
    let mut store = Store::memory().unwrap();
    create(
        &mut store,
        0,
        "O'Reilly; DROP TABLE notes;--",
        "'); DROP TABLE notes;--",
    );
    assert_eq!(store.read().unwrap().notes.len(), 1);
    create(&mut store, 1, "Still works", "");
    assert_eq!(store.read().unwrap().notes.len(), 2);
}

#[test]
fn incompatible_schema_is_not_silently_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("newer.sqlite3");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 999).unwrap();
    drop(conn);
    assert!(Store::open_with_seed(Path::new(&path), &[]).is_err());
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        999
    );
}
