#!/bin/bash
set -u

# A shell script intended to use for installing Edgee for quick usage
# It does platform detection, fetch latest release informations and
# download the corresponding executable binary if available.
#
# Mostly inspired by the Quickwit installer script

# Package metadata

GITHUB_OWNER='edgee-cloud'
GITHUB_REPO='edgee'

# Helper utilities

_divider='--------------------------------------------------------------------------------'
_prompt='>>>'
_indent='    '

_header() {
    cat 1>&2 <<EOF
                                    E D G E E
                                    Installer

$_divider
Website:        https://www.edgee.cloud
Documentation:  https://docs.edgee.cloud/introduction
$_divider

EOF
}

_usage() {
    cat 1>&2 <<EOF
edgee-installer
The installer for Edgee (https://www.edgee.cloud)

USAGE:
    edgee-installer [FLAGS]

FLAGS:
    -h, --help      Print help informations
EOF
}

err() {
    echo "$_prompt Error: $*" >&2
    exit 1
}

has_command() {
    command -v "$1" &>/dev/null
}

check_command() {
    if ! has_command "$1"; then
        err "This install script requires \`$1\` and was not found."
    fi
}

check_commands() {
    for cmd in "$@"; do
        check_command "$cmd"
    done
}

# Main utilities

check_dependencies() {
    check_commands curl chmod
}

get_arch() {
    local _ostype _cputype
    _cputype="$(uname -m)"
    _ostype="$(uname -s)"

    case "$_cputype" in
        x86_64 | x86-64 | x64 | amd64)
            _cputype=x86_64
            ;;

        *)
            err "Unrecognized CPU type: $_cputype"
            ;;
    esac

    case "$_ostype" in
        Linux)
            _ostype="unknown-linux-gnu"
            ;;

        *)
            err "Unrecognized OS type: $_ostype"
            ;;
    esac

    echo "$_cputype-$_ostype"
}

download() {
    echo "Downloading: $1"
    curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location "$1" --output "$2"
}

download_latest() {
    local _arch
    _arch="$(get_arch)"

    download "https://github.com/$GITHUB_OWNER/$GITHUB_REPO/releases/latest/download/edgee.$_arch" edgee
    chmod +x edgee
}

main() {
    _header
    check_dependencies
    download_latest
}

# Entrypoint
main "$@" || exit 1
