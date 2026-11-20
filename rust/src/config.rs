use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub cookie_arl: String,
    pub songs_dir: PathBuf,
    pub albums_dir: PathBuf,
    pub zips_dir: PathBuf,
    pub playlists_dir: PathBuf,
}

#[derive(Debug, Default)]
struct Ini {
    sections: HashMap<String, HashMap<String, String>>,
}

impl Ini {
    fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let mut ini = Ini::default();
        let mut section = String::new();

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                ini.sections.entry(section.clone()).or_default();
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                ini.sections
                    .entry(section.clone())
                    .or_default()
                    .insert(key, value);
            }
        }
        Ok(ini)
    }

    fn raw(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(section)
            .and_then(|s| s.get(key))
            .map(String::as_str)
    }

    fn get(&self, section: &str, key: &str) -> Option<String> {
        let value = self.raw(section, key)?;
        Some(self.expand(section, value))
    }

    // settings.ini uses %(base)s / %(url_prefix)s style interpolation.
    fn expand(&self, section: &str, value: &str) -> String {
        let mut out = value.to_string();
        for _ in 0..5 {
            let mut changed = false;
            if let Some(entries) = self.sections.get(section) {
                for (key, val) in entries {
                    let pattern = format!("%({})s", key);
                    if out.contains(&pattern) {
                        out = out.replace(&pattern, val);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        out
    }
}

fn find_settings() -> PathBuf {
    if let Ok(path) = std::env::var("DEEZER_SETTINGS") {
        return PathBuf::from(path);
    }
    for candidate in ["settings.ini", "../settings.ini"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("settings.ini")
}

pub fn load() -> Result<Config> {
    let path = find_settings();
    let ini = Ini::load(&path)?;

    let host = std::env::var("HOST")
        .ok()
        .or_else(|| ini.get("http", "host"))
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| ini.get("http", "port").and_then(|p| p.parse().ok()))
        .unwrap_or(5000);
    let workers = ini
        .get("threadpool", "workers")
        .and_then(|w| w.parse().ok())
        .unwrap_or(4)
        .max(1);
    let cookie_arl = std::env::var("DEEZER_ARL")
        .ok()
        .or_else(|| ini.get("deezer", "cookie_arl"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if cookie_arl.is_empty() {
        bail!("cookie_arl is empty in {} (or set DEEZER_ARL)", path.display());
    }

    let dir = |key: &str| -> Result<PathBuf> {
        ini.get("download_dirs", key)
            .map(PathBuf::from)
            .with_context(|| format!("missing [download_dirs] {key} in {}", path.display()))
    };

    let config = Config {
        host,
        port,
        workers,
        cookie_arl,
        songs_dir: dir("songs")?,
        albums_dir: dir("albums")?,
        zips_dir: dir("zips")?,
        playlists_dir: dir("playlists")?,
    };

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_section_values() {
        let mut ini = Ini::default();
        ini.sections.insert(
            "dirs".into(),
            HashMap::from([
                ("base".to_string(), "/tmp/music".to_string()),
                ("songs".to_string(), "%(base)s/songs".to_string()),
            ]),
        );
        assert_eq!(ini.get("dirs", "songs").as_deref(), Some("/tmp/music/songs"));
    }
}
