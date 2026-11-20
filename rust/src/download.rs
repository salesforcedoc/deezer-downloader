use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::crypto;
use crate::deezer::{AlbumMeta, Deezer, Song};
use crate::logging;
use crate::tags::{self, TagData};

pub fn clean_filename(name: &str) -> String {
    name.replace('/', "")
}

pub fn song_filename(song: &Song) -> String {
    let title = if song.version.is_empty() {
        song.title.clone()
    } else {
        format!("{} {}", song.title, song.version)
    };
    clean_filename(&format!("{} - {}.mp3", song.artist, title))
}

pub fn playlist_dir(playlists_dir: &Path, playlist_name: &str) -> PathBuf {
    playlists_dir.join(clean_filename(playlist_name))
}

pub fn favorites_dir(playlists_dir: &Path, user_id: &str) -> PathBuf {
    playlists_dir.join(format!("favorites_{user_id}"))
}

pub fn parent_name(files: &[PathBuf]) -> String {
    files
        .first()
        .and_then(|path| path.parent())
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string())
}

pub async fn download_song(
    deezer: &Deezer,
    song: &Song,
    output: &Path,
    album: Option<&AlbumMeta>,
) -> Result<PathBuf> {
    if song.sng_id.is_empty() {
        bail!("song has no SNG_ID");
    }

    let song_id = song
        .sng_id
        .parse::<i64>()
        .with_context(|| format!("invalid song id '{}'", song.sng_id))?;

    // Listing tokens (notably from pagePlaylist) are sometimes not entitled to
    // full media, so retry with a fresh token from song.getData.
    let filesize;
    let url = if song.track_token.is_empty() {
        let fresh = deezer.get_song(song_id).await?;
        let (url, size) = deezer.stream_url(&fresh.track_token).await?;
        filesize = size;
        url
    } else {
        match deezer.stream_url(&song.track_token).await {
            Ok((url, size)) => {
                filesize = size;
                url
            }
            Err(err) => {
                logging::log(format!(
                    "listing token for '{}' was rejected, fetching a fresh one: {err}",
                    song.title
                ));
                let fresh = deezer.get_song(song_id).await?;
                let (url, size) = deezer.stream_url(&fresh.track_token).await?;
                filesize = size;
                url
            }
        }
    };
    let cover = deezer.cover(&song.album_picture).await;

    let physical_date = if song.physical_release_date.is_empty() {
        album.map(|a| a.physical_date.as_str()).unwrap_or("")
    } else {
        song.physical_release_date.as_str()
    };
    let digital_date = if song.digital_release_date.is_empty() {
        album.map(|a| a.digital_date.as_str()).unwrap_or("")
    } else {
        song.digital_release_date.as_str()
    };

    let album_title = if song.album.is_empty() {
        album.map(|a| a.title.as_str()).unwrap_or("")
    } else {
        song.album.as_str()
    };
    let label = album.map(|a| a.label.as_str()).unwrap_or("");
    let total_tracks = album.map(|a| a.total_tracks.as_str()).unwrap_or("");

    let title = if song.version.is_empty() {
        song.title.clone()
    } else {
        format!("{} {}", song.title, song.version)
    };
    let date_ddmm = tags::ddmm_of(physical_date);
    let file_size = filesize
        .unwrap_or_else(|| song.filesize_mp3_320.parse().unwrap_or(0));

    let tag = TagData {
        title: &title,
        artist: &song.artist,
        album: album_title,
        track_number: &song.track_number,
        disk_number: &song.disk_number,
        year: tags::year_of(digital_date),
        original_year: tags::year_of(physical_date),
        date_ddmm: &date_ddmm,
        label,
        total_tracks,
        duration_secs: song.duration_secs(),
        isrc: &song.isrc,
        file_size,
        cover_jpeg: cover.as_deref(),
    };

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("could not create {}", parent.display()))?;
    }

    let mut file = tokio::fs::File::create(output)
        .await
        .with_context(|| format!("could not create {}", output.display()))?;

    file.write_all(&tags::build_id3v2(&tag)).await?;

    let response = deezer
        .client()
        .get(&url)
        .send()
        .await
        .context("audio download request failed")?;
    if !response.status().is_success() {
        bail!(
            "audio download for '{}' returned {}",
            song.title,
            response.status()
        );
    }

    let key = crypto::calc_bf_key(&song.sng_id);
    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::with_capacity(2048);
    let mut block_index: u64 = 0;
    let mut audio_bytes: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("reading the audio stream failed")?;
        let mut offset = 0;
        while offset < chunk.len() {
            let take = (2048 - buffer.len()).min(chunk.len() - offset);
            buffer.extend_from_slice(&chunk[offset..offset + take]);
            offset += take;

            if buffer.len() == 2048 {
                // Deezer encrypts every third 2048 byte block.
                if block_index % 3 == 0 {
                    crypto::decrypt_block_in_place(&mut buffer, &key)?;
                }
                file.write_all(&buffer).await?;
                audio_bytes += buffer.len() as u64;
                buffer.clear();
                block_index += 1;
            }
        }
    }

    if !buffer.is_empty() {
        file.write_all(&buffer).await?;
        audio_bytes += buffer.len() as u64;
    }

    file.write_all(&tags::build_id3v1(&tag)).await?;
    file.flush().await?;

    logging::log(format!(
        "Downloaded '{}' ({} bytes) -> {}",
        song.title,
        audio_bytes,
        output.display()
    ));

    Ok(output.to_path_buf())
}

pub async fn create_m3u8(dir: &Path, title: &str, files: &[PathBuf]) -> Result<PathBuf> {
    let path = dir.join(format!("00 {}.m3u8", clean_filename(title)));
    let mut content = String::new();
    for file in files {
        if let Some(name) = file.file_name() {
            content.push_str(&name.to_string_lossy());
            content.push('\n');
        }
    }
    tokio::fs::write(&path, content)
        .await
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(path)
}

pub async fn create_zip(
    zips_dir: &Path,
    parent_name: &str,
    files: &[PathBuf],
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(zips_dir)
        .await
        .with_context(|| format!("could not create {}", zips_dir.display()))?;

    let zip_path = zips_dir.join(format!("{parent_name}.zip"));
    let target = zip_path.clone();
    let entry_prefix = parent_name.to_string();
    let inputs: Vec<PathBuf> = files.to_vec();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::create(&target)?;
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for input in &inputs {
            if !input.exists() {
                continue;
            }
            let name = input
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "track.mp3".to_string());
            writer.start_file(format!("{entry_prefix}/{name}"), options)?;
            let mut source = std::fs::File::open(input)?;
            std::io::copy(&mut source, &mut writer)?;
        }

        writer.finish()?;
        Ok(())
    })
    .await
    .context("zip task panicked")??;

    Ok(zip_path)
}
