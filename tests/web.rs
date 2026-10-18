//! Exercise the actual HTTP adapter without a browser or a real CS2 installation.
use std::{
    fs,
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
    let wire = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n{extra}\r\n{body}",
        web::ADDRESS,
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

#[tokio::test(flavor = "current_thread")]
async fn local_player_login_capture_sync_errors_and_logout() {
    let directory = tempdir().unwrap();
    let root = directory.path().join("cs2");
    fs::create_dir_all(root.join("game/bin/win64")).unwrap();
    fs::create_dir_all(root.join("game/csgo/cfg")).unwrap();
    fs::write(root.join("game/bin/win64/cs2.exe"), []).unwrap();
    let installation = Installation::validate(&root).unwrap();
    let output = installation.cfg.join("console_utilizer_rust");
    let database = directory.path().join("test.sqlite3");
    let store = Arc::new(Mutex::new(Store::open(&database).unwrap()));
    let (app, code) = App::new(store.clone(), installation).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = web::router(app.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let reply = request(address, "GET", "/", "HX-Request: true\r\n", "").await;
    assert_eq!(reply.status, 401);
    assert!(reply.headers.contains("hx-redirect: /login"));
    let reply = request(address, "POST", "/login", "", &format!("code={code}")).await;
    assert_eq!(reply.status, 403, "A mutation without Origin must fail");
    let origin = format!("Origin: {}\r\n", web::URL);
    let reply = request(address, "POST", "/login", &origin, &format!("code={code}")).await;
    assert_eq!(reply.status, 303);
    let cookie = reply
        .headers
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert!(reply.headers.contains("httponly; samesite=strict"));
    let authenticated = format!("{origin}Cookie: {cookie}\r\nHX-Request: true\r\n");

    let reply = request(
        address,
        "POST",
        "/action",
        &authenticated,
        "kind=create&map_code=de_mirage&name=%3Cscript%3Ealert%281%29%3C%2Fscript%3E",
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(!reply.body.contains("<script>alert(1)</script>"));
    assert!(reply.body.contains("&#60;script&#62;"));
    assert_eq!(reply.body.matches("id=\"reader-status\"").count(), 1);
    assert!(!output.exists(), "Creating a playlist must not export cfgs");
    let playlist = store.lock().unwrap().list_playlists().unwrap().remove(0);
    let reply = request(
        address,
        "POST",
        "/action",
        &authenticated,
        &format!("kind=sync&playlist={}", playlist.id),
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(output.join("start.cfg").is_file());

    let capture = store
        .lock()
        .unwrap()
        .record_capture(&Capture {
            playlist_slug: Some(playlist.slug),
            position: Position {
                origin: [1., 2., 3.],
                angles: [4., 5., 6.],
            },
        })
        .unwrap();
    *app.status.lock().unwrap() = "Watching console.log".into();
    let reply = request(
        address,
        "GET",
        &format!("/captures?playlist={}", playlist.id),
        &authenticated,
        "",
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(reply.body.contains(&format!("Capture #{capture}")));
    assert!(reply.body.contains("hx-swap-oob=\"outerHTML\""));
    assert!(reply.body.contains("Watching console.log"));
    assert!(!reply.body.contains("<html"));
    let accept = format!(
        "kind=accept&playlist={}&capture={capture}&description=Window+smoke",
        playlist.id
    );
    assert_eq!(
        request(address, "POST", "/action", &authenticated, &accept)
            .await
            .status,
        200
    );
    assert_eq!(
        request(address, "POST", "/action", &authenticated, &accept)
            .await
            .status,
        200
    );
    assert_eq!(
        store.lock().unwrap().load_spots(playlist.id).unwrap().len(),
        1
    );
    let current = fs::read(output.join("current.cfg")).unwrap();
    let reply = request(
        address,
        "POST",
        "/action",
        &authenticated,
        &format!("kind=select&playlist={}&spot=99999", playlist.id),
    )
    .await;
    assert_eq!(reply.status, 400);
    assert!(reply.headers.contains("hx-retarget: #feedback"));
    assert_eq!(fs::read(output.join("current.cfg")).unwrap(), current);

    // Force a real storage error to check that polling targets feedback, keeping
    // the inbox and any unsaved capture form intact.
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute_batch("PRAGMA foreign_keys=OFF; DROP TABLE captures;")
        .unwrap();
    let reply = request(address, "GET", "/captures", &authenticated, "").await;
    assert_eq!(reply.status, 400);
    assert!(reply.headers.contains("hx-retarget: #feedback"));
    let reply = request(address, "POST", "/logout", &authenticated, "").await;
    assert_eq!(reply.status, 303);
    assert_eq!(
        request(address, "GET", "/", &authenticated, "")
            .await
            .status,
        401
    );
    server.abort();
    let _ = server.await;
}
