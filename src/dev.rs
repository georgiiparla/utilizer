//! Opt-in local preview. Never enabled by ordinary builds or normal startup.
use std::{
    collections::hash_map::DefaultHasher,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{Result, discovery::Installation, engine::Engine, invalid, storage::Store, web::App};

pub struct Preview {
    session: String,
    generation: String,
    pub(crate) require_auth: bool,
}

impl Preview {
    pub fn new(session: String) -> Result<Self> {
        if session.len() != 64 || !session.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(
                invalid("Preview needs a random 64-character hexadecimal session token").into(),
            );
        }
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|e| invalid(e.to_string()))?;
        Ok(Self {
            session,
            generation: nonce.iter().map(|b| format!("{b:02x}")).collect(),
            require_auth: false,
        })
    }

    pub fn with_auth(mut self) -> Self {
        self.require_auth = true;
        self
    }

    pub(crate) fn session(&self) -> &str {
        &self.session
    }
}

/// All preview persistence and cfg exports belong to this synthetic installation.
pub fn bootstrap(directory: &Path) -> Result<(Engine, Store, File, Installation)> {
    fs::create_dir_all(directory)?;
    let directory = directory.canonicalize()?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("instance.lock"))?;
    lock.try_lock()
        .map_err(|_| invalid("Another preview is using this data directory"))?;
    let root = directory.join("cs2");
    fs::create_dir_all(root.join("game/bin/win64"))?;
    fs::create_dir_all(root.join("game/csgo/cfg"))?;
    File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("game/bin/win64/cs2.exe"))?;
    let installation = Installation::validate(root)?;
    let store = Store::open(directory.join("spots.sqlite3"))?;
    // This first-version store is independent of earlier prototype databases.
    store.set_setting("active_map", "")?;
    seed_samples(&store)?;
    seed_buffer(&store)?;
    let engine = Engine::start(&installation.log)?;
    Ok((engine, store, lock, installation))
}

// Seed once: deleting samples or editing them must survive watcher restarts.
fn seed_samples(store: &Store) -> Result<()> {
    if store.setting("dev_samples_v1")?.is_some() {
        return Ok(());
    }
    if store.map_spots("de_mirage")?.is_empty() {
        let samples = [
            ("Window smoke · sample", [-1730.0, 88.0, 64.0]),
            ("Connector smoke · sample", [-1630.0, -12.0, 64.0]),
            ("Short flash · sample", [520.0, 113.0, 64.0]),
            ("Bench smoke · sample", [620.0, 63.0, 64.0]),
        ];
        for (name, origin) in samples {
            store.save_map_spot(
                None,
                "de_mirage",
                name,
                None,
                crate::protocol::Position {
                    origin,
                    angles: [0.0, 90.0, 0.0],
                },
            )?;
        }
        // A busy zone and a maximum-length title exercise scrolling and clipping.
        for index in 0..24 {
            let name = if index == 0 {
                format!("Long sample title {}", "W".repeat(142))
            } else {
                format!("A site sample {:02}", index + 1)
            };
            store.save_map_spot(
                None,
                "de_mirage",
                &name,
                None,
                crate::protocol::Position {
                    origin: [
                        20.0 + f64::from(index % 4) * 25.0,
                        -1662.0 + f64::from(index / 4) * 20.0,
                        64.0,
                    ],
                    angles: [0.0, 90.0, 0.0],
                },
            )?;
        }
    }
    store.set_setting("dev_samples_v1", "1")?;
    Ok(())
}

fn seed_buffer(store: &Store) -> Result<()> {
    if store.setting("dev_buffer_v1")?.is_some() {
        return Ok(());
    }
    let has_mirage =
        store
            .list_captures()?
            .iter()
            .try_fold(false, |found, capture| -> Result<bool> {
                Ok(found || store.capture_map(capture.id)?.as_deref() == Some("de_mirage"))
            })?;
    if !has_mirage {
        for origin in [
            [-2030.0, -512.0, 64.0],
            [-1980.0, -562.0, 64.0],
            [-880.0, -637.0, 64.0],
            [-805.0, -712.0, 64.0],
            [1220.0, -537.0, 64.0],
            [1270.0, -637.0, 64.0],
        ] {
            store.record_capture(&crate::protocol::Capture {
                playlist_slug: Some("de_mirage".into()),
                position: crate::protocol::Position {
                    origin,
                    angles: [0.0, 135.0, 0.0],
                },
            })?;
        }
    }
    store.set_setting("dev_buffer_v1", "1")?;
    Ok(())
}

pub(crate) fn routes(router: Router<Arc<App>>, app: &App) -> Router<Arc<App>> {
    if app.preview.is_none() {
        return router;
    }
    router
        .route("/__dev/revision", get(revision))
        .route("/assets/dev.js", get(script))
}

async fn version(app: Arc<App>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let preview = app
            .preview
            .as_ref()
            .ok_or_else(|| invalid("Preview is disabled"))?;
        let mut hash = DefaultHasher::new();
        for file in ["style.css", "app.js", "dev.js", "theme.js"] {
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(file),
            )?
            .hash(&mut hash);
        }
        Ok(format!("{}:{:x}", preview.generation, hash.finish()))
    })
    .await?
}

async fn revision(State(app): State<Arc<App>>) -> Response {
    match version(app).await {
        Ok(version) => version.into_response(),
        Err(error) => {
            eprintln!("Preview reload: {error}");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

async fn script(State(app): State<Arc<App>>) -> Response {
    let result: Result<(String, String)> = async {
        let version = version(app).await?;
        let script = tokio::task::spawn_blocking(|| {
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dev.js"))
        })
        .await??;
        Ok((version, script))
    }
    .await;
    match result {
        Ok((version, script)) => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            format!("const utilizerPreviewVersion = '{version}';\n{script}"),
        )
            .into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

pub(crate) async fn asset(name: &'static str, mime: &'static str) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join(name),
        )
    })
    .await;
    match result {
        Ok(Ok(bytes)) => ([(header::CONTENT_TYPE, mime)], bytes).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(crate) async fn enhance(response: Response) -> Response {
    if !response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"text/html"))
    {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    match to_bytes(body, 2 * 1024 * 1024).await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let text = text.replacen(
                "</head>",
                "<script src=\"/assets/dev.js\" defer></script></head>",
                1,
            );
            parts.headers.remove(header::CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(text))
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
