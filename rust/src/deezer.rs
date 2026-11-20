use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const GW_LIGHT: &str = "https://www.deezer.com/ajax/gw-light.php";
const LICENSE_CACHE: Duration = Duration::from_secs(30 * 60);

struct License {
    token: String,
    csrf: String,
    fetched: Instant,
}

pub struct Deezer {
    client: reqwest::Client,
    license: Mutex<Option<License>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Song {
    #[serde(rename = "SNG_ID", default, deserialize_with = "de_string")]
    pub sng_id: String,
    #[serde(rename = "SNG_TITLE", default, deserialize_with = "de_string")]
    pub title: String,
    #[serde(rename = "VERSION", default, deserialize_with = "de_string")]
    pub version: String,
    #[serde(rename = "ART_NAME", default, deserialize_with = "de_string")]
    pub artist: String,
    #[serde(rename = "ALB_ID", default, deserialize_with = "de_string")]
    pub album_id: String,
    #[serde(rename = "ALB_TITLE", default, deserialize_with = "de_string")]
    pub album: String,
    #[serde(rename = "ALB_PICTURE", default, deserialize_with = "de_string")]
    pub album_picture: String,
    #[serde(rename = "TRACK_NUMBER", default, deserialize_with = "de_string")]
    pub track_number: String,
    #[serde(rename = "DISK_NUMBER", default, deserialize_with = "de_string")]
    pub disk_number: String,
    #[serde(rename = "DURATION", default, deserialize_with = "de_string")]
    pub duration: String,
    #[serde(rename = "ISRC", default, deserialize_with = "de_string")]
    pub isrc: String,
    #[serde(rename = "TRACK_TOKEN", default, deserialize_with = "de_string")]
    pub track_token: String,
    #[serde(rename = "FILESIZE_MP3_320", default, deserialize_with = "de_string")]
    pub filesize_mp3_320: String,
    #[serde(
        rename = "PHYSICAL_RELEASE_DATE",
        default,
        deserialize_with = "de_string"
    )]
    pub physical_release_date: String,
    #[serde(
        rename = "DIGITAL_RELEASE_DATE",
        default,
        deserialize_with = "de_string"
    )]
    pub digital_release_date: String,
    #[serde(rename = "MEDIA", default)]
    pub media: Vec<MediaEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MediaEntry {
    #[serde(rename = "TYPE", default, deserialize_with = "de_string")]
    pub kind: String,
    #[serde(rename = "HREF", default, deserialize_with = "de_string")]
    pub href: String,
}

#[derive(Debug, Clone, Default)]
pub struct AlbumMeta {
    pub title: String,
    pub artist: String,
    pub label: String,
    pub physical_date: String,
    pub digital_date: String,
    pub total_tracks: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchItem {
    pub id: String,
    pub id_type: String,
    pub title: String,
    pub album: String,
    pub album_id: String,
    pub artist: String,
    pub img_url: String,
    pub big_img_url: String,
    pub duration: String,
    pub link: String,
    pub preview_url: String,
}

fn de_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Null => String::new(),
        Value::String(text) => text,
        other => other.to_string(),
    })
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

fn format_duration(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn cover_url(picture: &str, size: &str) -> String {
    if picture.is_empty() {
        String::new()
    } else {
        format!("https://cdn-images.dzcdn.net/images/cover/{picture}/{size}")
    }
}

impl Song {
    pub fn duration_secs(&self) -> u64 {
        self.duration.parse().unwrap_or(0)
    }

    pub fn preview_url(&self) -> String {
        self.media
            .iter()
            .find(|entry| entry.kind == "preview")
            .map(|entry| entry.href.clone())
            .unwrap_or_default()
    }

    pub fn search_item(&self) -> SearchItem {
        SearchItem {
            id: self.sng_id.clone(),
            id_type: "track".to_string(),
            title: self.title.clone(),
            album: self.album.clone(),
            album_id: self.album_id.clone(),
            artist: self.artist.clone(),
            img_url: cover_url(&self.album_picture, "120x120-000000-80-0-0.jpg"),
            big_img_url: cover_url(&self.album_picture, "500x500-000000-80-0-0.jpg"),
            duration: format_duration(self.duration_secs()),
            link: format!("https://www.deezer.com/track/{}", self.sng_id),
            preview_url: self.preview_url(),
        }
    }
}

impl Deezer {
    pub fn new(cookie_arl: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(reqwest::header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
        headers.insert(
            reqwest::header::REFERER,
            HeaderValue::from_static("https://www.deezer.com/"),
        );
        headers.insert(
            reqwest::header::ORIGIN,
            HeaderValue::from_static("https://www.deezer.com"),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            "X-Requested-With",
            HeaderValue::from_static("XMLHttpRequest"),
        );

        let jar = Arc::new(reqwest::cookie::Jar::default());
        let cookie = format!("arl={cookie_arl}; Domain=.deezer.com; Path=/");
        jar.add_cookie_str(&cookie, &reqwest::Url::parse("https://www.deezer.com")?);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_provider(jar)
            .build()
            .context("could not build the HTTP client")?;

        Ok(Self {
            client,
            license: Mutex::new(None),
        })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn cached_license(&self) -> Option<(String, String)> {
        let guard = self.license.lock().unwrap();
        guard.as_ref().and_then(|license| {
            if license.fetched.elapsed() < LICENSE_CACHE {
                Some((license.token.clone(), license.csrf.clone()))
            } else {
                None
            }
        })
    }

    async fn user_data(&self) -> Result<(String, String)> {
        if let Some(cached) = self.cached_license() {
            return Ok(cached);
        }

        // Send an explicit body: Deezer answers 411 to a POST without Content-Length.
        let value = self
            .gw_light("deezer.getUserData", "", Some(json!({})))
            .await?;
        let results = value
            .get("results")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));

        let token = results
            .pointer("/USER/OPTIONS/license_token")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let csrf = results
            .get("checkForm")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        if token.is_empty() || csrf.is_empty() {
            bail!("could not get a license token from Deezer — is the arl cookie still valid?");
        }

        *self.license.lock().unwrap() = Some(License {
            token: token.clone(),
            csrf: csrf.clone(),
            fetched: Instant::now(),
        });
        Ok((token, csrf))
    }

    async fn csrf(&self) -> Result<String> {
        Ok(self.user_data().await?.1)
    }

    async fn gw_light(&self, method: &str, token: &str, body: Option<Value>) -> Result<Value> {
        let url = format!(
            "{GW_LIGHT}?method={method}&input=3&api_version=1.0&api_token={token}"
        );
        let request = self.client.post(&url);
        let request = match body {
            Some(body) => request.json(&body),
            None => request,
        };
        let response = request
            .send()
            .await
            .with_context(|| format!("Deezer {method} request failed"))?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!(
                "Deezer {method} returned {status}: {}",
                truncate(&text, 300)
            );
        }

        let value: Value = serde_json::from_str(&text).with_context(|| {
            format!(
                "invalid JSON from Deezer {method}: {}",
                truncate(&text, 300)
            )
        })?;

        if let Some(error) = value.get("error") {
            let empty = error.is_null()
                || error
                    .as_array()
                    .map(|items| items.is_empty())
                    .unwrap_or(false);
            if !empty {
                bail!("Deezer {method} error: {error}");
            }
        }

        Ok(value)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url} failed"))?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            bail!("GET {url} returned {status}: {}", truncate(&text, 300));
        }

        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("invalid JSON from {url}: {}", truncate(&text, 300)))?;

        if let Some(error) = value.get("error") {
            if !error.is_null() {
                bail!("Deezer API error for {url}: {error}");
            }
        }

        serde_json::from_value(value)
            .with_context(|| format!("unexpected JSON from {url}"))
    }

    pub async fn get_song(&self, song_id: i64) -> Result<Song> {
        let csrf = self.csrf().await?;
        let value = self
            .gw_light("song.getData", &csrf, Some(json!({ "sng_id": song_id })))
            .await?;
        let song: Song = serde_json::from_value(
            value
                .get("results")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default())),
        )
        .context("song.getData returned an unexpected payload")?;
        if song.sng_id.is_empty() {
            bail!("song.getData returned no SNG_ID for track {song_id}");
        }
        Ok(song)
    }

    pub async fn page_album(&self, album_id: i64) -> Result<(AlbumMeta, Vec<Song>)> {
        let csrf = self.csrf().await?;
        let value = self
            .gw_light(
                "deezer.pageAlbum",
                &csrf,
                Some(json!({ "alb_id": album_id, "lang": "en", "tab": 0, "nb": 200 })),
            )
            .await?;
        let page: PageAlbumResponse =
            serde_json::from_value(value).context("deezer.pageAlbum returned an unexpected payload")?;
        if page.results.songs.data.is_empty() {
            bail!("album {album_id} has no tracks (private or unavailable?)");
        }
        Ok((page.results.data.into_meta(), page.results.songs.data))
    }

    pub async fn page_playlist(&self, playlist_id: i64) -> Result<(String, Vec<Song>)> {
        let csrf = self.csrf().await?;
        let value = self
            .gw_light(
                "deezer.pagePlaylist",
                &csrf,
                Some(json!({
                    "playlist_id": playlist_id,
                    "start": 0,
                    "tab": 0,
                    "header": true,
                    "lang": "en",
                    "nb": 1000
                })),
            )
            .await?;
        let page: PagePlaylistResponse = serde_json::from_value(value)
            .context("deezer.pagePlaylist returned an unexpected payload")?;
        if page.results.songs.data.is_empty() {
            bail!("playlist {playlist_id} has no tracks (private or unavailable?)");
        }
        let title = page.results.data.title;
        let title = if title.trim().is_empty() {
            format!("playlist_{playlist_id}")
        } else {
            title
        };
        Ok((title, page.results.songs.data))
    }

    pub async fn album_meta(&self, album_id: i64) -> Result<AlbumMeta> {
        let detail: ApiAlbumDetail =
            self.get_json(&format!("https://api.deezer.com/album/{album_id}")).await?;
        let release = detail.release_date.unwrap_or_default();
        Ok(AlbumMeta {
            title: detail.title,
            artist: detail.artist.map(|a| a.name).unwrap_or_default(),
            label: detail.label.unwrap_or_default(),
            physical_date: release.clone(),
            digital_date: release,
            total_tracks: detail.nb_tracks.map(|n| n.to_string()).unwrap_or_default(),
        })
    }

    pub async fn stream_url(&self, track_token: &str) -> Result<(String, Option<u64>)> {
        let (license_token, _) = self.user_data().await?;
        let body = json!({
            "license_token": license_token,
            "media": [{
                "type": "FULL",
                "formats": [
                    { "cipher": "BF_CBC_STRIPE", "format": "MP3_320" },
                    { "cipher": "BF_CBC_STRIPE", "format": "MP3_128" }
                ]
            }],
            "track_tokens": [track_token]
        });

        let response = self
            .client
            .post("https://media.deezer.com/v1/get_url")
            .json(&body)
            .send()
            .await
            .context("media.deezer.com get_url request failed")?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            bail!("get_url returned {status}: {}", truncate(&text, 300));
        }
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("invalid JSON from get_url: {}", truncate(&text, 300)))?;

        let media = value
            .pointer("/data/0/media/0")
            .ok_or_else(|| anyhow!("no media in get_url response: {}", truncate(&text, 300)))?;
        let url = media
            .pointer("/sources/0/url")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("no stream url in get_url response: {}", truncate(&text, 300)))?;

        Ok((
            url.to_string(),
            media.get("filesize").and_then(Value::as_u64),
        ))
    }

    pub async fn cover(&self, picture: &str) -> Option<Vec<u8>> {
        if picture.is_empty() {
            return None;
        }
        let url =
            format!("https://e-cdns-images.dzcdn.net/images/cover/{picture}/1200x1200.jpg");
        let response = self.client.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.bytes().await.ok().map(|bytes| bytes.to_vec())
    }

    pub async fn favorites(&self, user_id: &str) -> Result<Vec<i64>> {
        if !user_id.chars().all(|c| c.is_ascii_digit()) {
            bail!("user_id must be numeric");
        }
        let page: FavoritePage = self
            .get_json(&format!(
                "https://api.deezer.com/user/{user_id}/tracks?limit=100000"
            ))
            .await?;
        Ok(page.data.into_iter().map(|item| item.id).collect())
    }

    pub async fn search(&self, kind: &str, query: &str) -> Result<Vec<SearchItem>> {
        let encoded = urlencoding::encode(query);

        match kind {
            "track" => {
                let page: ApiPage<ApiTrack> = self
                    .get_json(&format!("https://api.deezer.com/search/track?q={encoded}"))
                    .await?;
                Ok(page.data.iter().map(track_item).collect())
            }
            "album" => {
                let page: ApiPage<ApiAlbumItem> = self
                    .get_json(&format!("https://api.deezer.com/search/album?q={encoded}"))
                    .await?;
                Ok(page.data.iter().map(album_item).collect())
            }
            "artist" => {
                let page: ApiPage<ApiArtistItem> = self
                    .get_json(&format!("https://api.deezer.com/search/artist?q={encoded}"))
                    .await?;
                Ok(page.data.iter().map(artist_item).collect())
            }
            "album_track" => {
                let album_id =
                    parse_id(query).ok_or_else(|| anyhow!("album_track needs an album id"))?;
                let (_, songs) = self.page_album(album_id).await?;
                Ok(songs.iter().map(Song::search_item).collect())
            }
            "playlist" => {
                let playlist_id =
                    parse_id(query).ok_or_else(|| anyhow!("playlist needs a playlist id or url"))?;
                let page: PlaylistApi = self
                    .get_json(&format!("https://api.deezer.com/playlist/{playlist_id}"))
                    .await?;
                Ok(page.tracks.data.iter().map(track_item).collect())
            }
            "artist_track" => {
                if let Ok(artist_id) = query.trim().parse::<i64>() {
                    let page: ApiPage<ApiTrack> = self
                        .get_json(&format!(
                            "https://api.deezer.com/artist/{artist_id}/top?limit=50"
                        ))
                        .await?;
                    return Ok(page.data.iter().map(track_item).collect());
                }

                let artists: ApiPage<ApiArtistItem> = self
                    .get_json(&format!("https://api.deezer.com/search/artist?q={encoded}"))
                    .await?;
                let needle = query.to_lowercase();
                let mut items = Vec::new();
                for artist in artists.data {
                    let name = artist.name.to_lowercase();
                    let excluded = ["karaoke", "famous by", "experience"]
                        .iter()
                        .any(|needle| name.contains(needle));
                    if !name.contains(&needle) || excluded {
                        continue;
                    }
                    let page: ApiPage<ApiTrack> = self
                        .get_json(&format!(
                            "https://api.deezer.com/artist/{}/top?limit=50",
                            artist.id
                        ))
                        .await?;
                    items.extend(page.data.iter().map(track_item));
                }
                Ok(items)
            }
            other => bail!("unsupported search type '{other}'"),
        }
    }
}

fn track_item(track: &ApiTrack) -> SearchItem {
    let album = track.album.as_ref();
    SearchItem {
        id: track.id.to_string(),
        id_type: "track".to_string(),
        title: track.title.clone(),
        album: album.map(|a| a.title.clone()).unwrap_or_default(),
        album_id: album.map(|a| a.id.to_string()).unwrap_or_default(),
        artist: track
            .artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        img_url: album
            .and_then(|a| a.cover_small.clone())
            .unwrap_or_default(),
        big_img_url: album
            .and_then(|a| a.cover_big.clone())
            .unwrap_or_default(),
        duration: format_duration(track.duration),
        link: track.link.clone().unwrap_or_default(),
        preview_url: track.preview.clone().unwrap_or_default(),
    }
}

fn album_item(album: &ApiAlbumItem) -> SearchItem {
    SearchItem {
        id: album.id.to_string(),
        id_type: "album".to_string(),
        title: String::new(),
        album: album.title.clone(),
        album_id: album.id.to_string(),
        artist: album
            .artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        img_url: album.cover_small.clone().unwrap_or_default(),
        big_img_url: album.cover_big.clone().unwrap_or_default(),
        duration: format_duration(album.duration),
        link: album.link.clone().unwrap_or_default(),
        preview_url: String::new(),
    }
}

fn artist_item(artist: &ApiArtistItem) -> SearchItem {
    SearchItem {
        id: artist.id.to_string(),
        id_type: "artist".to_string(),
        title: artist.name.clone(),
        album: format!("{} albums", artist.nb_album),
        album_id: artist.id.to_string(),
        artist: artist.name.clone(),
        img_url: artist.picture_small.clone().unwrap_or_default(),
        big_img_url: artist.picture_big.clone().unwrap_or_default(),
        duration: format!("{} fans", artist.nb_fan),
        link: artist.link.clone().unwrap_or_default(),
        preview_url: String::new(),
    }
}

pub fn parse_id(input: &str) -> Option<i64> {
    let mut digits = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    digits.parse().ok()
}

// ── gw-light payloads ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct SongsPage {
    #[serde(default)]
    data: Vec<Song>,
}

#[derive(Debug, Deserialize, Default)]
struct AlbumData {
    #[serde(rename = "ALB_TITLE", default, deserialize_with = "de_string")]
    title: String,
    #[serde(rename = "ART_NAME", default, deserialize_with = "de_string")]
    artist: String,
    #[serde(rename = "LABEL_NAME", default, deserialize_with = "de_string")]
    label: String,
    #[serde(
        rename = "PHYSICAL_RELEASE_DATE",
        default,
        deserialize_with = "de_string"
    )]
    physical_date: String,
    #[serde(
        rename = "DIGITAL_RELEASE_DATE",
        default,
        deserialize_with = "de_string"
    )]
    digital_date: String,
    #[serde(rename = "NUMBER_TRACK", default, deserialize_with = "de_string")]
    total_tracks: String,
}

impl AlbumData {
    fn into_meta(self) -> AlbumMeta {
        AlbumMeta {
            title: self.title,
            artist: self.artist,
            label: self.label,
            physical_date: self.physical_date,
            digital_date: self.digital_date,
            total_tracks: self.total_tracks,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct PageAlbumResults {
    #[serde(rename = "DATA", default)]
    data: AlbumData,
    #[serde(rename = "SONGS", default)]
    songs: SongsPage,
}

#[derive(Debug, Deserialize, Default)]
struct PageAlbumResponse {
    #[serde(default)]
    results: PageAlbumResults,
}

#[derive(Debug, Deserialize, Default)]
struct PlaylistData {
    #[serde(rename = "TITLE", default, deserialize_with = "de_string")]
    title: String,
}

#[derive(Debug, Deserialize, Default)]
struct PagePlaylistResults {
    #[serde(rename = "DATA", default)]
    data: PlaylistData,
    #[serde(rename = "SONGS", default)]
    songs: SongsPage,
}

#[derive(Debug, Deserialize, Default)]
struct PagePlaylistResponse {
    #[serde(default)]
    results: PagePlaylistResults,
}

// ── api.deezer.com payloads ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct ApiPage<T> {
    #[serde(default)]
    data: Vec<T>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiArtist {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct ApiAlbumRef {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    cover_small: Option<String>,
    #[serde(default)]
    cover_big: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiTrack {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    preview: Option<String>,
    #[serde(default)]
    artist: Option<ApiArtist>,
    #[serde(default)]
    album: Option<ApiAlbumRef>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiAlbumItem {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    cover_small: Option<String>,
    #[serde(default)]
    cover_big: Option<String>,
    #[serde(default)]
    artist: Option<ApiArtist>,
}

#[derive(Debug, Deserialize, Default)]
struct ApiArtistItem {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    nb_album: u64,
    #[serde(default)]
    nb_fan: u64,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    picture_small: Option<String>,
    #[serde(default)]
    picture_big: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PlaylistApi {
    #[serde(default)]
    tracks: ApiPage<ApiTrack>,
}

#[derive(Debug, Deserialize, Default)]
struct FavoritePage {
    #[serde(default)]
    data: Vec<FavoriteItem>,
}

#[derive(Debug, Deserialize, Default)]
struct FavoriteItem {
    #[serde(default)]
    id: i64,
}

#[derive(Debug, Deserialize, Default)]
struct ApiAlbumDetail {
    #[serde(default)]
    title: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    nb_tracks: Option<u64>,
    #[serde(default)]
    artist: Option<ApiArtist>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ids_from_urls() {
        assert_eq!(
            parse_id("https://www.deezer.com/us/playlist/908622995"),
            Some(908622995)
        );
        assert_eq!(parse_id("4329106072"), Some(4329106072));
        assert_eq!(parse_id("no digits here"), None);
    }

    #[test]
    fn song_deserializes_with_nulls() {
        let song: Song = serde_json::from_value(json!({
            "SNG_ID": "1",
            "SNG_TITLE": "Title",
            "ART_NAME": "Artist",
            "ALB_TITLE": null,
            "PHYSICAL_RELEASE_DATE": null,
            "DURATION": 183
        }))
        .unwrap();
        assert_eq!(song.title, "Title");
        assert_eq!(song.album, "");
        assert_eq!(song.duration, "183");
    }
}
