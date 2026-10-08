#!/bin/sh
# Checksum-pinned official native SDK. Run inside the Linux build image.
set -eu
sdk_destination=${1:?Usage: install-duckdb-sdk.sh DESTINATION}
case $(uname -m) in
  x86_64) sdk_arch=amd64; sdk_sha=b845005f5132a7d8180057c35e14a7626632258782f871a90861b19c1c03841b ;;
  aarch64|arm64) sdk_arch=arm64; sdk_sha=b72ed9f05003f5e9d2015f7ceada6416b377d9dd33169cdde3c9e33897856eee ;;
  *) echo 'Unsupported DuckDB SDK architecture' >&2; exit 1 ;;
esac
sdk_temporary=$(mktemp -d)
trap 'rm -rf "$sdk_temporary"' EXIT HUP INT TERM
curl --retry 3 -fsSL "https://github.com/duckdb/duckdb/releases/download/v1.5.6/libduckdb-linux-$sdk_arch.zip" -o "$sdk_temporary/sdk.zip"
printf '%s  %s\n' "$sdk_sha" "$sdk_temporary/sdk.zip" | sha256sum --check
mkdir -p "$sdk_destination"
unzip -q "$sdk_temporary/sdk.zip" -d "$sdk_destination"
