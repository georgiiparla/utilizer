use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;
use utilizer::{
    Result,
    discovery::{self, Installation},
    engine::Engine,
    storage::Store,
    web::{self, App},
};

mod uninstall;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Utilizer: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().is_some_and(|arg| arg == "uninstall") {
        args.next();
        let purge = match (args.next().as_deref(), args.next()) {
            (None, None) => false,
            (Some("--purge"), None) => true,
            _ => return Err("Usage: utilizer uninstall [--purge]".into()),
        };
        return tokio::task::spawn_blocking(move || uninstall::schedule(purge)).await?;
    }
    let mut cs2_path = None;
    let mut browser = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("Utilizer {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "utilizer [--cs2-path <CS2 installation>] [--no-browser]\nKeep this console open while using the local practice player.\n\nutilizer uninstall          Remove the app and user PATH entry; keep playlists.\nutilizer uninstall --purge  Also delete Utilizer's local data folder.\nClose any running Utilizer player first. Exported CS2 cfg files and bindings are kept."
                );
                return Ok(());
            }
            "--no-browser" => browser = false,
            "--cs2-path" => {
                cs2_path = Some(PathBuf::from(
                    args.next().ok_or("--cs2-path needs a directory")?,
                ))
            }
            _ => return Err(format!("Unknown argument: {arg}. Use --help.").into()),
        }
    }
    println!("Utilizer {}", env!("CARGO_PKG_VERSION"));
    let listener = tokio::net::TcpListener::bind(web::ADDRESS)
        .await
        .map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "Cannot open {}: {error}. Close another Utilizer instance or free this port.",
                    web::URL
                ),
            )
        })?;
    let (engine, store, _lock, installation) =
        tokio::task::spawn_blocking(move || bootstrap(cs2_path)).await??;
    println!("CS2: {}", installation.root.display());
    let (app, code) = App::new(Arc::new(Mutex::new(store)), installation)?;
    println!("\nOpen {}\nConsole code: {code}\n", web::URL);
    println!("CS2 launch options: -condebug -conclearlog");
    println!("Open a practice map, create a playlist in the player, and click Sync to CS2.");
    println!("Then run in CS2: exec console_utilizer_rust/start");
    println!("Keep this console open. Press Ctrl+C to stop.\n");
    let (stop, mut server_stop) = watch::channel(false);
    let mut worker = tokio::spawn(ingest(engine, app.clone(), stop.subscribe()));
    let router = web::router(app.clone());
    let mut server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = server_stop.changed().await;
            })
            .await
    });
    if browser {
        open_browser();
    }
    let mut server_done = false;
    let mut worker_done = false;
    let outcome: Result<()> = tokio::select! {
        result = tokio::signal::ctrl_c() => result.map_err(Into::into),
        result = &mut server => { server_done = true; result.map_err(Into::into).and_then(|r| r.map_err(Into::into)) },
        result = &mut worker => { worker_done = true; result.map_err(Into::into).and_then(|r| r) },
    };
    println!("Stopping...");
    let _ = stop.send(true);
    if !worker_done {
        worker.await??;
    }
    if !server_done {
        server.await??;
    }
    tokio::task::spawn_blocking(move || drop(app)).await?;
    outcome
}

fn open_browser() {
    #[cfg(windows)]
    if let Err(error) = std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(web::URL)
        .spawn()
    {
        eprintln!(
            "Cannot open the browser automatically: {error}. Open {} yourself.",
            web::URL
        );
    }
}

async fn ingest(mut engine: Engine, app: Arc<App>, mut stop: watch::Receiver<bool>) -> Result<()> {
    let wake = engine.monitor.wake_signal();
    let mut recovery = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(2),
        Duration::from_secs(2),
    );
    recovery.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_error = None;
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            () = wake.notified() => {},
            _ = recovery.tick() => {},
        }
        let shared = app.store.clone();
        let log = app.installation.log.clone();
        let result;
        let exists;
        (engine, result, exists) = tokio::task::spawn_blocking(move || {
            let result = match shared.lock() {
                Ok(store) => engine.step(&store, Instant::now()),
                Err(_) => Err("Storage lock failed".into()),
            };
            (engine, result, log.is_file())
        })
        .await?;
        if let Some(error) = engine.monitor.take_watcher_error() {
            eprintln!("File notifications interrupted: {error}. Recovery checks remain active.");
        }
        let status = match result {
            Ok(ids) => {
                if last_error.take().is_some() {
                    println!("Capture reader recovered.");
                }
                for id in ids {
                    println!("Capture received (#{id}). Review it in the player.");
                }
                if exists {
                    "Watching console.log"
                } else {
                    "Waiting for console.log"
                }
                .to_owned()
            }
            Err(error) => {
                let message = error.to_string();
                if last_error.as_ref() != Some(&message) {
                    eprintln!("Cannot read/save captures: {message}. Retrying automatically.");
                    last_error = Some(message);
                }
                "Capture reader retrying — check console".into()
            }
        };
        if let Ok(mut slot) = app.status.lock() {
            *slot = status;
        }
    }
    tokio::task::spawn_blocking(move || drop(engine)).await?;
    Ok(())
}

fn bootstrap(explicit: Option<PathBuf>) -> Result<(Engine, Store, File, Installation)> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is unavailable; this executable targets Windows")?;
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
    let installation = if let Some(path) = explicit {
        Installation::validate(path)?
    } else if let Some(saved) = saved {
        saved
    } else {
        discovery::discover(&discovery::steam_roots()).map_err(|error| format!("{error}\nOr run: utilizer --cs2-path \"C:\\path\\to\\Counter-Strike Global Offensive\""))?
    };
    store.set_setting("cs2_install_root", &installation.root.to_string_lossy())?;
    let engine = Engine::start(&installation.log)?;
    Ok((engine, store, lock, installation))
}
