#!/bin/bash
set -e
cd "$(dirname "$0")"

PORT_TO_CHECK="${PORT:-5000}"
if lsof -nP -iTCP:"$PORT_TO_CHECK" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "Port $PORT_TO_CHECK is already in use (the Python app?)."
    echo "Stop it first, or run: PORT=5001 ./start-rust.sh"
    exit 1
fi

BIN=rust/target/release/deezer-downloader
if [ ! -x "$BIN" ]; then
    echo "Building the Rust app..."
    (cd rust && cargo build --release)
fi

exec "$BIN"
