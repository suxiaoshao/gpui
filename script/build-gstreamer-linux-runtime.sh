#!/usr/bin/env bash
# Produce an x86_64 Linux GStreamer prefix suitable for the private-runtime
# staging helper. This is a release-producer script: it intentionally fetches
# the pinned Cerbero source and its build inputs when it runs in CI.
set -euo pipefail

readonly CERBERO_REPOSITORY="https://gitlab.freedesktop.org/gstreamer/cerbero.git"
readonly CERBERO_COMMIT="78666745b34b6245a85510ac47a03a5033af4711"
readonly SUPPORTED_TARGET="x86_64-unknown-linux-gnu"
readonly PRODUCER_DISTRIBUTION="ubuntu"
readonly PRODUCER_VERSION="22.04"
readonly MINIMUM_GLIBC="2.35"

usage() {
    echo "usage: $0 --output <empty-directory> [--work-dir <directory>]" >&2
}

output=""
work_dir="${GPUI_GSTREAMER_CERBERO_WORK_DIR:-target/gstreamer-cerbero}"
while (($# > 0)); do
    case "$1" in
        --output)
            output="${2:-}"
            shift 2
            ;;
        --work-dir)
            work_dir="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
done

if [[ -z "$output" ]]; then
    usage
    exit 2
fi
if [[ "$(uname -m)" != "x86_64" ]]; then
    echo "private runtime producer requires x86_64, got $(uname -m)" >&2
    exit 1
fi
if [[ ! -r /etc/os-release ]]; then
    echo "private runtime producer requires ${PRODUCER_DISTRIBUTION} ${PRODUCER_VERSION}" >&2
    exit 1
fi
# shellcheck disable=SC1091
source /etc/os-release
if [[ "${ID:-}" != "$PRODUCER_DISTRIBUTION" || "${VERSION_ID:-}" != "$PRODUCER_VERSION" ]]; then
    echo "private runtime producer requires ${PRODUCER_DISTRIBUTION} ${PRODUCER_VERSION}; got ${ID:-unknown} ${VERSION_ID:-unknown}" >&2
    exit 1
fi
glibc_version="$(getconf GNU_LIBC_VERSION | awk '{print $2}')"
if [[ -z "$glibc_version" || "$(printf '%s\n%s\n' "$MINIMUM_GLIBC" "$glibc_version" | sort -V | head -n1)" != "$MINIMUM_GLIBC" ]]; then
    echo "private runtime producer requires glibc >= ${MINIMUM_GLIBC}; got ${glibc_version:-unknown}" >&2
    exit 1
fi

for command in git python3 tar sha256sum; do
    command -v "$command" >/dev/null || {
        echo "required producer command is missing: $command" >&2
        exit 1
    }
done

if [[ -e "$output" ]] && [[ -n "$(find "$output" -mindepth 1 -print -quit)" ]]; then
    echo "output directory must be empty: $output" >&2
    exit 1
fi
mkdir -p "$output" "$work_dir"
cerbero_dir="$work_dir/cerbero"
if [[ ! -d "$cerbero_dir/.git" ]]; then
    git clone --no-checkout "$CERBERO_REPOSITORY" "$cerbero_dir"
fi
git -C "$cerbero_dir" fetch --depth 1 origin "$CERBERO_COMMIT"
git -C "$cerbero_dir" checkout --detach "$CERBERO_COMMIT"
if [[ "$(git -C "$cerbero_dir" rev-parse HEAD)" != "$CERBERO_COMMIT" ]]; then
    echo "Cerbero checkout does not match pinned commit $CERBERO_COMMIT" >&2
    exit 1
fi

cerbero="$cerbero_dir/cerbero-uninstalled"
if [[ ! -x "$cerbero" ]]; then
    echo "Cerbero entrypoint is missing: $cerbero" >&2
    exit 1
fi

"$cerbero" bootstrap
package_dir="$work_dir/packages"
mkdir -p "$package_dir"
for package in \
    gstreamer-1.0-core \
    gstreamer-1.0-system \
    gstreamer-1.0-playback \
    gstreamer-1.0-codecs \
    gstreamer-1.0-libav; do
    "$cerbero" package \
        --tarball \
        --force \
        --compress-method xz \
        --output-dir "$package_dir" \
        "$package"
done

mapfile -t package_archives < <(find "$package_dir" -type f -name '*.tar.xz' -print | sort)
if ((${#package_archives[@]} == 0)); then
    echo "Cerbero did not produce runtime/development tarballs" >&2
    exit 1
fi
for archive in "${package_archives[@]}"; do
    tar -xJf "$archive" -C "$output"
done
prefix="$output"
for required in \
    "$prefix/bin/gst-inspect-1.0" \
    "$prefix/lib" \
    "$prefix/lib/gstreamer-1.0" \
    "$prefix/libexec/gstreamer-1.0/gst-plugin-scanner"; do
    if [[ ! -e "$required" ]]; then
        echo "Cerbero private prefix is missing required runtime path: $required" >&2
        exit 1
    fi
done

registry="$work_dir/registry.bin"
export LD_LIBRARY_PATH="$prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export GST_PLUGIN_SYSTEM_PATH="$prefix/lib/gstreamer-1.0"
export GST_PLUGIN_SYSTEM_PATH_1_0="$prefix/lib/gstreamer-1.0"
export GST_PLUGIN_PATH_1_0=""
export GST_PLUGIN_SCANNER="$prefix/libexec/gstreamer-1.0/gst-plugin-scanner"
export GST_PLUGIN_SCANNER_1_0="$GST_PLUGIN_SCANNER"
export GST_REGISTRY_1_0="$registry"
for element in \
    playbin uridecodebin appsink queue \
    audioconvert audioresample volume autoaudiosink \
    videoconvert videoscale qtdemux matroskademux oggdemux wavparse flacparse \
    avdec_h264 avdec_aac avdec_mp3 vp8dec vp9dec opusdec vorbisdec flacdec; do
    if ! "$prefix/bin/gst-inspect-1.0" --exists "$element"; then
        echo "Cerbero private prefix is missing required element: $element" >&2
        exit 1
    fi
done

mkdir -p "$output/share/http-client-runtime"
printf '%s\n' "$CERBERO_COMMIT" > "$output/share/http-client-runtime/source-revision.txt"
(
    cd "$output"
    find . -type f -print0 | sort -z | xargs -0 sha256sum > runtime-files.sha256
)
printf '%s\n' "$SUPPORTED_TARGET" > "$output/runtime-target.txt"
printf '%s\n' "$CERBERO_COMMIT" > "$output/cerbero-commit.txt"
