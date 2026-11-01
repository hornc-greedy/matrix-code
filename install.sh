#!/bin/sh
# Downloads the latest release for this system, checks it against SHA256SUMS
# and runs its install command.
set -eu

release=https://github.com/hornc-greedy/matrix-code/releases/latest/download

# The whole script is one function, so a download cut off by `curl | sh`
# runs nothing
main() {
    case "$(uname -s) $(uname -m)" in
    "Linux x86_64") target=x86_64-unknown-linux-musl ;;
    "Linux aarch64") target=aarch64-unknown-linux-musl ;;
    "Darwin arm64") target=aarch64-apple-darwin ;;
    *)
        echo "matrix-code: no release for $(uname -s) on $(uname -m)" >&2
        exit 1
        ;;
    esac
    archive="matrix-code-$target.tar.gz"

    folder=$(mktemp -d)
    trap 'rm -rf "$folder"' EXIT
    cd "$folder"

    curl -fsSLO "$release/$archive"
    curl -fsSLO "$release/SHA256SUMS"
    # The archive's own line on stdin works with GNU, busybox and macOS tools
    if [ "$(uname -s)" = Darwin ]; then
        grep " $archive\$" SHA256SUMS | shasum -a 256 -c -
    else
        grep " $archive\$" SHA256SUMS | sha256sum -c -
    fi

    tar -xzf "$archive"
    ./matrix-code/matrix-code install
}

main
