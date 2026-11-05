#!/bin/sh
# Installs the fennec Linux binary from GitHub Releases.
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/ikarys/fennec/main/install.sh | sh
#
# Environment:
#   FENNEC_VERSION       release tag to install (default: latest; edge is supported)
#   FENNEC_INSTALL_DIR   install directory (default: $HOME/.local/bin)
#   FENNEC_REPO_URL      repository URL for releases (default: https://github.com/ikarys/fennec)
#   FENNEC_SERVICE       none: do not manage a systemd user service; keep: do not restart an existing service

set -eu

fail() {
    printf 'fennec-install: %s\n' "$1" >&2
    exit 1
}

warn() {
    printf 'fennec-install: warning: %s\n' "$1" >&2
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        fail "required command not found: $1"
    fi
}

detect_platform() {
    OS_NAME="$(uname -s)"
    if [ "$OS_NAME" != "Linux" ]; then
        fail "not a Linux system: $OS_NAME"
    fi

    case "$(uname -m)" in
        x86_64|amd64)
            TARGET="x86_64-unknown-linux-gnu"
            ;;
        aarch64|arm64)
            TARGET="aarch64-unknown-linux-gnu"
            ;;
        *)
            fail "unsupported architecture: $(uname -m)"
            ;;
    esac
}

resolve_latest_tag() {
    # GitHub's /releases/latest redirect encodes the tag without an API call.
    if ! TAG_URL="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$REPO_URL/releases/latest")"; then
        fail "could not resolve latest release tag from $REPO_URL"
    fi

    case "$TAG_URL" in
        */tag/*)
            ;;
        *)
            fail "latest release redirect did not end with /tag/: $TAG_URL"
            ;;
    esac

    VERSION="${TAG_URL##*/}"
    if [ -z "$VERSION" ] || [ "$VERSION" = "latest" ]; then
        fail "latest release did not resolve to a tag: $TAG_URL"
    fi
}

download_asset() {
    if ! curl -fsSL -o "$2" "$1"; then
        fail "download failed: $1"
    fi
}

verify_checksum() {
    # The checksum file names the asset relative to the temp directory.
    if ! (cd "$1" && sha256sum -c "$2" >/dev/null 2>&1); then
        fail "checksum mismatch for $2"
    fi
}

extract_tarball() {
    if ! tar -xzf "$1" -C "$2"; then
        fail "extraction failed: $1"
    fi
}

install_binary() {
    mkdir -p "$2" || fail "could not create install directory: $2"
    # Copy next to the target then rename: overwriting a running fennec in place
    # fails with "Text file busy", a rename swaps it atomically.
    cp "$1" "$2/.fennec.new" && chmod 755 "$2/.fennec.new" && mv -f "$2/.fennec.new" "$2/fennec" \
        || fail "could not install binary to $2/fennec"
}

install_service() {
    dir="$1"
    if [ "${FENNEC_SERVICE:-}" = "none" ]; then
        return 0
    fi

    if ! systemctl --user show-environment >/dev/null 2>&1; then
        warn "no systemd user session: start fennec by hand (fennec serve)"
        return 0
    fi

    case "$dir" in
        /*)
            ;;
        *)
            dir="$(pwd)/$dir"
            ;;
    esac

    unit_dir="${XDG_CONFIG_HOME:-${HOME:-}/.config}/systemd/user"
    unit="$unit_dir/fennec.service"

    if [ ! -f "$unit" ]; then
        mkdir -p "$unit_dir" || fail "could not create systemd unit directory: $unit_dir"
        # A unit owned by the user is never overwritten: if one exists we only
        # manage its state below.
        {
            printf '[Unit]\n'
            printf 'Description=fennec\n'
            printf '\n'
            printf '[Service]\n'
            printf 'ExecStart=%s/fennec serve\n' "$dir"
            printf 'Restart=on-failure\n'
            printf '\n'
            printf '[Install]\n'
            printf 'WantedBy=default.target\n'
        } > "$unit" || fail "could not write systemd unit: $unit"
        systemctl --user daemon-reload || fail "systemd daemon-reload failed"
        systemctl --user enable --now fennec || fail "could not enable fennec service"
        printf 'fennec runs as a systemd user service: journalctl --user -u fennec -f\n'
    else
        if [ "${FENNEC_SERVICE:-}" = "keep" ]; then
            return 0
        fi
        if systemctl --user is-active --quiet fennec; then
            systemctl --user restart fennec || fail "could not restart fennec service"
        fi
    fi
}

path_contains_dir() {
    case ":${PATH:-}:" in
        *":$1:"*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

warn_glibc() {
    # Advisory: the binary is built against glibc 2.35. An unparseable version
    # (musl, no ldd) says nothing.
    version="$(ldd --version 2>&1 | head -n 1 | sed -n 's/.* \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')" || return 0
    [ -n "$version" ] || return 0
    major="${version%.*}"
    minor="${version#*.}"
    if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 35 ]; }; then
        warn "glibc $version is older than 2.35: fennec will not start; build it from source"
    fi
}

main() {
    VERSION="${FENNEC_VERSION:-latest}"

    if [ -n "${FENNEC_INSTALL_DIR:-}" ]; then
        INSTALL_DIR="$FENNEC_INSTALL_DIR"
    else
        INSTALL_DIR="${HOME:-}/.local/bin"
    fi

    REPO_URL="${FENNEC_REPO_URL:-https://github.com/ikarys/fennec}"
    REPO_URL="${REPO_URL%/}"

    detect_platform
    require_command "curl"
    require_command "tar"
    require_command "sha256sum"

    if [ "$VERSION" = "latest" ]; then
        resolve_latest_tag
    fi

    ASSET="fennec-$VERSION-$TARGET.tar.gz"
    CHECKSUM="$ASSET.sha256"

    if ! TMP_DIR="$(mktemp -d)"; then
        fail "could not create temporary directory"
    fi
    trap 'rm -rf "$TMP_DIR"' EXIT

    download_asset "$REPO_URL/releases/download/$VERSION/$ASSET" "$TMP_DIR/$ASSET"
    download_asset "$REPO_URL/releases/download/$VERSION/$CHECKSUM" "$TMP_DIR/$CHECKSUM"

    verify_checksum "$TMP_DIR" "$CHECKSUM"
    extract_tarball "$TMP_DIR/$ASSET" "$TMP_DIR"

    SOURCE="$TMP_DIR/fennec-$VERSION-$TARGET/fennec"
    if [ ! -f "$SOURCE" ]; then
        fail "binary not found in tarball: $SOURCE"
    fi

    install_binary "$SOURCE" "$INSTALL_DIR"
    install_service "$INSTALL_DIR"

    warn_glibc
    if ! command -v bwrap >/dev/null 2>&1; then
        warn "the command sandbox needs bubblewrap: install the bubblewrap package"
    fi
    if ! path_contains_dir "$INSTALL_DIR"; then
        printf 'fennec-install: warning: %s is not in PATH; add it with:\n' "$INSTALL_DIR" >&2
        printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR" >&2
    fi

    printf 'fennec %s installed to %s\n' "$VERSION" "$INSTALL_DIR/fennec"
    "$INSTALL_DIR/fennec" --version
}

main "$@"
