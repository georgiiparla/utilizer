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

use crate::{Result, discovery::Installation, invalid, maps, protocol::Position, storage::Store};

mod editor;

pub const ADDRESS: &str = "127.0.0.1:47825";
pub const URL: &str = "http://127.0.0.1:47825";
pub type SharedStore = Arc<Mutex<Store>>;

pub struct App {
    pub store: SharedStore,
    pub installation: Installation,
    pub status: Mutex<String>,
    auth: Mutex<Auth>,
    #[cfg(feature = "dev")]
    pub(crate) preview: Option<crate::dev::Preview>,
}

struct Auth {
    code: String,
    failures: u8,
    blocked_until: Option<Instant>,
    sessions: VecDeque<String>,
    #[cfg(feature = "dev")]
    preview_session: Option<String>,
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
        #[cfg(feature = "dev")]
        let token = self.preview_session.clone().unwrap_or(token);
        self.failures = 0;
        if self.sessions.len() == 32 {
            self.sessions.pop_front();
        }
        self.sessions.push_back(token.clone());
        Ok(token)
    }
}

impl App {
    pub fn requires_auth(&self) -> bool {
        #[cfg(feature = "dev")]
        if let Some(preview) = &self.preview {
            return preview.require_auth;
        }
        true
    }

    pub fn new(store: SharedStore, installation: Installation) -> Result<(Arc<Self>, String)> {
        let bytes = random_bytes()?;
        let code = format!("{:04}", u32::from_ne_bytes(bytes[..4].try_into()?) % 10_000);
        let auth = Auth {
            code: code.clone(),
            failures: 0,
            blocked_until: None,
            sessions: VecDeque::new(),
            #[cfg(feature = "dev")]
            preview_session: None,
        };
        Ok((
            Arc::new(Self {
                store,
                installation,
                status: Mutex::new("Waiting for console output".into()),
                auth: Mutex::new(auth),
                #[cfg(feature = "dev")]
                preview: None,
            }),
            code,
        ))
    }

    #[cfg(feature = "dev")]
    pub fn new_preview(
        store: SharedStore,
        installation: Installation,
        preview: crate::dev::Preview,
    ) -> Result<(Arc<Self>, String)> {
        let (mut app, code) = Self::new(store, installation)?;
        let inner = Arc::get_mut(&mut app).expect("new app has one owner");
        let auth = inner
            .auth
            .get_mut()
            .map_err(|_| invalid("Login lock failed"))?;
        auth.sessions.push_back(preview.session().to_owned());
        auth.preview_session = Some(preview.session().to_owned());
        inner.preview = Some(preview);
        Ok((app, code))
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
    let router = Router::new()
        .route("/editor", get(editor::show))
        .route("/manage", post(editor::manage))
        .route("/", get(home))
        .route(
            "/manifest.webmanifest",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/manifest+json")],
                    include_str!("../assets/manifest.webmanifest"),
                )
            }),
        )
        .route(
            "/assets/icon-192.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../assets/icon-192.png").as_slice(),
                )
            }),
        )
        .route(
            "/assets/icon-512.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../assets/icon-512.png").as_slice(),
                )
            }),
        )
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/locations", get(locations))
        .route("/assets/maps/{file}", get(map_asset))
        .route(
            "/assets/fonts/plex.ttf",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "font/ttf")],
                    include_bytes!("../assets/fonts/IBMPlexSans.ttf").as_slice(),
                )
            }),
        )
        .route(
            "/assets/logo.svg",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!("../assets/logo.svg"),
                )
            }),
        )
        .route(
            "/assets/logo.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../assets/logo.png").as_slice(),
                )
            }),
        )
        .route(
            "/assets/logo.ico",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/x-icon")],
                    include_bytes!("../assets/logo.ico").as_slice(),
                )
            }),
        )
        .route(
            "/assets/style.css",
            get(|State(_app): State<Arc<App>>| async move {
                #[cfg(feature = "dev")]
                if _app.preview.is_some() {
                    return crate::dev::asset("style.css", "text/css; charset=utf-8").await;
                }
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../assets/style.css"),
                )
                    .into_response()
            }),
        )
        .route(
            "/assets/theme.js",
            get(|State(_app): State<Arc<App>>| async move {
                #[cfg(feature = "dev")]
                if _app.preview.is_some() {
                    return crate::dev::asset("theme.js", "text/javascript; charset=utf-8").await;
                }
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../assets/theme.js"),
                )
                    .into_response()
            }),
        )
        .route(
            "/assets/app.js",
            get(|State(_app): State<Arc<App>>| async move {
                #[cfg(feature = "dev")]
                if _app.preview.is_some() {
                    return crate::dev::asset("app.js", "text/javascript; charset=utf-8").await;
                }
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../assets/app.js"),
                )
                    .into_response()
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
        );
    #[cfg(feature = "dev")]
    let router = crate::dev::routes(router, &app);
    router
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
        return security_headers(
            (
                StatusCode::FORBIDDEN,
                "Open Utilizer at http://127.0.0.1:47825/login.",
            )
                .into_response(),
        );
    }
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && !valid_origin(request.headers())
    {
        let mut response = if request.uri().path() == "/login" {
            html(&LoginTemplate {
                error: "The browser could not verify this local request. Reload the login page and enter the console code again.",
            })
        } else {
            failure(invalid("The browser could not verify this local request. Reload the player and try again.").into(), request.headers())
        };
        *response.status_mut() = StatusCode::FORBIDDEN;
        return security_headers(response);
    }
    let public = matches!(request.uri().path(), "/login" | "/manifest.webmanifest")
        || request.uri().path().starts_with("/assets/");
    #[cfg(feature = "dev")]
    let public = public || (app.preview.is_some() && request.uri().path() == "/__dev/revision");
    let authenticated = !app.requires_auth()
        || app.auth.lock().ok().is_some_and(|auth| {
            session(request.headers()).is_some_and(|s| auth.sessions.iter().any(|known| known == s))
        });
    let response = if !public && !authenticated {
        if request.headers().contains_key("hx-request") {
            (StatusCode::UNAUTHORIZED, [("hx-redirect", "/login")]).into_response()
        } else {
            Redirect::to("/login").into_response()
        }
    } else {
        next.run(request).await
    };
    #[cfg(feature = "dev")]
    let response = if app.preview.is_some() {
        crate::dev::enhance(response).await
    } else {
        response
    };
    security_headers(response)
}

fn security_headers(mut response: Response) -> Response {
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
        // Native POST forms need their local Origin for the request guard.
        // no-referrer makes browsers send Origin: null for these submissions.
        "same-origin".parse().expect("static header"),
    );
    headers.insert("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self' data:; manifest-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'".parse().expect("static header"));
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

async fn login_page(State(app): State<Arc<App>>) -> Response {
    if !app.requires_auth() {
        return Redirect::to("/").into_response();
    }
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
    point: Option<String>,
    map: Option<String>,
    revision: Option<String>,
    mode: Option<String>,
}

struct MapPoint {
    key: String,
    name: String,
    command: String,
    x: f64,
    y: f64,
    placed: bool,
    reason: &'static str,
    selected: bool,
}

impl MapPoint {
    fn new(
        key: String,
        name: String,
        position: Position,
        map: Option<&str>,
        selected_map: &maps::Map,
    ) -> Self {
        let projected = (map == Some(selected_map.code) && selected_map.artwork.is_some())
            .then(|| selected_map.calibration.project(position.origin))
            .flatten();
        let reason = if map.is_none() {
            "Map unknown"
        } else if map != Some(selected_map.code) {
            "Another map"
        } else if selected_map.artwork.is_none() {
            "Map artwork unavailable"
        } else if projected.is_none() {
            "Outside map bounds"
        } else {
            ""
        };
        let [x, y] = projected.unwrap_or([0.0, 0.0]);
        Self {
            key,
            name,
            command: position.command(),
            x,
            y,
            placed: projected.is_some(),
            reason,
            selected: false,
        }
    }
}

struct MapView {
    mode: &'static str,
    points: Vec<MapPoint>,
    selected: String,
    map: &'static maps::Map,
    maps: &'static [maps::Map],
    detected: String,
    revision: String,
}

async fn map_asset(axum::extract::Path(code): axum::extract::Path<String>) -> Response {
    let Some(name) = code.strip_suffix(".svg") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (name, dark) = name
        .strip_suffix("-dark")
        .map_or((name, false), |name| (name, true));
    match maps::find(&format!("de_{name}"))
        .and_then(|map| if dark { map.dark_artwork } else { map.artwork })
    {
        Some(artwork) => ([(header::CONTENT_TYPE, "image/svg+xml")], artwork).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate<'a> {
    view: &'a MapView,
}

#[derive(Template)]
#[template(path = "locations.html")]
struct LocationsTemplate<'a> {
    view: &'a MapView,
}

async fn map_view(app: &App, query: ViewQuery) -> Result<MapView> {
    let detected = app
        .work(|store| store.setting("active_map"))
        .await?
        .unwrap_or_default();
    let map = if !detected.is_empty() {
        maps::find(&detected)
    } else {
        query.map.as_deref().and_then(maps::find)
    }
    .unwrap_or(&maps::MAPS[1]);
    let mode = match query.mode.as_deref() {
        None | Some("spots") => "spots",
        Some("captures") => "captures",
        _ => return Err(invalid("Unknown mode").into()),
    };
    let mut points = app
        .work(move |store| {
            let mut points = Vec::new();
            if mode == "spots" {
                for spot in store.map_spots(map.code)? {
                    points.push(MapPoint::new(
                        format!("spot-{}", spot.id),
                        spot.description,
                        spot.position,
                        Some(map.code),
                        map,
                    ));
                }
            } else {
                for pending in store.list_captures()? {
                    let recorded = store.capture_map(pending.id)?;
                    let capture_map = recorded.as_deref();
                    if capture_map != Some(map.code) {
                        continue;
                    }
                    points.push(MapPoint::new(
                        format!("capture-{}", pending.id),
                        format!("Capture #{}", pending.id),
                        pending.capture.position,
                        capture_map,
                        map,
                    ));
                }
            }
            Ok(points)
        })
        .await?;
    points.retain(|point| point.placed);
    let selected = points
        .iter()
        .find(|p| Some(&p.key) == query.point.as_ref())
        .or_else(|| points.first())
        .map(|p| p.key.clone())
        .unwrap_or_default();
    for point in &mut points {
        point.selected = point.key == selected;
    }
    use std::hash::{Hash, Hasher};
    let mut revision = std::collections::hash_map::DefaultHasher::new();
    (map.code, mode, &detected, &selected).hash(&mut revision);
    for point in &points {
        (&point.key, &point.name, &point.command, point.reason).hash(&mut revision);
    }
    Ok(MapView {
        mode,
        points,
        selected,
        map,
        maps: &maps::MAPS,
        detected,
        revision: format!("{:x}", revision.finish()),
    })
}

async fn home(State(app): State<Arc<App>>, Query(query): Query<ViewQuery>) -> Response {
    match map_view(&app, query).await {
        Ok(view) => html(&PageTemplate { view: &view }),
        Err(error) => failure(error, &HeaderMap::new()),
    }
}

async fn locations(
    State(app): State<Arc<App>>,
    Query(query): Query<ViewQuery>,
    headers: HeaderMap,
) -> Response {
    let previous = query.revision.clone();
    match map_view(&app, query).await {
        Ok(view) if previous.as_deref() == Some(&view.revision) => {
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(view) => {
            let mut response = html(&LocationsTemplate { view: &view });
            if previous.is_none() {
                let url = format!(
                    "/?map={}&mode={}&point={}",
                    view.map.code, view.mode, view.selected
                );
                if let Ok(value) = url.parse() {
                    response.headers_mut().insert("hx-replace-url", value);
                }
            }
            response
        }
        Err(error) => failure(error, &headers),
    }
}

#[derive(Template)]
#[template(
    source = "<p data-important role=\"alert\">{{ message }}</p>",
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
            #[cfg(feature = "dev")]
            preview_session: None,
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
