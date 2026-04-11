#!/usr/bin/env bash

set -e

echo "Installing toolbx-zed..."

# 1. Check if it is Linux
if [ "$(uname -s)" != "Linux" ]; then
    echo "Error: This tool is only supported on Linux."
    exit 1
fi

ARCH=$(uname -m)
TEMP_DIR=$(mktemp -d)

# Helper to check if glibc is < 2.39 or missing
requires_musl() {
    if ! command -v ldd &> /dev/null; then
        return 0 # ldd not found, likely Alpine/musl
    fi
    local version_output
    version_output=$(ldd --version 2>&1 | head -n 1 || true)
    if [[ ! "$version_output" == *"GNU"* && ! "$version_output" == *"GLIBC"* ]]; then
        return 0 # Not GNU libc
    fi
    local version
    version=$(echo "$version_output" | grep -oP '[0-9]+\.[0-9]+' | head -n 1 || true)
    if [ -z "$version" ]; then
        return 0
    fi

    # Check if version is less than 2.39
    if [ "$(printf '%s\n' "2.39" "$version" | sort -V | head -n1)" = "$version" ] && [ "$version" != "2.39" ]; then
        return 0 # < 2.39
    fi

    return 1 # >= 2.39, can use gnu
}

# 2. Determine installation method
BUILD_FROM_SOURCE=false
if [ -n "$TOOLBX_ZED_BUILD_FROM_SOURCE" ] || [ "$ARCH" != "x86_64" ]; then
    BUILD_FROM_SOURCE=true
fi

# We use github repo URL
REPO="lingrottin/toolbx-zed"

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
INSTALL_DIR="$DATA_HOME/toolbx-zed"

if [ "$BUILD_FROM_SOURCE" = true ]; then
    echo "Installing from source..."

    # Check if cargo exists
    if ! command -v cargo &> /dev/null; then
        echo "Error: 'cargo' is not installed or not in PATH. Please install Rust toolchain first."
        exit 1
    fi

    FETCH_METHOD=""
    if command -v git &> /dev/null; then
        FETCH_METHOD="git"
    elif command -v curl &> /dev/null && command -v unzip &> /dev/null; then
        FETCH_METHOD="curl"
    else
        echo "Error: Neither 'git' nor 'curl' + 'unzip' were found. Please install them to download the source."
        exit 1
    fi

    cd "$TEMP_DIR"
    echo "Fetching source code using $FETCH_METHOD..."
    if [ "$FETCH_METHOD" = "git" ]; then
        git clone "https://github.com/${REPO}.git"
        cd toolbx-zed
    else
        curl -sL "https://github.com/${REPO}/archive/refs/heads/main.zip" -o main.zip
        unzip -q main.zip
        cd toolbx-zed-main
    fi

    echo "Compiling toolbx-zed in release mode..."
    cargo build --release

    echo "Installing binary to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR"
    cp target/release/toolbx-zed "$INSTALL_DIR/zed"

else
    echo "Installing from pre-compiled binary..."

    if ! command -v curl &> /dev/null; then
        echo "Error: 'curl' is required to download binaries."
        exit 1
    fi

    # Fetch VERSION_INFO
    echo "Fetching latest version info..."
    VERSION=$(curl -sL "https://raw.githubusercontent.com/${REPO}/main/VERSION_INFO" | tr -d '[:space:]')

    if [ -z "$VERSION" ]; then
        echo "Error: Could not determine latest version."
        exit 1
    fi

    echo "Latest version is $VERSION"

    TARGET_SUFFIX=""
    if requires_musl; then
        echo "System glibc < 2.39 or not found. Selecting musl build."
        TARGET_SUFFIX="x86_64-unknown-linux-musl"
    else
        echo "System glibc >= 2.39. Selecting gnu build."
        TARGET_SUFFIX="x86_64-unknown-linux-gnu"
    fi

    FILENAME="toolbx-zed-${VERSION}-${TARGET_SUFFIX}"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${FILENAME}"

    cd "$TEMP_DIR"
    echo "Downloading ${FILENAME}..."
    if ! curl -sL -f "$DOWNLOAD_URL" -o "toolbx-zed"; then
        echo "Error: Failed to download the binary. The release might not exist yet."
        echo "You can set TOOLBX_ZED_BUILD_FROM_SOURCE=1 to force building from source."
        exit 1
    fi

    chmod +x toolbx-zed

    echo "Installing binary to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR"
    cp toolbx-zed "$INSTALL_DIR/zed"
fi

# 5. Add environment variable to shell rc file
SHELL_RC=""
case "$SHELL" in
    */zsh) SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
    */fish) SHELL_RC="$HOME/.config/fish/config.fish" ;;
    *) SHELL_RC="$HOME/.profile" ;;
esac

if [ -n "$SHELL_RC" ]; then
    touch "$SHELL_RC"
    if ! grep -q "$INSTALL_DIR" "$SHELL_RC"; then
        echo "Adding $INSTALL_DIR to PATH in $SHELL_RC..."
        if [[ "$SHELL" == *"fish"* ]]; then
            echo "fish_add_path \"$INSTALL_DIR\"" >> "$SHELL_RC"
        else
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$SHELL_RC"
        fi
        echo "Please restart your shell or run 'source $SHELL_RC' to apply the PATH changes."
    else
        echo "$INSTALL_DIR is already in $SHELL_RC."
    fi
else
    echo "Could not detect shell rc file. Please manually add $INSTALL_DIR to your PATH."
fi

# 6. Clean up
echo "Cleaning up temporary files..."
cd "$HOME"
rm -rf "$TEMP_DIR"

# 7. Check if zed is installed via flatpak
if command -v flatpak &> /dev/null; then
    if flatpak list --app | grep -qi "dev.zed.Zed"; then
        echo ""
        echo "========================================================================="
        echo "Note:"
        echo "We detected that you are using the Flatpak version of Zed."
        echo ""
        echo "We need to enable the home directory access permission (--filesystem="
        echo "home) for Zed to use toolbx-zed properly. This is enabled by default in"
        echo "the Zed Flatpak. If you don't know what this means, it likely means that"
        echo "the permission is correct and no action is required."
        echo "========================================================================="
        echo ""
    fi
fi

echo "Installation Successful! Now you can invoke toolbx-zed using 'zed' command in your terminal."
echo ""
echo "Usage:"
echo "$ toolbox enter <container>"
echo "$ zed <some_path>"
