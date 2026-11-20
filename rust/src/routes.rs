use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::deezer::{parse_id, SearchItem};
use crate::logging;
use crate::queue::{AppState, Job, TaskInfo};

#[derive(RustEmbed)]
#[folder = "web/assets/"]
struct Assets;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/find", get(find))
        .route("/debug", get(debug))
        .route("/queue", get(queue))
        .route("/search", post(search))
        .route("/download", post(download))
        .route("/playlist/deezer", post(playlist))
        .route("/favorites/deezer", post(favorites))
        .route("/static/{*path}", get(static_file))
        .with_state(state)
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": self.message })),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        let message = format!("{:#}", error.into());
        logging::error(&message);
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message,
        }
    }
}

// The jQuery frontend sends a JSON string but historically used the default
// form content type, so deserialize the raw body instead of requiring
// `Content-Type: application/json`.
fn parse_body<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, ApiError> {
    serde_json::from_str(body).map_err(|err| ApiError::bad(format!("invalid JSON body: {err}")))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_index(query: &str) -> String {
    include_str!("../web/index.html")
        .replace("{{ static_root }}", "/static")
        .replace("{{ api_root }}", "")
        .replace("{{ use_mpd }}", "false")
        .replace("{{ query }}", &escape_html(query))
}

async fn index(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    Html(render_index(
        params.get("query").map(String::as_str).unwrap_or(""),
    ))
}

async fn find(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    Html(render_index(
        params.get("query").map(String::as_str).unwrap_or(""),
    ))
}

async fn debug() -> Json<Value> {
    Json(json!({ "debug_msg": logging::tail() }))
}

async fn queue(State(state): State<AppState>) -> Json<Vec<TaskInfo>> {
    Json(state.tasks.lock().unwrap().clone())
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    #[serde(rename = "type")]
    kind: String,
    query: String,
}

async fn search(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<Vec<SearchItem>>, ApiError> {
    let request: SearchRequest = parse_body(&body)?;
    if request.query.trim().is_empty() {
        return Err(ApiError::bad("query is empty"));
    }
    let allowed = [
        "all",
        "track",
        "album",
        "album_track",
        "playlist",
        "artist",
        "artist_track",
    ];
    if !allowed.contains(&request.kind.as_str()) {
        return Err(ApiError::bad(format!(
            "unsupported search type '{}'",
            request.kind
        )));
    }

    let items = state.deezer.search(&request.kind, &request.query).await?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct DownloadRequest {
    #[serde(rename = "type")]
    kind: String,
    music_id: i64,
    #[serde(default)]
    add_to_playlist: bool,
    #[serde(default)]
    create_zip: bool,
}

async fn download(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<Value>, ApiError> {
    let request: DownloadRequest = parse_body(&body)?;
    let _ = request.add_to_playlist; // MPD integration is not part of the Rust app.

    let (description, job) = match request.kind.as_str() {
        "track" => (
            "Downloading song track",
            Job::Track {
                track_id: request.music_id,
            },
        ),
        "album" => (
            "Downloading song album",
            Job::Album {
                album_id: request.music_id,
                create_zip: request.create_zip,
            },
        ),
        other => {
            return Err(ApiError::bad(format!(
                "unsupported download type '{other}'"
            )))
        }
    };

    let task_id = state.enqueue(description, job);
    Ok(Json(json!({ "task_id": task_id })))
}

#[derive(Debug, Deserialize)]
struct PlaylistRequest {
    playlist_url: String,
    #[serde(default)]
    add_to_playlist: bool,
    #[serde(default)]
    create_zip: bool,
}

async fn playlist(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<Value>, ApiError> {
    let request: PlaylistRequest = parse_body(&body)?;
    let _ = request.add_to_playlist;
    if request.playlist_url.trim().is_empty() {
        return Err(ApiError::bad("playlist_url is empty"));
    }
    let playlist_id = parse_id(&request.playlist_url)
        .ok_or_else(|| ApiError::bad("playlist_url must contain a numeric id"))?;

    let task_id = state.enqueue(
        "Downloading Deezer playlist",
        Job::Playlist {
            playlist_id,
            create_zip: request.create_zip,
        },
    );
    Ok(Json(json!({ "task_id": task_id })))
}

#[derive(Debug, Deserialize)]
struct FavoritesRequest {
    user_id: String,
    #[serde(default)]
    add_to_playlist: bool,
    #[serde(default)]
    create_zip: bool,
}

async fn favorites(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<Value>, ApiError> {
    let request: FavoritesRequest = parse_body(&body)?;
    let _ = request.add_to_playlist;
    let user_id = request.user_id.trim();
    if user_id.is_empty() || !user_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::bad("user_id must be a numeric string"));
    }

    let task_id = state.enqueue(
        "Downloading Deezer favorites",
        Job::Favorites {
            user_id: user_id.to_string(),
            create_zip: request.create_zip,
        },
    );
    Ok(Json(json!({ "task_id": task_id })))
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "svg" => "image/svg+xml",
        "html" => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn static_file(Path(path): Path<String>) -> Response {
    match Assets::get(&path) {
        Some(file) => {
            let mut response = Response::new(Body::from(file.data.into_owned()));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type(&path)),
            );
            response
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
