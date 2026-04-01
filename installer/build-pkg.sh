#!/bin/bash
set -e

VERSION="0.1.0"
BUILD_DIR="$(mktemp -d)"

echo "Building release binaries..."
cargo build --release -p pixma-cli
cargo build --release -p pixma-bridge --bin pixma-bridge

echo "Assembling payload..."
mkdir -p "$BUILD_DIR/usr/local/bin"
mkdir -p "$BUILD_DIR/usr/local/share/pixma"

cp target/release/pixma "$BUILD_DIR/usr/local/bin/"
cp target/release/pixma-bridge "$BUILD_DIR/usr/local/bin/"
cp installer/com.pdrgds.pixma-bridge.plist "$BUILD_DIR/usr/local/share/pixma/"

echo "Building .pkg..."
pkgbuild \
  --identifier "com.pdrgds.pixma-rs" \
  --version "$VERSION" \
  --root "$BUILD_DIR" \
  --install-location "/" \
  --scripts installer/scripts/ \
  "PixmaDriver-${VERSION}.pkg"

rm -rf "$BUILD_DIR"
echo "Done: PixmaDriver-${VERSION}.pkg"
