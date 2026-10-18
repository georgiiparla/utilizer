use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    time::{Duration, Instant},
};

use utilizer::{
    Result,
    discovery::{self, Installation},
    engine::Engine,
    storage::Store,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Utilizer: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    println!("Utilizer v{}", env!("CARGO_PKG_VERSION"));
    let (mut engine, store, _lock, installation) = tokio::task::spawn_blocking(bootstrap).await??;
    println!("CS2 found: {}", installation.root.display());
    println!("Listening for captures: {}", installation.log.display());
    if !installation.log.exists() {
        println!("Waiting for console.log. CS2 needs -condebug -conclearlog launch options.");
    }
    println!("Press Ctrl+C to stop.");
    let wake = engine.monitor.wake_signal();
    let mut recovery = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(2),
        Duration::from_secs(2),
    );
    recovery.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    let mut store = store;
    let mut last_error = None;
    loop {
        tokio::select! {
            result = &mut shutdown => { result?; break; },
            () = wake.notified() => {},
            _ = recovery.tick() => {},
        }
        let result;
        (engine, store, result) = tokio::task::spawn_blocking(move || {
            let result = engine.step(&store, Instant::now());
            (engine, store, result)
        })
        .await?;
        if let Some(error) = engine.monitor.take_watcher_error() {
            eprintln!("File notifications interrupted: {error}. Recovery checks remain active.");
        }
        match result {
            Ok(ids) => {
                if last_error.take().is_some() {
                    println!("Capture reader recovered.");
                }
                for id in ids {
                    println!("Capture received (#{id}).");
                }
            }
            Err(error) => {
                let message = error.to_string();
                if last_error.as_ref() != Some(&message) {
                    eprintln!("Cannot read/save captures: {message}. Retrying automatically.");
                    last_error = Some(message);
                }
            }
        }
    }
    println!("Stopping...");
    // Drop watcher and SQLite before releasing the per-user instance lock.
    tokio::task::spawn_blocking(move || {
        drop(engine);
        drop(store);
    })
    .await?;
    Ok(())
}

fn bootstrap() -> Result<(Engine, Store, File, Installation)> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is unavailable; this executable currently targets Windows")?;
    let directory = PathBuf::from(base).join("Utilizer");
    fs::create_dir_all(&directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("instance.lock"))?;
    lock.try_lock().map_err(|error| {
        std::io::Error::other(format!(
            "Cannot acquire instance lock (Utilizer may already be running): {error}"
        ))
    })?;
    let store = Store::open(directory.join("utilizer.sqlite3"))?;
    let saved = store
        .setting("cs2_install_root")?
        .and_then(|path| Installation::validate(path).ok());
    let installation = match saved {
        Some(installation) => installation,
        None => discovery::discover(&discovery::steam_roots())?,
    };
    store.set_setting("cs2_install_root", &installation.root.to_string_lossy())?;
    let engine = Engine::start(&installation.log)?;
    Ok((engine, store, lock, installation))
}
