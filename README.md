# Deezer Downloader

A local web app for downloading Deezer tracks, albums, public playlists and
favorites. It is written in Rust and builds to a single binary with the web UI
embedded.

## Features

- Search **All**, tracks, albums, artists and playlists
- Download tracks, albums, public playlists and a user's favorites
- 320 kbps MP3 with Blowfish decryption, ID3v2.3/ID3v1 tags and cover art
- ZIP + `m3u8` for albums, playlists and favorites
- Task queue with progress and per-task state

## Requirements

- Rust 1.85+ (to build)
- A Deezer account and its `arl` cookie

## Configuration

Copy `settings.ini.example` to `settings.ini` and set:

- `[deezer] cookie_arl` — the `arl` cookie from a logged-in deezer.com session
  (browser devtools → Application/Storage → Cookies → `arl`)
- `[download_dirs] base` — where downloads are written
- optionally `[http] port` and `[threadpool] workers`

`settings.ini` is gitignored. The binary looks for it in the current directory,
then `../settings.ini`.

## Build and run

```bash
cargo build --release --manifest-path rust/Cargo.toml
./start.sh
```

`start.sh` builds the binary if it is missing, refuses to start if the port is
already in use, and opens <http://127.0.0.1:5000>.

You can also run the binary directly:

```bash
./rust/target/release/deezer-downloader
```

Environment overrides:

| Variable | Meaning |
| --- | --- |
| `DEEZER_SETTINGS` | Path to an alternative `settings.ini` |
| `DEEZER_ARL` | Override the ARL cookie |
| `HOST`, `PORT` | Override `[http] host` / `port` |
| `NO_OPEN` | Set to any value to skip opening the browser |

## Tests

```bash
cargo test --manifest-path rust/Cargo.toml
```

## Layout

- `rust/` — the application (see [`rust/README.md`](rust/README.md))
- `settings.ini.example` — configuration template
- `start.sh` — build/run launcher

## Not supported

MPD, FLAC (MP3 only), Spotify, youtube-dl and the browser file index. Downloads
are written directly to the configured directory.

## License

MIT — see [`LICENSE`](LICENSE).
