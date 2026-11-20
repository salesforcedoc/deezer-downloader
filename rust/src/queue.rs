use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::Serialize;
use tokio::sync::Semaphore;

use crate::config::Config;
use crate::deezer::Deezer;
use crate::download;
use crate::logging;

#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub id: u64,
    pub description: String,
    pub args: String,
    pub state: String,
    pub result: String,
    pub exception: String,
    pub progress: [u32; 2],
}

#[derive(Debug, Clone)]
pub enum Job {
    Track {
        track_id: i64,
    },
    Album {
        album_id: i64,
        create_zip: bool,
    },
    Playlist {
        playlist_id: i64,
        create_zip: bool,
    },
    Favorites {
        user_id: String,
        create_zip: bool,
    },
}

impl Job {
    fn args(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub deezer: Arc<Deezer>,
    pub tasks: Arc<Mutex<Vec<TaskInfo>>>,
    permits: Arc<Semaphore>,
    next_id: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(config: Config, deezer: Deezer) -> Self {
        let permits = Arc::new(Semaphore::new(config.workers));
        Self {
            config: Arc::new(config),
            deezer: Arc::new(deezer),
            tasks: Arc::new(Mutex::new(Vec::new())),
            permits,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn enqueue(&self, description: impl Into<String>, job: Job) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.tasks.lock().unwrap().push(TaskInfo {
            id,
            description: description.into(),
            args: job.args(),
            state: "waiting".to_string(),
            result: String::new(),
            exception: String::new(),
            progress: [0, 0],
        });

        let state = self.clone();
        tokio::spawn(async move {
            let permit = state.permits.clone().acquire_owned().await;
            match permit {
                Ok(_permit) => {
                    state.set_state(id, "active");
                    match run_job(&state, id, job).await {
                        Ok(paths) => {
                            state.set_result(id, paths.join(", "));
                            state.set_state(id, "mission accomplished");
                        }
                        Err(err) => {
                            logging::error(format!("task {id} failed: {err:#}"));
                            state.set_exception(id, format!("{err:#}"));
                            state.set_state(id, "failed");
                        }
                    }
                }
                Err(err) => {
                    state.set_exception(id, format!("queue closed: {err}"));
                    state.set_state(id, "failed");
                }
            }
        });

        id
    }

    fn with_task(&self, id: u64, update: impl FnOnce(&mut TaskInfo)) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.iter_mut().find(|task| task.id == id) {
            update(task);
        }
    }

    fn set_state(&self, id: u64, new_state: &str) {
        self.with_task(id, |task| task.state = new_state.to_string());
    }

    fn set_result(&self, id: u64, result: String) {
        self.with_task(id, |task| task.result = result);
    }

    fn set_exception(&self, id: u64, exception: String) {
        self.with_task(id, |task| task.exception = exception);
    }

    fn set_progress(&self, id: u64, value: u32, maximum: u32) {
        self.with_task(id, |task| task.progress = [value, maximum]);
    }
}

async fn run_job(state: &AppState, id: u64, job: Job) -> Result<Vec<String>> {
    match job {
        Job::Track { track_id } => {
            let song = state.deezer.get_song(track_id).await?;
            let album = match song.album_id.parse::<i64>() {
                Ok(album_id) => state.deezer.album_meta(album_id).await.ok(),
                Err(_) => None,
            };
            let path = state
                .config
                .songs_dir
                .join(download::song_filename(&song));
            download::download_song(&state.deezer, &song, &path, album.as_ref()).await?;
            Ok(vec![path.display().to_string()])
        }

        Job::Album {
            album_id,
            create_zip,
        } => {
            let (meta, songs) = state.deezer.page_album(album_id).await?;
            let dir = state.config.albums_dir.join(download::clean_filename(&format!(
                "{} - {}",
                meta.artist, meta.title
            )));
            let mut paths = Vec::new();

            for (index, song) in songs.iter().enumerate() {
                state.set_progress(id, index as u32, songs.len() as u32);
                let path = dir.join(download::song_filename(song));
                download::download_song(&state.deezer, song, &path, Some(&meta)).await?;
                paths.push(path);
            }
            state.set_progress(id, songs.len() as u32, songs.len() as u32);

            if create_zip {
                let parent = download::parent_name(&paths);
                let zip = download::create_zip(&state.config.zips_dir, &parent, &paths).await?;
                return Ok(vec![zip.display().to_string()]);
            }
            Ok(paths.iter().map(|p| p.display().to_string()).collect())
        }

        Job::Playlist {
            playlist_id,
            create_zip,
        } => {
            let (name, songs) = state.deezer.page_playlist(playlist_id).await?;
            let dir = download::playlist_dir(&state.config.playlists_dir, &name);
            let mut paths = Vec::new();

            for (index, song) in songs.iter().enumerate() {
                state.set_progress(id, index as u32, songs.len() as u32);
                let path = dir.join(download::song_filename(song));
                download::download_song(&state.deezer, song, &path, None).await?;
                paths.push(path);
            }
            state.set_progress(id, songs.len() as u32, songs.len() as u32);

            let m3u8 = download::create_m3u8(&dir, &name, &paths).await?;
            let mut all = paths.clone();
            all.push(m3u8);

            if create_zip {
                let parent = download::parent_name(&paths);
                let zip = download::create_zip(&state.config.zips_dir, &parent, &all).await?;
                return Ok(vec![zip.display().to_string()]);
            }
            Ok(all.iter().map(|p| p.display().to_string()).collect())
        }

        Job::Favorites {
            user_id,
            create_zip,
        } => {
            let song_ids = state.deezer.favorites(&user_id).await?;
            let dir = download::favorites_dir(&state.config.playlists_dir, &user_id);
            let mut paths = Vec::new();

            for (index, song_id) in song_ids.iter().enumerate() {
                state.set_progress(id, index as u32, song_ids.len() as u32);
                match state.deezer.get_song(*song_id).await {
                    Ok(song) => {
                        let path = dir.join(download::song_filename(&song));
                        match download::download_song(&state.deezer, &song, &path, None).await {
                            Ok(path) => paths.push(path),
                            Err(err) => logging::error(format!(
                                "skipping favorite {song_id}: {err:#}"
                            )),
                        }
                    }
                    Err(err) => {
                        logging::error(format!("skipping favorite {song_id}: {err:#}"))
                    }
                }
            }
            state.set_progress(id, song_ids.len() as u32, song_ids.len() as u32);

            if paths.is_empty() {
                return Ok(Vec::new());
            }

            let title = format!("favorites_{user_id}");
            let m3u8 = download::create_m3u8(&dir, &title, &paths).await?;
            let mut all = paths.clone();
            all.push(m3u8);

            if create_zip {
                let parent = download::parent_name(&paths);
                let zip = download::create_zip(&state.config.zips_dir, &parent, &all).await?;
                return Ok(vec![zip.display().to_string()]);
            }
            Ok(all.iter().map(|p| p.display().to_string()).collect())
        }
    }
}
