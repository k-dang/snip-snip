#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) snip-snip.app and zips it into
# target/dist. The app is ad-hoc signed, not notarized.
set -euo pipefail

if [[ "$(uname)" != Darwin ]]; then
    echo 'Build the macOS app on macOS with Xcode installed.' >&2
    exit 1
fi

cd "$(dirname "$0")/.."
targets=(aarch64-apple-darwin x86_64-apple-darwin)
rustup target add "${targets[@]}"
binaries=()
for target in "${targets[@]}"; do
    cargo build --release --locked --target "$target" -p snip-gpui
    binaries+=("target/$target/release/snip-snip")
done

# The ID ends in "#0.1.2" or "#snip-gpui@0.1.2", depending on the directory name.
pkgid=$(cargo pkgid -p snip-gpui)
version=${pkgid##*[#@]}
stage=target/dist/macos
app=$stage/snip-snip.app
rm -rf "$stage"
mkdir -p "$app/Contents/MacOS"
lipo -create -output "$app/Contents/MacOS/snip-snip" "${binaries[@]}"
cp crates/snip-core/fonts/Inter-LICENSE.txt "$stage/"

# LSUIElement keeps the menu-bar app out of the Dock and app switcher.
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>snip-snip</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.k-dang.snip-snip</string>
    <key>CFBundleName</key>
    <string>snip-snip</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# Apple Silicon refuses to run unsigned code; an ad-hoc signature also seals
# Info.plist into the bundle.
codesign --force --sign - "$app"
codesign --verify --strict "$app"

zip=target/dist/snip-snip-$version-macos-universal.zip
rm -f "$zip"
ditto -c -k --sequesterRsrc "$stage" "$zip"
ls -l "$zip"
shasum -a 256 "$zip"
