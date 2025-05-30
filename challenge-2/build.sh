#!/bin/bash
set -e

echo "Building Bitcoin Address Generator WASM..."

# Install wasm-pack if not present
if ! command -v wasm-pack &>/dev/null; then
  echo "Installing wasm-pack..."
  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Run tests
echo "Running tests..."
cargo test

# Build WASM
echo "Building WASM..."
wasm-pack build --target web --out-dir pkg --release

# Check output
echo "Build complete! Files generated:"
ls -la pkg/

echo ""
echo "To serve:"
echo "python3 -m http.server 8000"
echo ""
echo "Then open:"
echo "http://localhost:8000"
