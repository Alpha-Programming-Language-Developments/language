#!/usr/bin/env bash
set -e

echo "===================================================================="
echo "          ALPHA LANGUAGE FULL RELEASE BUILD SCRIPT (BASH)"
echo "===================================================================="
echo ""

# 1. Check for Cargo
if ! command -v cargo &> /dev/null; then
    echo "[ERROR] 'cargo' is not found in PATH!"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

# 2. Check for Clang / LLVM
if ! command -v clang &> /dev/null; then
    echo "[WARNING] Clang / LLVM not found in PATH."
    echo "Direct LLVM backend requires Clang."
fi

# 3. Compile full workspace in release mode
echo "[1/3] Building Alpha Compiler, LSP Server, and Precompiled Static Runtime..."
cargo build --workspace --release

# 4. Locate artifacts
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/target/release"

EXE_EXT=""
LIB_NAME="libalpha_rt.a"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" || "$OSTYPE" == "cygwin" ]]; then
    EXE_EXT=".exe"
    LIB_NAME="alpha_rt.lib"
fi

ALPHA_BIN="$TARGET_DIR/alpha$EXE_EXT"
LSP_BIN="$TARGET_DIR/alpha-lsp$EXE_EXT"
RT_LIB="$TARGET_DIR/$LIB_NAME"

if [[ ! -f "$ALPHA_BIN" ]]; then
    echo "[ERROR] Expected binary not found: $ALPHA_BIN"
    exit 1
fi

if [[ ! -f "$LSP_BIN" ]]; then
    echo "[ERROR] Expected binary not found: $LSP_BIN"
    exit 1
fi

if [[ ! -f "$RT_LIB" ]]; then
    echo "[ERROR] Expected runtime library not found: $RT_LIB"
    exit 1
fi

# 5. Install to Cargo bin
INSTALL_DIR="$HOME/.cargo/bin"
mkdir -p "$INSTALL_DIR"

echo ""
echo "[2/3] Installing binaries to $INSTALL_DIR ..."
cp -f "$ALPHA_BIN" "$INSTALL_DIR/alpha$EXE_EXT"
cp -f "$LSP_BIN" "$INSTALL_DIR/alpha-lsp$EXE_EXT"
cp -f "$RT_LIB" "$INSTALL_DIR/$LIB_NAME"

# 6. Done
echo "[3/3] Installation complete!"
echo ""
echo "===================================================================="
echo "[SUCCESS] Alpha Full Release Build Completed:"
echo "  - Compiler:       $INSTALL_DIR/alpha$EXE_EXT"
echo "  - LSP Server:     $INSTALL_DIR/alpha-lsp$EXE_EXT"
echo "  - Static Runtime: $INSTALL_DIR/$LIB_NAME"
echo "===================================================================="
echo ""
echo "Run 'alpha -v' or 'alpha help' to get started."
