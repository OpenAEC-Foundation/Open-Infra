#!/usr/bin/env bash
# Build the plugin and install it into Open CAD Studio's user plugins folder.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

case "$(uname -s)" in
  Darwin) lib=target/release/libopen_infra.dylib; cfg="$HOME/Library/Application Support" ;;
  MINGW*|MSYS*|CYGWIN*) lib=target/release/open_infra.dll; cfg="$APPDATA" ;;
  *) lib=target/release/libopen_infra.so; cfg="${XDG_CONFIG_HOME:-$HOME/.config}" ;;
esac

rustc_version="$(rustc --version)"
acadrust_source="$(awk '/^name = "acadrust"$/{f=1} f&&/^source = /{gsub(/^source = "|"$/,""); print; exit}' Cargo.lock)"

dest="$cfg/OpenCADStudio/plugins/openaec.open-infra"
mkdir -p "$dest"
rm -f "$dest"/*.so "$dest"/*.dll "$dest"/*.dylib
cp "$lib" "$dest/"
sed -e "s|__RUSTC_VERSION__|$rustc_version|" -e "s|__ACADRUST_SOURCE__|$acadrust_source|" plugin.toml > "$dest/plugin.toml"
echo "Installed in $dest"
grep -E "rustc_version|acadrust_source" "$dest/plugin.toml"
