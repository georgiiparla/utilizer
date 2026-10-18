use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

use tempfile::tempdir;
use utilizer::{
    cfg::{self, CfgFile, GENERATED_HEADER},
    discovery::{self, Installation},
    engine::Engine,
    log::TailReader,
    protocol::{CAPTURE_TIMEOUT, Capture, MAX_LINE_BYTES, Parser, Position},
    storage::Store,
};

const BACKGROUND: &[u8] = include_bytes!("fixtures/background.log");
const SYNTHETIC: &[u8] = include_bytes!("fixtures/synthetic_capture.log");

fn sample_capture() -> Capture {
    Capture {
        playlist_slug: Some("de_mirage_a".into()),
        position: Position {
            origin: [-123.5, 42.0, 64.0],
            angles: [-10.25, 90.0, 0.0],
        },
    }
}

fn append(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
}

#[test]
fn parser_accepts_only_marked_positions_and_ignores_real_noise() {
    let now = Instant::now();
    let mut parser = Parser::new();
    assert!(parser.feed(BACKGROUND, now).is_empty());
    let noise = "10/18 01:09:00  [ALL] Sławek: [UTILIZER_CAPTURE]\n\
        10/18 01:09:00 [Client] [UTILIZER_CAPTURE]\n\
        10/18 01:09:00 [UTILIZER_CAPTURE] extra\n\
        99/99 99:99:99 [UTILIZER_CAPTURE]\n\
        setpos 1 2 3;setang 4 5 6\n";
    assert!(parser.feed(noise.as_bytes(), now).is_empty());
    assert_eq!(parser.feed(SYNTHETIC, now), vec![sample_capture()]);
    assert!(parser.feed(b"setpos 1 2 3;setang 4 5 6\n", now).is_empty());
    assert_eq!(
        parser
            .feed(b"[UTILIZER_CAPTURE]\r\nsetpos 1 2 3; setang 4 5 6\r\n", now)
            .len(),
        1
    );
}

#[test]
fn parser_handles_every_fragment_boundary_and_incomplete_lines() {
    let now = Instant::now();
    for split in 0..=SYNTHETIC.len() {
        let mut parser = Parser::new();
        let mut captures = parser.feed(&SYNTHETIC[..split], now);
        captures.extend(parser.feed(&SYNTHETIC[split..], now));
        assert_eq!(captures, vec![sample_capture()], "split at {split}");
    }
    let mut parser = Parser::new();
    let captures: Vec<_> = SYNTHETIC
        .iter()
        .flat_map(|b| parser.feed(&[*b], now))
        .collect();
    assert_eq!(captures, vec![sample_capture()]);
    assert!(
        parser
            .feed(b"[UTILIZER_CAPTURE]\nsetpos 1 2 3;setang 4 5 6", now)
            .is_empty()
    );
    assert_eq!(parser.feed(b"\n", now).len(), 1);
}

#[test]
fn parser_validates_coordinates_and_rejects_command_injection() {
    for text in [
        "setpos 1 2;setang 4 5 6",
        "setpos 1 2 3 4;setang 4 5 6",
        "setpos NaN 2 3;setang 4 5 6",
        "setpos 1 2 3;setang inf 5 6",
        "setpos 1e999 2 3;setang 4 5 6",
        "setpos 1 2 3;setang 4 5 6;quit",
        "player: setpos 1 2 3;setang 4 5 6",
        "setpos 1 2 3;setang 4 5",
    ] {
        assert!(Position::parse(text).is_none(), "{text}");
    }
    let position = Position::parse("setpos -1.5 2e2 0; setang -89 180 0").unwrap();
    assert_eq!(Position::parse(&position.command()), Some(position));
}

#[test]
fn parser_expires_replaces_and_resets_pending_capture() {
    let now = Instant::now();
    let mut parser = Parser::new();
    parser.feed(b"[UTILIZER_CAPTURE]\n", now);
    assert!(
        parser
            .feed(b"setpos 1 2 3;setang 4 5 6\n", now + CAPTURE_TIMEOUT)
            .is_empty()
    );
    parser.feed(b"[UTILIZER_CAPTURE]\n", now);
    parser.feed(b"[UTILIZER_CAPTURE]\n", now + Duration::from_secs(4));
    assert_eq!(
        parser
            .feed(b"setpos 1 2 3;setang 4 5 6\n", now + Duration::from_secs(6))
            .len(),
        1
    );
    parser.feed(
        b"[UTILIZER_PLAYLIST] a\n[UTILIZER_CAPTURE]\n[UTILIZER_PLAYLIST] b\n",
        now,
    );
    assert!(parser.feed(b"setpos 1 2 3;setang 4 5 6\n", now).is_empty());
    parser.feed(b"[UTILIZER_CAPTURE]\nsetpos 1 2", now);
    parser.reset();
    assert!(parser.feed(b" 3;setang 4 5 6\n", now).is_empty());
    let captures = parser.feed(b"[UTILIZER_CAPTURE]\nsetpos 1 2 3;setang 4 5 6\n", now);
    assert_eq!(captures[0].playlist_slug, None);
}

#[test]
fn parser_bounds_lines_and_survives_bad_bytes() {
    let now = Instant::now();
    let mut parser = Parser::new();
    parser.feed(b"[UTILIZER_CAPTURE]\n", now);
    parser.feed(&vec![b'x'; MAX_LINE_BYTES + 1], now);
    assert!(
        parser
            .feed(b"[UTILIZER_CAPTURE]\nsetpos 1 2 3;setang 4 5 6\n", now)
            .is_empty()
    );
    assert!(
        parser
            .feed(b"\xff[UTILIZER_CAPTURE]\nsetpos 1 2 3;setang 4 5 6\n", now)
            .is_empty()
    );
    assert_eq!(parser.feed(SYNTHETIC, now), vec![sample_capture()]);
}

#[test]
fn reader_starts_at_eof_and_recovers_from_file_lifecycle() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    fs::write(&path, b"old output\n").unwrap();
    let mut reader = TailReader::attach(&path).unwrap();
    assert!(reader.poll().unwrap().bytes.is_empty());
    append(&path, b"first fragment");
    let batch = reader.poll().unwrap();
    assert!(!batch.reset);
    assert_eq!(batch.bytes, b"first fragment");
    assert!(reader.poll().unwrap().bytes.is_empty());
    append(&path, b"\n");
    assert_eq!(reader.poll().unwrap().bytes, b"\n");
    fs::write(&path, b"short\n").unwrap();
    let batch = reader.poll().unwrap();
    assert!(batch.reset);
    assert_eq!(batch.bytes, b"short\n");
    // Same file, truncated and regrown beyond the previous cursor.
    fs::write(&path, b"a longer new stream\n").unwrap();
    let batch = reader.poll().unwrap();
    assert!(batch.reset);
    assert_eq!(batch.bytes, b"a longer new stream\n");
    fs::rename(&path, dir.path().join("archived.log")).unwrap();
    fs::write(&path, b"replacement larger than the old stream\n").unwrap();
    let batch = reader.poll().unwrap();
    assert!(batch.reset);
    assert_eq!(batch.bytes, b"replacement larger than the old stream\n");
    fs::remove_file(&path).unwrap();
    assert!(reader.poll().unwrap().reset);
    assert!(!reader.poll().unwrap().reset);
    fs::write(&path, b"created again\n").unwrap();
    assert_eq!(reader.poll().unwrap().bytes, b"created again\n");
}

#[test]
fn reader_waits_for_creation_and_does_not_advance_on_read_failure() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    let mut reader = TailReader::attach(&path).unwrap();
    assert!(reader.poll().unwrap().bytes.is_empty());
    fs::create_dir(&path).unwrap();
    assert!(reader.poll().is_err());
    fs::remove_dir(&path).unwrap();
    fs::write(&path, b"new stream\n").unwrap();
    let batch = reader.poll().unwrap();
    assert!(batch.reset);
    assert_eq!(batch.bytes, b"new stream\n");
}

#[test]
fn reader_does_not_interpret_the_suffix_of_an_old_partial_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    fs::write(&path, b"old chat: ").unwrap();
    let mut reader = TailReader::attach(&path).unwrap();
    append(&path, b"[UTILIZER_CAP");
    assert!(reader.poll().unwrap().bytes.is_empty());
    append(&path, b"TURE]\nnew complete line\n");
    assert_eq!(reader.poll().unwrap().bytes, b"new complete line\n");
}

#[test]
fn engine_retries_unsaved_captures_after_a_database_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    let db_path = dir.path().join("db");
    let store = Store::open(&db_path).unwrap();
    let mut engine = Engine::start(&path).unwrap();
    let control = rusqlite::Connection::open(&db_path).unwrap();
    control.execute_batch("CREATE TRIGGER reject_capture BEFORE INSERT ON captures BEGIN SELECT RAISE(ABORT, 'simulated storage failure'); END;").unwrap();
    append(&path, SYNTHETIC);
    assert!(engine.step(&store, Instant::now()).is_err());
    assert!(store.list_captures().unwrap().is_empty());
    control
        .execute_batch("DROP TRIGGER reject_capture;")
        .unwrap();
    assert_eq!(engine.step(&store, Instant::now()).unwrap().len(), 1);
    assert!(engine.step(&store, Instant::now()).unwrap().is_empty());
    assert_eq!(store.list_captures().unwrap().len(), 1);
}

#[test]
fn engine_rolls_back_a_partially_failed_capture_batch_before_retrying() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    let db_path = dir.path().join("db");
    let store = Store::open(&db_path).unwrap();
    let mut engine = Engine::start(&path).unwrap();
    let control = rusqlite::Connection::open(&db_path).unwrap();
    control.execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON captures WHEN NEW.x=2 BEGIN SELECT RAISE(ABORT, 'second capture unavailable'); END;").unwrap();
    append(
        &path,
        b"[UTILIZER_CAPTURE]\nsetpos 1 0 0;setang 0 0 0\n[UTILIZER_CAPTURE]\nsetpos 2 0 0;setang 0 0 0\n",
    );
    assert!(engine.step(&store, Instant::now()).is_err());
    assert!(store.list_captures().unwrap().is_empty());

    control
        .execute_batch("DROP TRIGGER reject_second;")
        .unwrap();
    let ids = engine.step(&store, Instant::now()).unwrap();
    let captures = store.list_captures().unwrap();
    assert_eq!(ids, captures.iter().map(|c| c.id).collect::<Vec<_>>());
    assert_eq!(
        captures
            .iter()
            .map(|c| c.capture.position.origin[0])
            .collect::<Vec<_>>(),
        vec![1.0, 2.0]
    );
    assert!(engine.step(&store, Instant::now()).unwrap().is_empty());
}

#[test]
fn engine_reports_retried_captures_before_attempting_another_log_read() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("console.log");
    let db_path = dir.path().join("db");
    let store = Store::open(&db_path).unwrap();
    let mut engine = Engine::start(&path).unwrap();
    let control = rusqlite::Connection::open(&db_path).unwrap();
    control.execute_batch("CREATE TRIGGER reject_capture BEFORE INSERT ON captures BEGIN SELECT RAISE(ABORT, 'storage unavailable'); END;").unwrap();
    append(&path, SYNTHETIC);
    assert!(engine.step(&store, Instant::now()).is_err());
    control
        .execute_batch("DROP TRIGGER reject_capture;")
        .unwrap();
    fs::rename(&path, dir.path().join("archived.log")).unwrap();
    fs::create_dir(&path).unwrap();

    let ids = engine.step(&store, Instant::now()).unwrap();
    assert_eq!(ids, vec![store.list_captures().unwrap()[0].id]);
    assert!(engine.step(&store, Instant::now()).is_err());
    assert_eq!(store.list_captures().unwrap().len(), 1);
}

#[test]
fn database_migration_persistence_ordering_and_acceptance_are_atomic() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("utilizer.sqlite3");
    let mut store = Store::open(&path).unwrap();
    assert!(store.list_playlists().unwrap().is_empty());
    store
        .set_setting("quoted'key", "value'; DROP TABLE playlists;--")
        .unwrap();
    let playlist = store
        .create_playlist("de_mirage", "de_mirage_a", "A executes")
        .unwrap();
    let first = store.record_capture(&sample_capture()).unwrap();
    // Foreign-key failure must leave the capture pending and no partial spot.
    assert!(store.accept_capture(first, 999, "Window", None).is_err());
    assert_eq!(store.list_captures().unwrap().len(), 1);
    let spot_id = store
        .accept_capture(
            first,
            playlist.id,
            "Window",
            Some("https://example.com/lineup"),
        )
        .unwrap();
    assert_eq!(
        store
            .accept_capture(first, playlist.id, "Window", None)
            .unwrap(),
        spot_id
    );
    assert!(store.list_captures().unwrap().is_empty());
    let second = store.record_capture(&sample_capture()).unwrap();
    store
        .accept_capture(second, playlist.id, "Stairs", None)
        .unwrap();
    let other = store
        .create_playlist("de_mirage", "other", "Other")
        .unwrap();
    assert!(
        store
            .accept_capture(first, other.id, "Moved", None)
            .is_err()
    );
    assert!(
        store
            .accept_capture(500, playlist.id, "Absent", None)
            .is_err()
    );
    assert!(store.accept_capture(second, playlist.id, "", None).is_err());
    assert!(
        store
            .accept_capture(second, playlist.id, "Link", Some("javascript:alert(1)"))
            .is_err()
    );
    let mut bad = sample_capture();
    bad.position.origin[0] = f64::NAN;
    assert!(store.record_capture(&bad).is_err());
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.setting("quoted'key").unwrap().as_deref(),
        Some("value'; DROP TABLE playlists;--")
    );
    assert_eq!(
        store.list_playlists().unwrap(),
        vec![playlist.clone(), other]
    );
    let spots = store.load_spots(playlist.id).unwrap();
    assert_eq!(
        spots.iter().map(|s| s.ordinal).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(spots[0].description, "Window");
    assert_eq!(spots[0].position, sample_capture().position);
}

#[test]
fn database_refuses_future_schema_without_rewriting_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("future.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("PRAGMA user_version=99;").unwrap();
    drop(connection);
    assert!(Store::open(&path).is_err());
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        99
    );
}

#[test]
fn cfg_output_is_deterministic_and_descriptions_cannot_inject_commands() {
    let dir = tempdir().unwrap();
    let mut store = Store::open(dir.path().join("db")).unwrap();
    let playlist = store
        .create_playlist("de_mirage", "de_mirage_a", "A executes")
        .unwrap();
    let id = store.record_capture(&sample_capture()).unwrap();
    store
        .accept_capture(id, playlist.id, "Window\nquit\r\necholn evil", None)
        .unwrap();
    let spots = store.load_spots(playlist.id).unwrap();
    let files = cfg::render(&playlist, &spots).unwrap();
    assert_eq!(files[0].filename, "playlist_1.cfg");
    assert_eq!(
        files[0].content,
        format!(
            "{GENERATED_HEADER}\nsv_cheats 1\necholn \"[UTILIZER_PLAYLIST] de_mirage_a\"\n// 1: Window quit  echoln evil\nsetpos -123.5 42 64;setang -10.25 90 0\n"
        )
    );
    assert_eq!(
        files[1].content,
        format!("{GENERATED_HEADER}\nsv_cheats 1\necholn \"[UTILIZER_CAPTURE]\"\ngetpos\n")
    );
    let mut wrong = spots.clone();
    wrong[0].playlist_id = 999;
    assert!(cfg::render(&playlist, &wrong).is_err());
    assert!(cfg::render(&playlist, &[spots[0].clone(), spots[0].clone()]).is_err());
}

#[test]
fn cfg_export_protects_unrelated_files_and_replaces_only_owned_output() {
    let dir = tempdir().unwrap();
    let files = vec![CfgFile {
        filename: "playlist_1.cfg".into(),
        content: format!("{GENERATED_HEADER}\nsetpos 1 2 3;setang 4 5 6\n"),
    }];
    let paths = cfg::export(dir.path(), &files).unwrap();
    cfg::export(dir.path(), &files).unwrap();
    assert_eq!(fs::read_to_string(&paths[0]).unwrap(), files[0].content);
    fs::write(&paths[0], "// Player owned\n").unwrap();
    assert!(cfg::export(dir.path(), &files).is_err());
    assert_eq!(fs::read_to_string(&paths[0]).unwrap(), "// Player owned\n");
    for name in ["../autoexec.cfg", "C:escape.cfg", "nested\\evil.cfg"] {
        let invalid = vec![CfgFile {
            filename: name.into(),
            content: format!("{GENERATED_HEADER}\n"),
        }];
        assert!(cfg::export(dir.path(), &invalid).is_err());
    }
    assert!(!dir.path().join("autoexec.cfg").exists());
}

fn fake_install(root: &Path) {
    fs::create_dir_all(root.join("game/bin/win64")).unwrap();
    fs::create_dir_all(root.join("game/csgo/cfg")).unwrap();
    File::create(root.join("game/bin/win64/cs2.exe")).unwrap();
}

#[test]
fn discovery_reads_steam_metadata_and_validates_installation() {
    let dir = tempdir().unwrap();
    let steam = dir.path().join("Steam");
    let library = dir.path().join("Other Library");
    fs::create_dir_all(steam.join("steamapps")).unwrap();
    fs::create_dir_all(library.join("steamapps")).unwrap();
    let escaped = library.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        steam.join("steamapps/libraryfolders.vdf"),
        format!("\"libraryfolders\" {{ \"0\" {{ \"path\" \"{escaped}\" }} }}"),
    )
    .unwrap();
    fs::write(
        library.join("steamapps/appmanifest_730.acf"),
        "\"AppState\" { \"appid\" \"730\" \"installdir\" \"Counter-Strike Global Offensive\" }",
    )
    .unwrap();
    let root = library.join("steamapps/common/Counter-Strike Global Offensive");
    fake_install(&root);
    let installation = discovery::discover(std::slice::from_ref(&steam)).unwrap();
    assert_eq!(installation, Installation::validate(&root).unwrap());
    assert_eq!(
        installation.log,
        installation.root.join("game/csgo/console.log")
    );
    fs::remove_file(root.join("game/bin/win64/cs2.exe")).unwrap();
    assert!(discovery::discover(std::slice::from_ref(&steam)).is_err());
    fake_install(&root);
    fs::write(
        library.join("steamapps/appmanifest_730.acf"),
        "\"AppState\" { \"appid\" \"730\" \"installdir\" \"../escape\" }",
    )
    .unwrap();
    assert!(discovery::discover(&[steam]).is_err());
}

/// No interval, reader polling loop, or recovery tick: only an actual OS
/// notification can wake this integration test.
async fn capture_from_callback(engine: &mut Engine, store: &Store) -> Vec<i64> {
    let wake = engine.monitor.wake_signal();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            wake.notified().await;
            let ids = engine.step(store, Instant::now()).unwrap();
            assert!(engine.monitor.take_watcher_error().is_none());
            if !ids.is_empty() {
                return ids;
            }
        }
    })
    .await
    .expect("Native filesystem callback did not deliver a capture")
}

#[tokio::test(flavor = "current_thread")]
async fn native_callback_to_sqlite_to_cfg_without_recovery_polling() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("fake-cs2");
    fake_install(&root);
    let installation = Installation::validate(&root).unwrap();
    let db = dir.path().join("utilizer.sqlite3");
    let mut store = Store::open(&db).unwrap();
    let playlist = store
        .create_playlist("de_mirage", "de_mirage_a", "A executes")
        .unwrap();
    // Watcher registration completes before any log creation or writes.
    let mut engine = Engine::start(&installation.log).unwrap();
    append(&installation.log, BACKGROUND);
    // Separate writes intentionally split the marker. They may be coalesced
    // by Windows; the parser's boundary tests cover every possible split.
    let split = 70.min(SYNTHETIC.len());
    append(&installation.log, &SYNTHETIC[..split]);
    append(&installation.log, &SYNTHETIC[split..]);
    let ids = capture_from_callback(&mut engine, &store).await;
    assert_eq!(ids.len(), 1);
    assert_eq!(store.list_captures().unwrap()[0].capture, sample_capture());
    store
        .accept_capture(ids[0], playlist.id, "Window smoke", None)
        .unwrap();
    // Replacement with a larger file must reset identity/cursor and capture
    // once again, even when the position itself has not changed.
    fs::rename(&installation.log, root.join("archived.log")).unwrap();
    append(&installation.log, BACKGROUND);
    append(&installation.log, SYNTHETIC);
    let replacement = capture_from_callback(&mut engine, &store).await;
    assert_eq!(replacement.len(), 1);
    store
        .accept_capture(replacement[0], playlist.id, "Window again", None)
        .unwrap();
    drop(store);
    let store = Store::open(&db).unwrap();
    let spots = store.load_spots(playlist.id).unwrap();
    assert_eq!(spots.len(), 2);
    let files = cfg::render(&playlist, &spots).unwrap();
    let paths = cfg::export(&installation.cfg, &files).unwrap();
    assert_eq!(fs::read_to_string(&paths[0]).unwrap(), files[0].content);
    assert!(store.list_captures().unwrap().is_empty());
    assert!(!installation.cfg.join("autoexec.cfg").exists());
}
