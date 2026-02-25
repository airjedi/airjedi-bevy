# Phase 3: Mac App Store Preparation

## Goal

Prepare the app for Mac App Store submission with sandboxing, entitlements, and the App Store signing/upload pipeline.

## Prerequisites

- Phase 1 and 2 complete
- Apple Developer Program membership
- "3rd Party Mac Developer Application" certificate in Keychain
- "3rd Party Mac Developer Installer" certificate in Keychain

## Entitlements

File: `macos/entitlements.plist`

Required entitlements:
- `com.apple.security.app-sandbox`: true (mandatory for App Store)
- `com.apple.security.network.client`: true (ADS-B data feeds, tile downloads, reqwest HTTP)
- `com.apple.security.files.user-selected.read-write`: true (config export, file save dialogs)

Potential future entitlements:
- `com.apple.security.device.usb`: for direct SDR/ADS-B receiver hardware access

## Sandboxing Code Changes

App Store apps run in a sandbox container at `~/Library/Containers/com.airjedi.app/`. This affects file paths:

1. **Tile cache**: Move from `assets/tiles/` to the sandbox container's `Library/Caches/` directory. Use `dirs::cache_dir()` (already a dependency) to resolve.
2. **Config file**: Move `config.toml` read/write to the sandbox container's `Library/Application Support/` directory. Use `dirs::config_dir()`.
3. **Read-only assets**: Fonts, models, and bundled images stay in `Contents/Resources/` inside the app bundle (read-only access is allowed).
4. **Runtime detection**: Check if running sandboxed (environment variable `APP_SANDBOX_CONTAINER_ID` is set) and route file paths accordingly. Non-sandboxed builds (development, direct distribution) continue using current paths.

## App Store Signing

Different from Phase 2's Developer ID signing:

1. Sign the binary with entitlements:
   ```
   codesign --force --sign "3rd Party Mac Developer Application: Name (TEAMID)" \
     --entitlements entitlements.plist --options runtime \
     AirJedi.app/Contents/MacOS/airjedi_bevy
   ```
2. Sign the bundle:
   ```
   codesign --force --sign "3rd Party Mac Developer Application: Name (TEAMID)" \
     --entitlements entitlements.plist --options runtime --deep \
     AirJedi.app
   ```

## Package and Submit

Build a `.pkg` installer (required for App Store, not DMG):

1. Build the signed .app bundle.
2. Create installer package:
   ```
   productbuild --sign "3rd Party Mac Developer Installer: Name (TEAMID)" \
     --component AirJedi.app /Applications AirJedi.pkg
   ```
3. Validate:
   ```
   xcrun altool --validate-app -f AirJedi.pkg -t macos \
     -u "$APPLE_ID" -p "$APP_PASSWORD"
   ```
4. Upload:
   ```
   xcrun altool --upload-app -f AirJedi.pkg -t macos \
     -u "$APPLE_ID" -p "$APP_PASSWORD"
   ```

Alternatively, use Transporter.app for manual upload.

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make appstore` | Full pipeline: build, sign with App Store certs, package as .pkg |
| `make validate` | Validate .pkg with Apple |
| `make upload` | Upload .pkg to App Store Connect |

## Makefile Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `APPSTORE_IDENTITY` | 3rd Party Mac Developer Application identity | Yes |
| `INSTALLER_IDENTITY` | 3rd Party Mac Developer Installer identity | Yes |
| `APPLE_ID` | Apple ID email | Yes |
| `APP_PASSWORD` | App-specific password | Yes |

## Out of Scope

- App Store Connect metadata (screenshots, descriptions, categories, pricing) — managed directly in the App Store Connect web UI
- App review guidelines compliance audit — separate effort
- TestFlight beta distribution — can reuse the same .pkg upload
