#!/usr/bin/env bash
set -euo pipefail

# Valen toolchain installer
# Usage: curl -fsSL https://raw.githubusercontent.com/NaruseNia/valen-lang/main/install.sh | bash

REPO="NaruseNia/valen-lang"
INSTALL_DIR="${VALEN_HOME:-$HOME/.valen}/bin"

info()  { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      error "unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)             error "unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

main() {
    local version="${1:-}"
    local target

    target="$(detect_target)"

    if [ -z "$version" ]; then
        info "Fetching latest release..."
        version="$(get_latest_version)"
        [ -n "$version" ] || error "could not determine latest version"
    fi

    local archive="valen-${version}-${target}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/${version}/${archive}"

    info "Installing Valen ${version} (${target})"

    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    info "Downloading ${url}..."
    curl -fSL "$url" -o "${tmp}/${archive}" || error "download failed — check version and platform"

    info "Extracting..."
    tar xzf "${tmp}/${archive}" -C "$tmp"

    mkdir -p "$INSTALL_DIR"
    cp "${tmp}/valen-${version}-${target}/valenc" "$INSTALL_DIR/"
    cp "${tmp}/valen-${version}-${target}/valen-lsp" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/valenc" "$INSTALL_DIR/valen-lsp"

    info "Installed to ${INSTALL_DIR}"

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        info ""
        info "Add to your PATH:"
        info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi

    info ""
    info "Done! Run 'valenc version' to verify."
}

main "$@"
