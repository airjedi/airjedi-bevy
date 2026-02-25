# Phase 2: Distributable .app + DMG

## Goal

Produce a signed, notarized `.dmg` disk image that others can download and install without Gatekeeper warnings.

## Prerequisites

- Phase 1 complete (working .app bundle)
- Apple Developer account
- "Developer ID Application" certificate installed in Keychain
- "Developer ID Installer" certificate installed in Keychain (optional, for pkg)

## Code Signing

Sign with `codesign` using a Developer ID Application certificate. The signing identity is passed as a Makefile variable:

```
make dmg IDENTITY="Developer ID Application: Your Name (TEAMID)"
```

Signing steps:
1. Sign the binary: `codesign --force --sign "$IDENTITY" --options runtime Contents/MacOS/airjedi_bevy`
2. Sign the entire bundle: `codesign --force --sign "$IDENTITY" --options runtime --deep AirJedi.app`
3. Verify: `codesign --verify --deep --strict AirJedi.app`

The `--options runtime` flag enables hardened runtime, which is required for notarization.

## Notarization

Submit to Apple for notarization so the app runs cleanly on other machines:

1. Zip the signed app: `ditto -c -k --keepParent AirJedi.app AirJedi.zip`
2. Submit: `xcrun notarytool submit AirJedi.zip --apple-id "$APPLE_ID" --team-id "$TEAM_ID" --password "$APP_PASSWORD" --wait`
3. Staple the notarization ticket: `xcrun stapler staple AirJedi.app`
4. Verify: `spctl --assess --type exec AirJedi.app`

Credentials can be stored in Keychain to avoid passing them on the command line. The Makefile will support both inline credentials and a keychain profile.

## DMG Creation (build-dmg.sh)

1. Create a temporary staging directory.
2. Copy the signed+notarized `AirJedi.app` into it.
3. Create a symbolic link to `/Applications` for drag-and-drop install.
4. Create a read-write DMG: `hdiutil create -volname "AirJedi" -srcfolder "$STAGING" -format UDRW -ov tmp.dmg`
5. Convert to compressed read-only: `hdiutil convert tmp.dmg -format UDZO -o AirJedi.dmg`
6. Clean up temporary files.

Output: `macos/build/AirJedi.dmg`

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make sign` | Sign the .app bundle |
| `make notarize` | Submit for notarization and staple |
| `make dmg` | Full pipeline: build app, sign, notarize, create DMG |
| `make dmg-unsigned` | Create DMG without signing (for local testing) |

## Makefile Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `IDENTITY` | Code signing identity string | For signing |
| `APPLE_ID` | Apple ID email for notarization | For notarization |
| `TEAM_ID` | Apple Developer Team ID | For notarization |
| `APP_PASSWORD` | App-specific password or `@keychain:profile` | For notarization |
