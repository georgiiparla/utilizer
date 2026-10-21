use super::*;
use crate::{
    cfg,
    storage::{Store, validate_spot},
};
use std::hash::{Hash, Hasher};

#[derive(Default, Deserialize)]
pub(super) struct EditorQuery {
    #[serde(default)]
    kind: String,
    map: Option<String>,
    point: Option<String>,
}

#[derive(Template)]
#[template(path = "editor.html")]
struct EditorTemplate {
    mode: String,
    map: String,
    point: String,
    name: String,
    position: String,
    link: String,
    stamp: String,
    capture_key: String,
}

fn stamp(store: &Store, kind: &str, point: &str) -> Result<String> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    if kind == "binds" {
        store
            .setting("capture_key")?
            .unwrap_or_else(|| "F6".into())
            .hash(&mut hash);
    } else if kind == "spot" && point.starts_with("spot-") {
        let item = store.map_spot(point_id(point, "spot-")?)?;
        (
            item.id,
            item.map_code,
            item.ordinal,
            item.description,
            item.link,
            item.position.command(),
        )
            .hash(&mut hash);
    } else if kind == "spot" && point.starts_with("capture-") {
        let id = point_id(point, "capture-")?;
        let item = store
            .list_captures()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| invalid("Capture no longer pending"))?;
        (
            id,
            item.capture.playlist_slug,
            item.capture.position.command(),
            capture_map(store, id)?,
        )
            .hash(&mut hash);
    } else {
        return Ok(String::new());
    }
    Ok(format!("{:x}", hash.finish()))
}

fn point_id(key: &str, prefix: &str) -> Result<i64> {
    key.strip_prefix(prefix)
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("Invalid spot or capture").into())
}

fn capture_map(store: &Store, id: i64) -> Result<Option<String>> {
    store.capture_map(id)
}

pub(super) async fn show(
    State(app): State<Arc<App>>,
    Query(query): Query<EditorQuery>,
    headers: HeaderMap,
) -> Response {
    let result = app
        .work(move |store| {
            let map = maps::find(query.map.as_deref().unwrap_or("de_mirage"))
                .ok_or_else(|| invalid("Unsupported map"))?;
            let mut view = EditorTemplate {
                mode: query.kind,
                map: map.code.into(),
                point: query.point.unwrap_or_default(),
                name: String::new(),
                position: String::new(),
                link: String::new(),
                stamp: String::new(),
                capture_key: store.setting("capture_key")?.unwrap_or_else(|| "F6".into()),
            };
            match view.mode.as_str() {
                "binds" => {}
                "spot" => {
                    if view.point.starts_with("spot-") {
                        let spot = store.map_spot(point_id(&view.point, "spot-")?)?;
                        if spot.map_code != map.code {
                            return Err(invalid("Spot belongs to another map").into());
                        }
                        view.name = spot.description;
                        view.position = spot.position.command();
                        view.link = spot.link.unwrap_or_default();
                    } else if !view.point.is_empty() {
                        let id = point_id(&view.point, "capture-")?;
                        if capture_map(store, id)?
                            .as_deref()
                            .is_some_and(|m| m != map.code)
                        {
                            return Err(invalid("Capture belongs to another map").into());
                        }
                        let capture = store
                            .list_captures()?
                            .into_iter()
                            .find(|c| c.id == id)
                            .ok_or_else(|| invalid("Capture no longer pending"))?;
                        view.name = format!("Capture #{id}");
                        view.position = capture.capture.position.command();
                    }
                }
                _ => return Err(invalid("Unknown editor").into()),
            }
            view.stamp = stamp(store, &view.mode, &view.point)?;
            Ok(view)
        })
        .await;
    match result {
        Ok(view) => html(&view),
        Err(error) => failure(error, &headers),
    }
}

#[derive(Deserialize)]
pub(super) struct Action {
    kind: String,
    map: String,
    #[serde(default)]
    point: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    stamp: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    capture_key: String,
}

#[derive(Template)]
#[template(
    source = "<p data-saved data-map=\"{{ map }}\" data-changed=\"{{ changed }}\" role=\"status\">{{ message }}</p>",
    ext = "html"
)]
struct Saved {
    changed: bool,
    map: String,
    message: String,
}

fn sync(store: &Store, root: &std::path::Path, map: &str, selected: Option<i64>) -> Result<()> {
    let spots = store.map_spots(map)?;
    let key = store.setting("capture_key")?.unwrap_or_else(|| "F6".into());
    cfg::export_map_player_with_capture_key(root, map, &spots, selected, &key)?;
    Ok(())
}

fn perform(store: &mut Store, form: Action, root: &std::path::Path) -> Result<Saved> {
    if maps::find(&form.map).is_none() {
        return Err(invalid("Unsupported map").into());
    }
    if form.kind != "sync" && form.stamp != stamp(store, &form.kind, &form.point)? {
        return Err(invalid(
            "This item changed in another window. Reopen its details before saving.",
        )
        .into());
    }
    let mut selected = None;
    let message;
    match form.kind.as_str() {
        "binds" => {
            if form.action != "save" {
                return Err(invalid("Unknown action").into());
            }
            cfg::validate_capture_key(&form.capture_key)?;
            store.set_setting("capture_key", &form.capture_key)?;
            return Ok(Saved {
                map: form.map,
                changed: true,
                message: "Binding saved. Sync in Saved mode, then press Up in CS2 to apply.".into(),
            });
        }
        "sync" => {
            if form.mode == "captures" || form.point.starts_with("capture-") {
                return Err(invalid("Save captures before syncing").into());
            }
            if !matches!(form.mode.as_str(), "" | "spots") {
                return Err(invalid("Unknown mode").into());
            }
            if !form.point.is_empty() && !form.point.starts_with("spot-") {
                return Err(invalid("Invalid spot").into());
            }
            if form.point.starts_with("spot-") {
                let id = point_id(&form.point, "spot-")?;
                if store.map_spot(id)?.map_code != form.map {
                    return Err(invalid("Spot belongs to another map").into());
                }
                selected = Some(id);
            }
            sync(store, root, &form.map, selected)?;
            return Ok(Saved {
                map: form.map,
                changed: false,
                message: "Synced".into(),
            });
        }
        "spot" => {
            let source = if form.point.starts_with("spot-") {
                let spot = store.map_spot(point_id(&form.point, "spot-")?)?;
                if spot.map_code != form.map {
                    return Err(invalid("Spot belongs to another map").into());
                }
                Some(spot)
            } else {
                None
            };
            let capture = if form.point.starts_with("capture-") {
                let id = point_id(&form.point, "capture-")?;
                if capture_map(store, id)?
                    .as_deref()
                    .is_some_and(|m| m != form.map)
                {
                    return Err(invalid("Capture belongs to another map").into());
                }
                if !store.list_captures()?.iter().any(|c| c.id == id) {
                    return Err(invalid("Capture no longer pending").into());
                }
                Some(id)
            } else {
                None
            };
            if !form.point.is_empty() && source.is_none() && capture.is_none() {
                return Err(invalid("Invalid spot").into());
            }
            match form.action.as_str() {
                "delete" => {
                    if let Some(spot) = source {
                        store.delete_map_spot(spot.id, &form.map)?;
                    } else if let Some(id) = capture {
                        store.discard_capture(id)?;
                    } else {
                        return Err(invalid("Save a spot first").into());
                    }
                    message = "Deleted";
                }
                "save" => {
                    if form.position.len() > 256 {
                        return Err(invalid("Use a shorter position command").into());
                    }
                    let position = Position::parse(form.position.trim())
                        .ok_or_else(|| invalid("Use setpos X Y Z;setang PITCH YAW ROLL"))?;
                    if maps::find(&form.map)
                        .is_none_or(|map| map.calibration.project(position.origin).is_none())
                    {
                        return Err(invalid("Position is outside this map").into());
                    }
                    let link = (!form.link.trim().is_empty()).then_some(form.link.trim());
                    validate_spot(&form.name, link, position)?;
                    if let Some(spot) = source {
                        store.save_map_spot(
                            Some(spot.id),
                            &form.map,
                            &form.name,
                            link,
                            position,
                        )?;
                    } else if let Some(id) = capture {
                        store.accept_map_capture(id, &form.map, &form.name, link, position)?;
                    } else {
                        store.save_map_spot(None, &form.map, &form.name, link, position)?;
                    }
                    message = "Saved";
                }
                _ => return Err(invalid("Unknown spot action").into()),
            }
        }
        _ => return Err(invalid("Unknown action").into()),
    }
    Ok(Saved {
        changed: true,
        map: form.map,
        message: message.into(),
    })
}

pub(super) async fn manage(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Form(form): Form<Action>,
) -> Response {
    let root = app.installation.cfg.clone();
    match app.work(move |store| perform(store, form, &root)).await {
        Ok(saved) if !headers.contains_key("hx-request") => {
            Redirect::to(&format!("/?map={}", saved.map)).into_response()
        }
        Ok(saved) => html(&saved),
        Err(error) => failure(error, &headers),
    }
}
