# deezer-downloader (Rust)

A Rust port of the local web app. One binary, embedded UI, same REST endpoints
as the Python version.

## Features

- Search tracks, albums, artists and playlists
- Download tracks, albums, public playlists and a user's favorites
- 320 kbps MP3 with Blowfish decryption, ID3v2.3/ID3v1 tags and cover art
- ZIP + `m3u8` for albums, playlists and favorites
- Task queue with the same states and progress reporting as the Python app

## Not supported

- MPD (`use_mpd` is ignored; the "Download & Play" buttons are hidden)
- FLAC (`flac_quality` is ignored; MP3 only)
- Spotify and youtube-dl
- The Files browser tab — downloads are written to disk directly

## Requirements

- Rust 1.85+
- A valid Deezer `arl` cookie in `app/settings.ini` (the same file the Python app uses)

## Build & run

```bash
cd rust
cargo build --release
cd ..
./rust/target/release/deezer-downloader
```

The binary looks for `app/settings.ini` in the current directory, then
`settings.ini`, then `../app/settings.ini`. It serves
<http://127.0.0.1:5000> and opens the browser automatically.

Environment overrides:

| Variable | Meaning |
| --- | --- |
| `DEEZER_SETTINGS` | Path to an alternative `settings.ini` |
| `DEEZER_ARL` | Override the ARL cookie |
| `HOST`, `PORT` | Override `[http] host` / `port` |
| `NO_OPEN` | Set to any value to skip opening the browser |

## Tests

```bash
cd rust
cargo test
```

The tests cover the Blowfish key derivation (checked against a Python
reference), the CBC round trip, ID3 frame generation, config interpolation and
URL/id parsing.
