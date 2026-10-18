//! Exercise the actual HTTP adapter without a browser or a real CS2 installation.
use std::{
    fs,
    future::IntoFuture,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

use tempfile::tempdir;
use utilizer::{
    discovery::Installation,
    protocol::{Capture, Position},
    storage::Store,
    web::{self, App},
};

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

async fn request(address: SocketAddr, method: &str, path: &str, extra: &str, body: &str) -> Reply {
    let host = if extra.starts_with("Host: ") {
        String::new()
    } else {
        format!("Host: {}\r\n", web::ADDRESS)
    };
    let wire = format!(
        "{method} {path} HTTP/1.1\r\n{host}Connection: close\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n{extra}\r\n{body}",
        body.len()
    );
    tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(wire.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        Reply {
            status: headers
                .split_ascii_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap(),
            headers: headers.to_ascii_lowercase(),
            body: body.to_owned(),
        }
    })
    .await
    .unwrap()
}

fn fixture(root: &std::path::Path) -> Installation {
    fs::create_dir_all(root.join("game/bin/win64")).unwrap();
    fs::create_dir_all(root.join("game/csgo/cfg")).unwrap();
    fs::write(root.join("game/bin/win64/cs2.exe"), []).unwrap();
    Installation::validate(root).unwrap()
}

fn capture(slug: Option<&str>, origin: [f64; 3]) -> Capture {
    Capture {
        playlist_slug: slug.map(str::to_owned),
        position: Position {
            origin,
            angles: [0.0; 3],
        },
    }
}

fn fields(values: &[(&str, &str)]) -> String {
    fn encoded(value: &str) -> String {
        value
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                    char::from(byte).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect()
    }
    values
        .iter()
        .map(|(key, value)| format!("{}={}", encoded(key), encoded(value)))
        .collect::<Vec<_>>()
        .join("&")
}

// Submit the editor's current revision as an HTML form would. Individual stale
// revision checks below deliberately use raw requests instead.
async fn managed(address: SocketAddr, auth: &str, body: &str) -> Reply {
    let value = |key: &str| {
        body.split('&')
            .find_map(|part| {
                part.split_once('=')
                    .filter(|(k, _)| *k == key)
                    .map(|(_, v)| v)
            })
            .unwrap_or("")
    };
    let kind = value("kind");
    let mut body = body.to_owned();
    if kind == "spot" && !value("point").is_empty() {
        let path = format!(
            "/editor?kind={kind}&map={}&point={}",
            value("map"),
            value("point")
        );
        let editor = request(address, "GET", &path, auth, "").await;
        if let Some(stamp) = editor
            .body
            .split("name=\"stamp\" value=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
        {
            body.push_str("&stamp=");
            body.push_str(stamp);
        }
    }
    request(address, "POST", "/manage", auth, &body).await
}

#[tokio::test(flavor = "current_thread")]
async fn editor_crud_capture_review_and_sync_validate_inputs_and_preserve_game_files() {
    let directory = tempdir().unwrap();
    let installation = fixture(&directory.path().join("cs2"));
    let root = installation.cfg.clone();
    let output = root.join("console_utilizer_rust");
    let store = Arc::new(Mutex::new(
        Store::open(directory.path().join("test.sqlite3")).unwrap(),
    ));
    let (app, code) = App::new(store.clone(), installation).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, web::router(app)).into_future());
    assert_eq!(
        request(address, "GET", "/editor?kind=playlist", "", "")
            .await
            .status,
        303
    );
    let origin = format!("Origin: {}\r\n", web::URL);
    let login = request(address, "POST", "/login", &origin, &format!("code={code}")).await;
    let cookie = login
        .headers
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let auth = format!("Cookie: {cookie}\r\n{origin}HX-Request: true\r\n");
    let bad_origin =
        format!("Cookie: {cookie}\r\nOrigin: https://example.com\r\nHX-Request: true\r\n");
    let position = "setpos -730 -787 64;setang 0 90 0";
    let form = fields(&[
        ("kind", "spot"),
        ("map", "de_mirage"),
        ("action", "save"),
        ("name", "<img src=x onerror=alert(1)>"),
        ("position", position),
    ]);
    assert_eq!(
        request(address, "POST", "/manage", &bad_origin, &form)
            .await
            .status,
        403
    );
    assert_eq!(managed(address, &auth, &form).await.status, 200);
    assert!(!output.exists());
    assert!(store.lock().unwrap().list_playlists().unwrap().is_empty());
    assert!(store.lock().unwrap().list_captures().unwrap().is_empty());
    let spot = store.lock().unwrap().map_spots("de_mirage").unwrap()[0].clone();
    let point = format!("spot-{}", spot.id);
    let edit = request(
        address,
        "GET",
        &format!("/editor?kind=spot&map=de_mirage&point={point}"),
        &auth,
        "",
    )
    .await;
    assert_eq!(edit.status, 200);
    assert!(edit.body.contains("&#60;img"));
    assert!(!edit.body.contains("name=\"playlist\""));
    assert!(!edit.body.contains("data-close-editor"));
    for (name, pos, link) in [
        ("Bad".into(), "setpos 0 0 0;setang 0 0 0;quit", ""),
        ("Bad".into(), "setpos NaN 0 0;setang 0 0 0", ""),
        ("x".repeat(161), position, ""),
        ("Line\nname".into(), position, ""),
        ("Bad".into(), position, "javascript:alert(1)"),
        ("Bad".into(), position, "https://user:secret@example.com"),
    ] {
        let bad = fields(&[
            ("kind", "spot"),
            ("map", "de_mirage"),
            ("point", &point),
            ("action", "save"),
            ("name", &name),
            ("position", pos),
            ("link", link),
        ]);
        assert_eq!(managed(address, &auth, &bad).await.status, 400);
        assert_eq!(store.lock().unwrap().map_spot(spot.id).unwrap(), spot);
    }
    let update = fields(&[
        ("kind", "spot"),
        ("map", "de_mirage"),
        ("point", &point),
        ("action", "save"),
        ("name", "Updated"),
        ("position", position),
    ]);
    let old_stamp = edit
        .body
        .split("name=\"stamp\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(managed(address, &auth, &update).await.status, 200);
    assert_eq!(
        request(
            address,
            "POST",
            "/manage",
            &auth,
            &format!("{update}&stamp={old_stamp}")
        )
        .await
        .status,
        400
    );
    let wrong_map = format!("kind=spot&map=de_inferno&point={point}&action=delete");
    assert_eq!(managed(address, &auth, &wrong_map).await.status, 400);
    assert_eq!(
        managed(
            address,
            &auth,
            "kind=playlist&map=de_mirage&name=No&action=save"
        )
        .await
        .status,
        400
    );
    assert_eq!(
        request(address, "GET", "/editor?kind=playlist", &auth, "")
            .await
            .status,
        400
    );
    let pending = store
        .lock()
        .unwrap()
        .record_capture(&capture(Some("de_mirage"), [-730.0, -787.0, 64.0]))
        .unwrap();
    assert_eq!(
        managed(
            address,
            &auth,
            &format!("kind=sync&map=de_mirage&point=capture-{pending}")
        )
        .await
        .status,
        400
    );
    let accept = fields(&[
        ("kind", "spot"),
        ("map", "de_mirage"),
        ("point", &format!("capture-{pending}")),
        ("action", "save"),
        ("name", "Captured"),
        ("position", position),
    ]);
    assert_eq!(managed(address, &auth, &accept).await.status, 200);
    assert_eq!(managed(address, &auth, &accept).await.status, 400);
    assert_eq!(
        store.lock().unwrap().map_spots("de_mirage").unwrap().len(),
        2
    );
    for action in ["copy", "up", "down", "save-sync"] {
        let removed = update.replace("action=save", &format!("action={action}"));
        assert_eq!(managed(address, &auth, &removed).await.status, 400);
    }
    for body in [
        "kind=sync&map=de_mirage&point=unknown",
        "kind=sync&map=de_mirage&mode=unknown",
        "kind=sync&map=de_mirage&mode=captures",
    ] {
        let response = managed(address, &auth, body).await;
        assert_eq!(response.status, 400);
        assert!(response.body.contains("data-important"));
        assert!(!response.body.contains("Back to map"));
    }
    let other = store
        .lock()
        .unwrap()
        .save_map_spot(
            None,
            "de_inferno",
            "Other map",
            None,
            capture(None, [0.0, 0.0, 64.0]).position,
        )
        .unwrap();
    let sync = format!("kind=sync&map=de_mirage&point={point}");
    assert_eq!(managed(address, &auth, &sync).await.status, 200);
    assert!(!root.join(format!("ut_{other}.cfg")).exists());
    assert!(!output.exists());
    assert!(root.join("cfg.cfg").exists());
    let state = fs::read_to_string(root.join("cfg.cfg")).unwrap();
    assert!(state.contains("alias ut_current ut_1"));
    assert!(state.contains("say_team [1/2] Updated"));
    assert!(!state.contains("Other map"));
    assert!(!root.join("ut_2.cfg").exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::write(root.join("cfg.cfg"), "unrelated").unwrap();
    assert_eq!(managed(address, &auth, &sync).await.status, 400);
    assert_eq!(
        fs::read_to_string(root.join("cfg.cfg")).unwrap(),
        "unrelated"
    );
    let delete = format!("kind=spot&map=de_mirage&point={point}&action=delete");
    assert_eq!(managed(address, &auth, &delete).await.status, 200);
    assert_eq!(managed(address, &auth, &delete).await.status, 400);
    let home = request(address, "GET", "/", &auth, "").await;
    assert!(!home.body.contains("playlist"));
    assert!(!home.body.contains("<summary>Test</summary>"));
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "current_thread")]
async fn map_login_projection_unknown_maps_escaping_errors_and_logout() {
    let directory = tempdir().unwrap();
    let installation = fixture(&directory.path().join("cs2"));
    let output = installation.cfg.join("console_utilizer_rust");
    let database = directory.path().join("test.sqlite3");
    let store = Store::open(&database).unwrap();
    store
        .save_map_spot(
            None,
            "de_mirage",
            "<img src=x onerror=alert(1)>",
            None,
            capture(None, [0.0, 0.0, 64.0]).position,
        )
        .unwrap();
    let id = store
        .record_capture(&capture(Some("de_mirage"), [-730.0, -787.0, 64.0]))
        .unwrap();
    let unknown = store.record_capture(&capture(None, [0.0; 3])).unwrap();
    let outside = store
        .record_capture(&capture(Some("de_mirage"), [99999.0, 0.0, 0.0]))
        .unwrap();
    let other = store
        .record_capture(&capture(Some("de_inferno"), [0.0; 3]))
        .unwrap();
    let (app, code) = App::new(Arc::new(Mutex::new(store)), installation).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, web::router(app)).into_future());
    let reply = request(address, "GET", "/login", "", "").await;
    assert_eq!(reply.status, 200);
    assert!(reply.headers.contains("referrer-policy: same-origin"));
    assert!(reply.body.contains("<h1>Utilizer</h1>"));
    assert!(reply.body.contains("placeholder=\"Code\""));
    assert!(!reply.body.contains("Open the map"));
    assert!(!reply.body.contains("Unlock"));
    assert!(!reply.body.contains("<label"));
    assert!(reply.headers.contains("manifest-src 'self'"));
    assert!(reply.body.contains("href=\"/manifest.webmanifest\""));
    assert!(!reply.body.contains("/assets/dev.js"));
    let manifest = request(address, "GET", "/manifest.webmanifest", "", "").await;
    assert_eq!(manifest.status, 200);
    assert!(manifest.headers.contains("application/manifest+json"));
    assert!(manifest.body.contains("\"display\": \"standalone\""));
    assert!(manifest.body.contains("\"start_url\": \"/\""));
    for size in [192, 512] {
        let icon = request(address, "HEAD", &format!("/assets/icon-{size}.png"), "", "").await;
        assert_eq!(icon.status, 200);
        assert!(icon.headers.contains("content-type: image/png"));
    }
    assert_eq!(
        request(address, "GET", "/assets/dev.js", "", "")
            .await
            .status,
        404
    );
    assert_eq!(
        request(address, "GET", "/", "HX-Request: true\r\n", "")
            .await
            .status,
        401
    );
    for origin in ["", "Origin: null\r\n", "Origin: https://example.com\r\n"] {
        assert_eq!(
            request(address, "POST", "/login", origin, &format!("code={code}"))
                .await
                .status,
            403
        );
    }
    let origin = format!("Origin: {}\r\n", web::URL);
    assert_eq!(
        request(address, "POST", "/login", &origin, "code=wrong")
            .await
            .status,
        401
    );
    let reply = request(address, "POST", "/login", &origin, &format!("code={code}")).await;
    assert_eq!(reply.status, 303);
    let cookie = reply
        .headers
        .lines()
        .find_map(|l| l.strip_prefix("set-cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let authenticated = format!(
        "Cookie: {cookie}\r\nOrigin: {}\r\nHX-Request: true\r\n",
        web::URL
    );
    let reply = request(address, "GET", "/", &authenticated, "").await;
    assert!(reply.body.contains("/assets/maps/mirage.svg"));
    assert!(reply.body.contains("clipPathUnits=\"userSpaceOnUse\""));
    assert!(reply.body.contains("clip-rule=\"evenodd\""));
    assert!(
        reply
            .body
            .contains("<g clip-path=\"url(#zone-map-outline)\">")
    );
    assert!(reply.body.contains("Area with"));
    assert!(reply.body.contains("h 48 v 48 h -48 Z"));
    assert!(!reply.body.contains("class=\"spot"));
    assert!(!reply.body.contains(">All</a>"));
    let reply = request(address, "GET", "/?area=spot-1", &authenticated, "").await;
    assert!(reply.body.contains("&#60;img src=x onerror=alert(1)&#62;"));
    assert!(!reply.body.contains("<img src=x"));
    assert!(!reply.body.contains("<summary>Test</summary>"));
    assert!(!reply.body.contains("Sync to CS2"));
    assert!(reply.body.contains(">Add spot</button>"));
    assert!(reply.body.contains(">Mode: Saved</option>"));
    assert!(reply.body.contains("data-mode-select"));
    assert!(reply.body.contains("data-editor=\"help\">Help</button>"));
    assert!(!reply.body.contains(">Sync</button>"));
    assert!(!reply.body.contains("<title>1 spots"));
    assert!(!reply.body.contains("data-preview"));
    assert!(!reply.body.contains("Capture #"));
    assert!(!output.exists());
    let reply = request(
        address,
        "GET",
        &format!("/locations?mode=captures&area=capture-{id}&point=capture-{id}"),
        &authenticated,
        "",
    )
    .await;
    assert!(reply.body.contains("data-mode=\"captures\""));
    assert!(reply.body.contains(">Mode: Captures</option>"));
    assert!(!reply.body.contains("<img src=x"));
    assert!(!reply.body.contains("&#60;img"));
    assert!(reply.body.contains("cx=\"500\" cy=\"500\""));
    assert!(reply.body.contains("Area with"));
    assert!(!reply.body.contains("<!doctype"));
    let revision = reply
        .body
        .split("\"revision\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        request(
            address,
            "GET",
            &format!(
                "/locations?mode=captures&area=capture-{id}&point=capture-{id}&revision={revision}"
            ),
            &authenticated,
            ""
        )
        .await
        .status,
        204
    );
    for (id, reason) in [(unknown, "Map unknown"), (outside, "Outside map bounds")] {
        let reply = request(
            address,
            "GET",
            &format!("/locations?mode=captures&point=capture-{id}"),
            &authenticated,
            "",
        )
        .await;
        assert!(!reply.body.contains(reason));
        assert!(!reply.body.contains("class=\"spot"));
        assert!(
            !reply
                .body
                .contains(&format!("aria-label=\"Select Capture #{id}\""))
        );
    }
    let reply = request(
        address,
        "GET",
        &format!("/locations?map=de_inferno&mode=captures&area=capture-{other}"),
        &authenticated,
        "",
    )
    .await;
    assert!(reply.body.contains(&format!("Capture #{other}")));
    assert!(!reply.body.contains(&format!("Capture #{id}")));
    let reply = request(
        address,
        "GET",
        "/locations?map=de_train",
        &authenticated,
        "",
    )
    .await;
    assert!(reply.body.contains("/assets/maps/mirage.svg"));
    assert!(reply.body.contains("data-theme-select"));
    for removed in ["de_train", "de_overpass", "de_vertigo"] {
        assert!(!reply.body.contains(&format!("value=\"{removed}\"")));
        assert_eq!(
            request(
                address,
                "GET",
                &format!("/assets/maps/{}.svg", removed.trim_start_matches("de_")),
                "",
                ""
            )
            .await
            .status,
            404
        );
    }
    let reply = request(address, "GET", "/assets/maps/mirage.svg", "", "").await;
    assert_eq!(reply.status, 200);
    assert!(reply.headers.contains("image/svg+xml"));
    for map in &utilizer::maps::MAPS {
        let name = map.code.trim_start_matches("de_");
        let dark = request(
            address,
            "GET",
            &format!("/assets/maps/{name}-dark.svg"),
            "",
            "",
        )
        .await;
        assert_eq!(dark.status, 200);
        assert!(dark.headers.contains("image/svg+xml"));
        assert!(dark.body.contains("<svg"));
    }
    let theme = request(address, "GET", "/assets/theme.js", "", "").await;
    assert_eq!(theme.status, 200);
    assert!(theme.headers.contains("text/javascript"));
    for (method, path) in [
        ("POST", "/action"),
        ("GET", "/captures"),
        ("POST", "/__dev/capture"),
    ] {
        assert_eq!(
            request(address, method, path, &authenticated, "")
                .await
                .status,
            404
        );
    }
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE captures")
        .unwrap();
    let reply = request(
        address,
        "GET",
        "/locations?mode=captures",
        &authenticated,
        "",
    )
    .await;
    assert_eq!(reply.status, 400);
    assert!(reply.headers.contains("hx-retarget: #feedback"));
    assert_eq!(
        request(address, "POST", "/logout", &authenticated, "")
            .await
            .status,
        303
    );
    assert_eq!(
        request(address, "GET", "/locations", &authenticated, "")
            .await
            .status,
        401
    );
    server.abort();
    let _ = server.await;
}

#[cfg(feature = "dev")]
#[tokio::test(flavor = "current_thread")]
async fn preview_reads_capture_fixtures_without_a_test_feature_and_filters_zones() {
    use std::{io::Write, time::Instant};
    use utilizer::dev::{self, Preview};
    let directory = tempdir().unwrap();
    let token = "ab".repeat(32);
    assert!(Preview::new("bad".into()).is_err());
    let (mut engine, store, lock, installation) = dev::bootstrap(directory.path()).unwrap();
    assert!(dev::bootstrap(directory.path()).is_err());
    // Keep this log-reader fixture separate from the UI demonstration buffer.
    for capture in store.list_captures().unwrap() {
        store.discard_capture(capture.id).unwrap();
    }

    let log = installation.log.clone();
    let store = Arc::new(Mutex::new(store));
    let (app, _) = App::new_preview(
        store.clone(),
        installation.clone(),
        Preview::new(token.clone()).unwrap().with_auth(),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, web::router(app)).into_future());
    let auth = format!(
        "Cookie: utilizer_session={token}\r\nOrigin: {}\r\nHX-Request: true\r\n",
        web::URL
    );
    let page = request(address, "GET", "/", &auth, "").await;
    assert_eq!(page.status, 200);
    assert!(!page.body.contains("<summary>Test</summary>"));
    assert!(page.body.contains("/assets/dev.js"));
    assert_eq!(
        request(address, "POST", "/__dev/capture", &auth, "samples=yes")
            .await
            .status,
        404
    );
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(log)
        .unwrap();
    writeln!(
        file,
        "\n[Client] Map: \"de_mirage\"\n[UTILIZER_CAPTURE]\nsetpos -730 -787 64;setang 0 90 0"
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if !engine
            .step(&store.lock().unwrap(), Instant::now())
            .unwrap()
            .is_empty()
        {
            break;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let received = store.lock().unwrap().list_captures().unwrap()[0].id;
    let filtered = request(
        address,
        "GET",
        &format!("/locations?mode=captures&area=capture-{received}"),
        &auth,
        "",
    )
    .await;
    assert!(filtered.body.contains("cx=\"500\" cy=\"500\""));
    assert!(filtered.body.contains(&format!("Capture #{received}")));
    assert!(!installation.cfg.join("console_utilizer_rust").exists());
    let (first, nearby, distant) = {
        let guard = store.lock().unwrap();
        let first = guard
            .record_capture(&capture(Some("de_mirage"), [-1730.0, 213.0, 64.0]))
            .unwrap();
        let nearby = guard
            .record_capture(&capture(Some("de_mirage"), [-1630.0, 113.0, 64.0]))
            .unwrap();
        let distant = guard
            .record_capture(&capture(Some("de_mirage"), [270.0, -2037.0, 64.0]))
            .unwrap();
        (first, nearby, distant)
    };
    let filtered = request(
        address,
        "GET",
        &format!("/locations?mode=captures&area=capture-{first}"),
        &auth,
        "",
    )
    .await;
    assert!(
        filtered
            .body
            .contains(&format!("<span hx-disable>Capture #{first}</span>"))
    );
    assert!(
        filtered
            .body
            .contains(&format!("<span hx-disable>Capture #{nearby}</span>"))
    );
    assert!(
        !filtered
            .body
            .contains(&format!("<span hx-disable>Capture #{distant}</span>"))
    );
    assert!(filtered.body.contains("Area with 2 spots"));
    for path in [
        "/locations",
        "/locations?area=invalid",
        "/locations?mode=captures&point=capture-1",
    ] {
        let reply = request(address, "GET", path, &auth, "").await;
        assert_eq!(reply.status, 200);
        assert!(!reply.body.contains("class=\"spot"));
        assert!(!reply.body.contains("class=\"dot"));
    }
    server.abort();
    let _ = server.await;
    let (app, _) = App::new_preview(
        store.clone(),
        installation,
        Preview::new(token).unwrap().with_auth(),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, web::router(app)).into_future());
    assert_eq!(request(address, "GET", "/", &auth, "").await.status, 200);
    assert_eq!(store.lock().unwrap().list_captures().unwrap().len(), 4);
    server.abort();
    let _ = server.await;
    drop(engine);
    drop(lock);
}

#[cfg(feature = "dev")]
#[tokio::test(flavor = "current_thread")]
async fn preview_defaults_to_no_login_but_preserves_origin_and_host_checks() {
    use utilizer::dev::{self, Preview};
    let directory = tempdir().unwrap();
    let (_engine, store, _lock, installation) = dev::bootstrap(directory.path()).unwrap();
    let (app, _) = App::new_preview(
        Arc::new(Mutex::new(store)),
        installation,
        Preview::new("ab".repeat(32)).unwrap(),
    )
    .unwrap();
    assert!(!app.requires_auth());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, web::router(app)).into_future());
    let reply = request(address, "GET", "/", "", "").await;
    assert_eq!(reply.status, 200);
    assert!(!reply.body.contains("<summary>Test</summary>"));
    let reply = request(address, "GET", "/login", "", "").await;
    assert_eq!(reply.status, 303);
    assert!(reply.headers.contains("location: /"));
    assert_eq!(
        request(address, "GET", "/locations", "", "").await.status,
        200
    );
    assert_eq!(
        request(address, "POST", "/__dev/capture", "", "samples=yes")
            .await
            .status,
        403
    );
    assert_eq!(
        request(
            address,
            "POST",
            "/__dev/capture",
            "Origin: https://example.com\r\n",
            "samples=yes"
        )
        .await
        .status,
        403
    );
    let origin = format!("Origin: {}\r\n", web::URL);
    assert_eq!(
        request(
            address,
            "POST",
            "/manage",
            &origin,
            "kind=spot&map=de_mirage&name=Test&action=save&position=setpos%200%200%2064%3Bsetang%200%200%200"
        )
        .await
        .status,
        303
    );
    assert_eq!(
        request(address, "GET", "/", "Host: example.com\r\n", "")
            .await
            .status,
        403
    );
    server.abort();
    let _ = server.await;
}

#[cfg(feature = "dev")]
#[test]
fn preview_samples_survive_rebuilds_without_overwriting_edits_or_restoring_deletions() {
    let directory = tempdir().unwrap();
    let (engine, store, lock, installation) = utilizer::dev::bootstrap(directory.path()).unwrap();
    let samples = store.map_spots("de_mirage").unwrap();
    assert_eq!(samples.len(), 28);
    assert_eq!(
        samples.iter().map(|s| s.description.chars().count()).max(),
        Some(160)
    );
    assert!(store.list_playlists().unwrap().is_empty());
    let captures = store.list_captures().unwrap();
    assert_eq!(captures.len(), 6);
    assert!(captures.iter().all(|capture| {
        samples
            .iter()
            .all(|spot| spot.position.origin != capture.capture.position.origin)
    }));
    store.discard_capture(captures[0].id).unwrap();
    let pending = store.list_captures().unwrap();
    assert!(!installation.cfg.join("console_utilizer_rust").exists());
    store.delete_map_spot(samples[0].id, "de_mirage").unwrap();
    store
        .save_map_spot(
            Some(samples[1].id),
            "de_mirage",
            "My edited sample",
            None,
            samples[1].position,
        )
        .unwrap();
    let expected = store.map_spots("de_mirage").unwrap();
    drop(store);
    drop(engine);
    drop(lock);
    let (_, store, _, _) = utilizer::dev::bootstrap(directory.path()).unwrap();
    assert_eq!(store.map_spots("de_mirage").unwrap(), expected);
    assert_eq!(store.list_captures().unwrap(), pending);
}
