# Distribution Instructions

These notes describe how to prepare and publish OxiDisc release artifacts. Do not upload, push, submit, or publish anything unless that release action has been explicitly authorized.

---

## Preflight Checklist

1. **Version Alignment**: Confirm the release version and tag match across all package recipes and `Cargo.toml`.
   - Release version: `2.0.0`
   - Upstream Git tag: `v2.0.0`
2. **Clean Tree**: Confirm the working tree contains only intentional changes:
   ```bash
   git status --short --branch
   ```
3. **Automated Tests**: Ensure all automated unit and integration tests pass:
   ```bash
   cargo test --locked --features "egui-ui tui"
   ```
4. **Artifact Check**: Confirm built installable artifacts exist under `target/`:
   ```bash
   find target -type f \( -name '*.deb' -o -name '*.AppImage' -o -name '*.pkg.tar*' -o -name '*.rpm' -o -name '*.flatpak' -o -name 'oxidisc.exe' \) -print
   ```
5. **Target Verification**: Test install each artifact on clean target systems (e.g., Ubuntu LTS, Arch, Fedora, Windows 10/11) prior to public announcement.

---

## GitHub Release

GitHub Releases serves as the primary distribution hub for direct downloads and the upstream source archive for distro packaging.

1. **Create and Push the Git Tag**:
   ```bash
   git tag -a v2.0.0 -m "OxiDisc 2.0.0"
   git push origin v2.0.0
   ```
2. **Draft GitHub Release**: Create release targeting `v2.0.0`.
3. **Upload Release Artifacts**:
   - `target/appimage/oxidisc-2.0.0-x86_64.AppImage` (Linux Portable)
   - `target/debian/oxidisc_2.0.0-1_amd64.deb` (Debian/Ubuntu)
   - `target/release/oxidisc-2.0.0-windows-x86_64.zip` (Windows Portable containing `oxidisc.exe`)
4. **Release Notes**: Detail new features (Opus encoder, AcoustID Chromaprint recognition, GnuDB provider cascade, Windows native CDDA extraction, upgraded egui UI), known requirements, and SHA-256 checksums.

---

## AppImage

The standalone Linux AppImage is produced locally:

1. **Build**:
   ```bash
   packaging/appimage/build-appimage.sh
   ```
2. **Test**:
   ```bash
   chmod +x target/appimage/oxidisc-2.0.0-x86_64.AppImage
   ./target/appimage/oxidisc-2.0.0-x86_64.AppImage
   ```
3. **Publish**: Attach to the GitHub Release.

---

## Windows Portable / Installer

For Windows users:

1. **Build Release Binary**:
   ```powershell
   cargo build --release --locked --no-default-features --features "egui-ui"
   ```
2. **Bundle**: Create `target/release/oxidisc-2.0.0-windows-x86_64.zip` containing:
   - `oxidisc.exe`
   - `LICENSE`
   - `README.md`
3. **Publish**: Attach to the GitHub Release.

---

## Ubuntu PPA / Launchpad

Use Launchpad PPA for Ubuntu source packages.

1. Verify `packaging/debian/changelog` and `packaging/debian/control`.
2. Build source package on an Ubuntu build host:
   ```bash
   dpkg-buildpackage -S -sa -kYOUR_GPG_KEY_ID
   ```
3. Upload to PPA using `dput`:
   ```bash
   dput ppa:your-ppa/oxidisc ../oxidisc_2.0.0-1_source.changes
   ```
4. Monitor Launchpad build status across target series.

---

## Arch Linux (AUR)

1. Update `packaging/aur/PKGBUILD` with `pkgver=2.0.0`.
2. Regenerate `.SRCINFO`:
   ```bash
   cd packaging/aur
   makepkg --printsrcinfo > .SRCINFO
   ```
3. Test locally:
   ```bash
   makepkg -si
   ```
4. Commit and push `PKGBUILD` and `.SRCINFO` to the AUR Git remote.

---

## Fedora COPR

1. Verify `packaging/fedora/oxidisc.spec`.
2. Build source RPM:
   ```bash
   rpmbuild -bs packaging/fedora/oxidisc.spec
   ```
3. Submit SRPM to COPR via web interface or `copr-cli`.
4. Monitor builds and verify install on clean Fedora environments.

---

## Flatpak / Flathub

1. Update version and source hashes in `packaging/flatpak/io.github.dral.oxidisc.yml`.
2. Regenerate Cargo vendored sources if dependencies changed:
   ```bash
   flatpak-cargo-generator Cargo.lock -o packaging/flatpak/generated-sources.json
   ```
3. Test build locally:
   ```bash
   flatpak-builder --force-clean --repo=target/flatpak-repo target/flatpak-build packaging/flatpak/io.github.dral.oxidisc.yml
   ```
4. Bundle and test:
   ```bash
   flatpak build-bundle target/flatpak-repo target/oxidisc-2.0.0-x86_64.flatpak io.github.dral.oxidisc
   flatpak install --user target/oxidisc-2.0.0-x86_64.flatpak
   ```

---

## Checksums

Generate SHA-256 checksums for all release artifacts:

```bash
sha256sum target/appimage/*.AppImage target/debian/*.deb target/aur-build/*.pkg.tar* target/fedora/rpmbuild/RPMS/**/*.rpm target/*.flatpak target/release/*.zip
```
