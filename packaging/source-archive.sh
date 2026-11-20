#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p dist
stage=$(mktemp -d "$root/dist/source.XXXXXX")
trap 'rm -rf "$stage"' EXIT
name=audiofuse-control-center-1.0.0
mkdir -p "$stage/$name/.cargo"
mkdir "$stage/$name/kmod"
cp kmod/{audiofuse8pre_ctl.c,audiofuse8pre_ctl.h,audiofuse8pre_policy.h,test_policy.c,Makefile,dkms.conf} "$stage/$name/kmod/"
cp -r tests Cargo.toml Cargo.lock crates udev docs packaging README.md Makefile "$stage/$name/"
(cd "$stage/$name" && cargo vendor --locked vendor > .cargo/config.toml)
tar -C "$stage" -czf "dist/$name.tar.gz" "$name"
