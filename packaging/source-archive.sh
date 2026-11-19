#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p dist
stage=$(mktemp -d "$root/dist/source.XXXXXX")
trap 'rm -rf "$stage"' EXIT
name=audiofuse-control-center-1.0.0
mkdir -p "$stage/$name/.cargo"
cp -r Cargo.toml Cargo.lock crates udev docs packaging README.md Makefile "$stage/$name/"
(cd "$stage/$name" && cargo vendor --locked vendor > .cargo/config.toml)
tar -C "$stage" -czf "dist/$name.tar.gz" "$name"
