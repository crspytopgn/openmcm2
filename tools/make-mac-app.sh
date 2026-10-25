#!/bin/sh
# Builds OpenMCM2.app: the launcher with mm2-viewer beside it, so the
# game can be started from Finder. Run from the engine folder on a Mac:
#   sh tools/make-mac-app.sh        (puts OpenMCM2.app in target/release)
set -e
cargo build --release -p mm2-viewer -p mm2-launcher
app=target/release/OpenMCM2.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/mm2-launcher target/release/mm2-viewer "$app/Contents/MacOS/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>OpenMCM2</string>
  <key>CFBundleDisplayName</key><string>OpenMCM2</string>
  <key>CFBundleIdentifier</key><string>io.github.crspytopgn.openmcm2</string>
  <key>CFBundleExecutable</key><string>mm2-launcher</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# Ad-hoc signed, so Apple silicon runs it.
codesign --force --deep --sign - "$app" 2>/dev/null || true
echo "built $app"
