//! Local HTML adapter. Blocking persistence and cfg work stay off the runtime.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use askama::Template;
use axum::{
    Router,
    extract::{DefaultBodyLimit, Form, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;

use crate::{
    Result, cfg,
    discovery::Installation,
    invalid,
    storage::{PendingCapture, Playlist, Spot, Store},
};

pub const ADDRESS: &str = "127.0.0.1:47825";
pub const URL: &str = "http://127.0.0.1:47825";
pub type SharedStore = Arc<Mutex<Store>>;

pub struct App {
    pub store: SharedStore,
    pub installation: Installation,
    pub status: Mutex<String>,
    auth: Mutex<Auth>,
}

struct Auth {
    code: String,
    failures: u8,
    blocked_until: Option<Instant>,
    sessions: VecDeque<String>,
}

fn random_bytes() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| invalid(format!("Cannot generate login credentials: {error}")))?;
    Ok(bytes)
}

impl Auth {
    fn login(&mut self, code: &str, now: Instant) -> Result<String> {
        if self.blocked_until.is_some_and(|until| now < until) {
            return Err(invalid("Too many attempts. Wait 30 seconds, then try again.").into());
        }
        if self.blocked_until.take().is_some() {
            self.failures = 0;
        }
        if code != self.code {
            self.failures += 1;
            if self.failures == 5 {
                self.blocked_until = Some(now + Duration::from_secs(30));
                return Err(invalid("Too many attempts. Wait 30 seconds, then try again.").into());
            }
            return Err(invalid("That code does not match. Check the Utilizer console.").into());
        }
        let token: String = random_bytes()?.iter().map(|b| format!("{b:02x}")).collect();
        self.failures = 0;
        if self.sessions.len() == 32 {
            self.sessions.pop_front();
        }
        self.sessions.push_back(token.clone());
        Ok(token)
    }
}

impl App {
    pub fn new(store: SharedStore, installation: Installation) -> Result<(Arc<Self>, String)> {
        let bytes = random_bytes()?;
        let code = format!("{:04}", u32::from_ne_bytes(bytes[..4].try_into()?) % 10_000);
        let auth = Auth {
            code: code.clone(),
            failures: 0,
            blocked_until: None,
            sessions: VecDeque::new(),
        };
        Ok((
            Arc::new(Self {
                store,
                installation,
                status: Mutex::new("Waiting for console output".into()),
                auth: Mutex::new(auth),
            }),
            code,
        ))
    }

    async fn work<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut Store) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            let mut store = store.lock().map_err(|_| invalid("Storage lock failed"))?;
            action(&mut store)
        })
        .await?
    }
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/action", post(action))
        .route("/captures", get(captures))
        .route(
            "/assets/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../assets/style.css"),
                )
            }),
        )
        .route(
            "/assets/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../assets/app.js"),
                )
            }),
        )
        .route(
            "/assets/htmx.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../assets/htmx-2.0.8.min.js"),
                )
            }),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app)
}

fn session(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| part.trim().strip_prefix("utilizer_session="))
}

fn valid_origin(headers: &HeaderMap) -> bool {
    matches!(
        headers.get(header::HOST).and_then(|s| s.to_str().ok()),
        Some("127.0.0.1:47825" | "localhost:47825")
    ) && matches!(
        headers.get(header::ORIGIN).and_then(|s| s.to_str().ok()),
        Some("http://127.0.0.1:47825" | "http://localhost:47825")
    )
}

async fn guard(State(app): State<Arc<App>>, request: Request, next: Next) -> Response {
    if !matches!(
        request
            .headers()
            .get(header::HOST)
            .and_then(|s| s.to_str().ok()),
        Some("127.0.0.1:47825" | "localhost:47825")
    ) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if request.method() != axum::http::Method::GET && !valid_origin(request.headers()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let public = request.uri().path() == "/login" || request.uri().path().starts_with("/assets/");
    let authenticated = app.auth.lock().ok().is_some_and(|auth| {
        session(request.headers()).is_some_and(|s| auth.sessions.iter().any(|known| known == s))
    });
    let mut response = if !public && !authenticated {
        if request.headers().contains_key("hx-request") {
            (StatusCode::UNAUTHORIZED, [("hx-redirect", "/login")]).into_response()
        } else {
            Redirect::to("/login").into_response()
        }
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    headers.insert(header::VARY, "HX-Request".parse().expect("static header"));
    headers.insert(
        "x-content-type-options",
        "nosniff".parse().expect("static header"),
    );
    headers.insert(
        "referrer-policy",
        "no-referrer".parse().expect("static header"),
    );
    headers.insert("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'self'; frame-ancestors 'none'".parse().expect("static header"));
    response
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate<'a> {
    error: &'a str,
}

fn html(template: &impl Template) -> Response {
    match template.render() {
        Ok(body) => Html(body).into_response(),
        Err(error) => {
            eprintln!("Cannot render page: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn login_page() -> Response {
    html(&LoginTemplate { error: "" })
}

#[derive(Deserialize)]
struct LoginForm {
    code: String,
}

async fn login(State(app): State<Arc<App>>, Form(form): Form<LoginForm>) -> Response {
    let result = app
        .auth
        .lock()
        .map_err(|_| invalid("Login lock failed"))
        .and_then(|mut auth| {
            auth.login(&form.code, Instant::now())
                .map_err(|error| invalid(error.to_string()))
        });
    match result {
        Ok(token) => {
            let mut response = Redirect::to("/").into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                format!("utilizer_session={token}; HttpOnly; SameSite=Strict; Path=/")
                    .parse()
                    .expect("hex token"),
            );
            response
        }
        Err(error) => {
            let message = error.to_string();
            let mut response = html(&LoginTemplate { error: &message });
            *response.status_mut() = StatusCode::UNAUTHORIZED;
            response
        }
    }
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if let Ok(mut auth) = app.auth.lock() {
        auth.sessions
            .retain(|token| Some(token.as_str()) != session(&headers));
    }
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        "utilizer_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"
            .parse()
            .expect("static header"),
    );
    response
}

#[derive(Default, Deserialize)]
struct ViewQuery {
    playlist: Option<i64>,
    spot: Option<i64>,
    map: Option<String>,
    capture: Option<i64>,
}

struct View {
    playlists: Vec<Playlist>,
    maps: Vec<String>,
    spots: Vec<Spot>,
    playlist_id: i64,
    playlist_name: String,
    map_code: String,
    selected: i64,
    description: String,
    link: String,
    previous: i64,
    next: i64,
    capture_id: i64,
    captures_html: String,
    feedback: String,
    status: String,
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate<'a> {
    view: &'a View,
}

#[derive(Template)]
#[template(path = "workspace.html")]
struct WorkspaceTemplate<'a> {
    view: &'a View,
}

#[derive(Template)]
#[template(path = "captures.html")]
struct CapturesTemplate {
    captures: Vec<PendingCapture>,
    playlist_id: i64,
    status: String,
    polled: bool,
}

async fn view(app: &App, query: ViewQuery, feedback: String) -> Result<View> {
    let (playlists, spots, pending, saved_id, saved_spot) = app
        .work(move |store| {
            let playlists = store.list_playlists()?;
            let id = query.playlist.or(store
                .setting("selected_playlist")?
                .and_then(|s| s.parse().ok()));
            let selected = playlists
                .iter()
                .find(|p| Some(p.id) == id && query.map.as_ref().is_none_or(|m| *m == p.map_code))
                .or_else(|| {
                    playlists
                        .iter()
                        .find(|p| query.map.as_ref().is_none_or(|m| *m == p.map_code))
                });
            let id = selected.map_or(0, |p| p.id);
            let spot = query
                .spot
                .or(store.setting("selected_spot")?.and_then(|s| s.parse().ok()));
            Ok((
                playlists,
                store.load_spots(id)?,
                store.list_captures()?,
                id,
                spot,
            ))
        })
        .await?;
    let mut maps: Vec<_> = playlists.iter().map(|p| p.map_code.clone()).collect();
    maps.sort();
    maps.dedup();
    let playlist = playlists.iter().find(|p| p.id == saved_id);
    let map_code = playlist.map_or_else(|| "de_mirage".into(), |p| p.map_code.clone());
    let playlist_name = playlist.map_or_else(|| "Your first playlist".into(), |p| p.name.clone());
    let selected_index = spots
        .iter()
        .position(|s| Some(s.id) == saved_spot)
        .unwrap_or(0);
    let current = spots.get(selected_index);
    let selected = current.map_or(0, |s| s.id);
    let description = current.map_or_else(|| "No spot selected".into(), |s| s.description.clone());
    let link = current.and_then(|s| s.link.clone()).unwrap_or_default();
    let previous = if spots.is_empty() {
        0
    } else {
        spots[(selected_index + spots.len() - 1) % spots.len()].id
    };
    let next = if spots.is_empty() {
        0
    } else {
        spots[(selected_index + 1) % spots.len()].id
    };
    let capture_id = pending
        .iter()
        .find(|c| Some(c.id) == query.capture)
        .map_or(0, |c| c.id);
    let status = app
        .status
        .lock()
        .map_err(|_| invalid("Reader status unavailable"))?
        .clone();
    let captures_html = CapturesTemplate {
        captures: pending,
        playlist_id: saved_id,
        status: status.clone(),
        polled: false,
    }
    .render()?;
    Ok(View {
        playlists,
        maps,
        spots,
        playlist_id: saved_id,
        playlist_name,
        map_code,
        selected,
        description,
        link,
        previous,
        next,
        capture_id,
        captures_html,
        feedback,
        status,
    })
}

async fn page(app: &App, query: ViewQuery, headers: &HeaderMap, feedback: String) -> Response {
    match view(app, query, feedback).await {
        Ok(view) => {
            if headers.contains_key("hx-request") {
                html(&WorkspaceTemplate { view: &view })
            } else {
                html(&PageTemplate { view: &view })
            }
        }
        Err(error) => failure(error, headers),
    }
}

async fn home(
    State(app): State<Arc<App>>,
    Query(query): Query<ViewQuery>,
    headers: HeaderMap,
) -> Response {
    page(&app, query, &headers, String::new()).await
}

async fn captures(
    State(app): State<Arc<App>>,
    Query(query): Query<ViewQuery>,
    headers: HeaderMap,
) -> Response {
    match app.work(|store| store.list_captures()).await {
        Ok(captures) => html(&CapturesTemplate {
            captures,
            playlist_id: query.playlist.unwrap_or(0),
            status: app.status.lock().map_or_else(
                |_| "Reader status unavailable".into(),
                |status| status.clone(),
            ),
            polled: true,
        }),
        Err(error) => failure(error, &headers),
    }
}

#[derive(Template)]
#[template(
    source = "<div class=\"notice error\" role=\"alert\">{{ message }} <a href=\"/\">Back to player</a></div>",
    ext = "html"
)]
struct ErrorTemplate<'a> {
    message: &'a str,
}

fn failure(error: Box<dyn std::error::Error + Send + Sync>, headers: &HeaderMap) -> Response {
    eprintln!("Cannot complete browser action: {error}");
    let message = error.to_string();
    let mut response = html(&ErrorTemplate { message: &message });
    *response.status_mut() = StatusCode::BAD_REQUEST;
    if headers.contains_key("hx-request") {
        response
            .headers_mut()
            .insert("hx-retarget", "#feedback".parse().expect("static header"));
        response
            .headers_mut()
            .insert("hx-reswap", "innerHTML".parse().expect("static header"));
    }
    response
}

#[derive(Deserialize)]
struct Action {
    kind: String,
    playlist: Option<i64>,
    spot: Option<i64>,
    capture: Option<i64>,
    map_code: Option<String>,
    name: Option<String>,
    description: Option<String>,
    link: Option<String>,
}

async fn action(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Form(form): Form<Action>,
) -> Response {
    let cfg_root = app.installation.cfg.clone();
    let result = app
        .work(move |store| {
            if form.kind == "create" {
                let map = form.map_code.as_deref().unwrap_or("").trim();
                let name = form.name.as_deref().unwrap_or("").trim();
                if map.len() > 64 || name.len() > 120 {
                    return Err(invalid("Use a shorter map or playlist name").into());
                }
                let bytes = random_bytes()?;
                let suffix: String = bytes[..4].iter().map(|b| format!("{b:02x}")).collect();
                let playlist = store.create_playlist(map, &format!("{map}_{suffix}"), name)?;
                store.set_setting("selected_playlist", &playlist.id.to_string())?;
                return Ok((
                    playlist.id,
                    0,
                    "Playlist created. Sync it to prepare the console helpers.".into(),
                ));
            }
            let id = form.playlist.unwrap_or(0);
            let playlist = store
                .list_playlists()?
                .into_iter()
                .find(|p| p.id == id)
                .ok_or_else(|| invalid("Choose a playlist first"))?;
            let selected = match form.kind.as_str() {
                "sync" | "select" => form.spot.filter(|id| *id > 0),
                "accept" => {
                    let description = form.description.as_deref().unwrap_or("").trim();
                    let link = form.link.as_deref().filter(|s| !s.trim().is_empty());
                    if description.len() > 2000 || link.is_some_and(|s| s.len() > 2048) {
                        return Err(invalid("Use a shorter description or reference link").into());
                    }
                    let spot = store.accept_capture(
                        form.capture.ok_or_else(|| invalid("Choose a capture"))?,
                        id,
                        description,
                        link,
                    )?;
                    store.set_setting("selected_spot", &spot.to_string())?;
                    store.set_setting("selected_playlist", &id.to_string())?;
                    return Ok((
                        id,
                        spot,
                        "Spot saved. Sync the playlist to use it in CS2.".into(),
                    ));
                }
                "discard" => {
                    store.discard_capture(
                        form.capture.ok_or_else(|| invalid("Choose a capture"))?,
                    )?;
                    return Ok((id, 0, "Capture discarded.".into()));
                }
                _ => return Err(invalid("Unknown action").into()),
            };
            let spots = store.load_spots(id)?;
            let files = cfg::render_player(&playlist, &spots, selected)?;
            cfg::export(&cfg_root, &files)?;
            let spot = selected
                .or_else(|| spots.first().map(|s| s.id))
                .unwrap_or(0);
            store.set_setting("selected_playlist", &id.to_string())?;
            store.set_setting("selected_spot", &spot.to_string())?;
            Ok((
                id,
                spot,
                "Synced. Run the setup command once in CS2; press ↑ to load this selection.".into(),
            ))
        })
        .await;
    match result {
        Ok((playlist, spot, message)) => {
            if !headers.contains_key("hx-request") {
                return Redirect::to(&format!("/?playlist={playlist}&spot={spot}")).into_response();
            }
            page(
                &app,
                ViewQuery {
                    playlist: Some(playlist),
                    spot: Some(spot),
                    ..Default::default()
                },
                &headers,
                message,
            )
            .await
        }
        Err(error) => failure(error, &headers),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_code_enforces_cooldown_and_sessions_are_run_scoped() {
        let mut auth = Auth {
            code: "0123".into(),
            failures: 0,
            blocked_until: None,
            sessions: VecDeque::new(),
        };
        let now = Instant::now();
        for _ in 0..5 {
            assert!(auth.login("wrong", now).is_err());
        }
        assert!(auth.login("0123", now + Duration::from_secs(29)).is_err());
        let token = auth.login("0123", now + Duration::from_secs(30)).unwrap();
        assert_eq!(token.len(), 64);
        assert!(auth.sessions.contains(&token));
        assert_eq!(auth.failures, 0);
    }

    #[test]
    fn mutation_origin_rejects_cross_site_and_rebound_hosts() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, ADDRESS.parse().unwrap());
        assert!(!valid_origin(&headers));
        headers.insert(header::ORIGIN, URL.parse().unwrap());
        assert!(valid_origin(&headers));
        headers.insert(header::ORIGIN, "https://example.com".parse().unwrap());
        assert!(!valid_origin(&headers));
        headers.insert(header::ORIGIN, URL.parse().unwrap());
        headers.insert(header::HOST, "attacker.example:47825".parse().unwrap());
        assert!(!valid_origin(&headers));
    }
}
