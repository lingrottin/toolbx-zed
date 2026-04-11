#!/usr/bin/env bash

set -e

# Extract the version from Cargo.toml
VERSION=$(grep '^version =' Cargo.toml | head -n 1 | cut -d '"' -f 2)

if [ -z "$VERSION" ]; then
    echo "Error: Could not determine version from Cargo.toml"
    exit 1
fi

echo "Building toolbx-zed version $VERSION"

# Write VERSION_INFO
echo "$VERSION" > VERSION_INFO

# Ensure rustup targets are installed
echo "Adding rust targets..."
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-unknown-linux-musl

# Build GNU version
echo "Compiling x86_64-unknown-linux-gnu..."
cargo build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/toolbx-zed "toolbx-zed-${VERSION}-x86_64-unknown-linux-gnu"

# Build MUSL version
# Note: This requires a musl-gcc compiler installed on your system (e.g. `sudo apt install musl-tools`)
echo "Compiling x86_64-unknown-linux-musl..."
cargo build --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/toolbx-zed "toolbx-zed-${VERSION}-x86_64-unknown-linux-musl"

echo "Build complete! Artifacts generated:"
ls -lh "toolbx-zed-${VERSION}-x86_64-unknown-linux-gnu" "toolbx-zed-${VERSION}-x86_64-unknown-linux-musl" VERSION_INFO
