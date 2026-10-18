use std::{
    fs::{self, File, OpenOptions},
    io::IsTerminal,
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

mod pwa;
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
    let mut open_pwa = true;
    #[cfg(feature = "dev")]
    let mut preview = false;
    #[cfg(feature = "dev")]
    let mut preview_auth = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("Utilizer {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "utilizer [--cs2-path <CS2 installation>] [--no-open-pwa]\nKeep this console open while using the local practice player.\nOpens a separate app window by default (Edge or Chrome required). Use --no-open-pwa to print the URL only.\n\nutilizer uninstall          Remove the app and user PATH entry; keep local data.\nutilizer uninstall --purge  Also delete Utilizer's local data folder.\nClose any running Utilizer player first. Exported CS2 cfg files and bindings are kept."
                );
                #[cfg(feature = "dev")]
                println!(
                    "\nDevelopment: --preview uses isolated data without login. Add --auth to test console-code login."
                );
                return Ok(());
            }
            "--open-pwa" => open_pwa = true,
            // Kept for compatibility; the last launch option wins.
            "--no-open-pwa" | "--no-browser" => open_pwa = false,
            #[cfg(feature = "dev")]
            "--preview" => preview = true,
            #[cfg(feature = "dev")]
            "--auth" => preview_auth = true,
            "--cs2-path" => {
                cs2_path = Some(PathBuf::from(
                    args.next().ok_or("--cs2-path needs a directory")?,
                ))
            }
            _ => return Err(format!("Unknown argument: {arg}. Use --help.").into()),
        }
    }
    #[cfg(feature = "dev")]
    if preview_auth && !preview {
        return Err("--auth is only available with --preview (just dev --auth)".into());
    }
    #[cfg(feature = "dev")]
    if preview && cs2_path.is_some() {
        return Err(
            "--preview cannot use --cs2-path; preview always uses its synthetic installation"
                .into(),
        );
    }
    print_banner();
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
    let (engine, store, _lock, installation) = tokio::task::spawn_blocking(move || {
        #[cfg(feature = "dev")]
        if preview {
            return utilizer::dev::bootstrap(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/dev-preview"),
            );
        }
        bootstrap(cs2_path)
    })
    .await??;
    let shared = Arc::new(Mutex::new(store));
    #[cfg(feature = "dev")]
    let (app, code) = if preview {
        let session =
            std::env::var("UTILIZER_DEV_SESSION").map_err(|_| "Start preview using just dev")?;
        println!("  Development preview — sample data only. No real CS2 files are used.");
        let preview = utilizer::dev::Preview::new(session)?;
        let preview = if preview_auth {
            preview.with_auth()
        } else {
            preview
        };
        App::new_preview(shared, installation, preview)?
    } else {
        App::new(shared, installation)?
    };
    #[cfg(not(feature = "dev"))]
    let (app, code) = App::new(shared, installation)?;
    println!("  Open  {}", web::URL);
    if app.requires_auth() {
        println!("  Code  {code}");
    }
    println!();
    println!("  Running. Keep this terminal open. Ctrl+C to stop.\n");
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
    if open_pwa && let Err(error) = tokio::task::spawn_blocking(pwa::open).await? {
        eprintln!(
            "Cannot open the app window: {error}. Open {} manually.",
            web::URL
        );
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

fn print_banner() {
    let color = std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").as_deref() != Ok("dumb")
        && enable_terminal_color();
    let (gold, reset) = if color {
        ("\x1b[33m", "\x1b[0m")
    } else {
        ("", "")
    };
    println!(
        "\n{gold}  /\\ /\\\n  < X >  Utilizer {}\n  \\/ \\/{reset}\n",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(windows)]
fn enable_terminal_color() -> bool {
    use std::ffi::c_void;
    const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(kind: u32) -> *mut c_void;
        fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
    }
    let mut mode = 0;
    // SAFETY: borrow stdout's OS handle, pass a valid writable mode pointer,
    // and preserve all existing flags. Unsupported handles fail without ANSI.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        GetConsoleMode(handle, &mut mode) != 0
            && SetConsoleMode(
                handle,
                mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
            ) != 0
    }
}

#[cfg(not(windows))]
fn enable_terminal_color() -> bool {
    true
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
    let store = Store::open(directory.join("spots.sqlite3"))?;
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
    store.set_setting("active_map", "")?;
    let engine = Engine::start(&installation.log)?;
    Ok((engine, store, lock, installation))
}
