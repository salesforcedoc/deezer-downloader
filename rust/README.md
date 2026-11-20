# deezer-downloader (Rust)

A local web app for downloading Deezer tracks, albums, public playlists and
favorites. One binary, embedded UI, small REST API.

## Features

- Search All, tracks, albums, artists and playlists
- Download tracks, albums, public playlists and a user's favorites
- 320 kbps MP3 with Blowfish decryption, ID3v2.3/ID3v1 tags and cover art
- ZIP + `m3u8` for albums, playlists and favorites
- Task queue with progress reporting

## Not supported

- MPD (`use_mpd` is ignored; the "Download & Play" buttons are hidden)
- FLAC (`flac_quality` is ignored; MP3 only)
- Spotify and youtube-dl
- The Files browser tab — downloads are written to disk directly

## Requirements

- Rust 1.85+
- A valid Deezer `arl` cookie in `settings.ini` at the repository root

## Build & run

From the repository root:

```bash
cargo build --release --manifest-path rust/Cargo.toml
./start.sh
```

`start.sh` builds the binary if it is missing, checks the port, starts the server
and opens <http://127.0.0.1:5000>.

Or run the binary directly:

```bash
./rust/target/release/deezer-downloader
```

The binary looks for `settings.ini` in the current directory, then
`../settings.ini`.

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

The tests cover the Blowfish key derivation (checked against a Python
reference), the CBC round trip, ID3 frame generation, config interpolation and
URL/id parsing.
