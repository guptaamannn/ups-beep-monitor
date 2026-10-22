#!/usr/bin/env bash
# Builds target/deb/upsbeep_<version>_<arch>.deb (system-wide install under /usr).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n '/^\[workspace.package\]/,/^$/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")"
arch="$(dpkg --print-architecture)"
maintainer="$(git -C "$root" config user.name) <$(git -C "$root" config user.email)>"
out="$root/target/deb"
pkg="$out/upsbeep_${version}_${arch}"

cargo build --release --bins --manifest-path "$root/Cargo.toml"

rm -rf "$pkg"
mkdir -p "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/lib/systemd/user" \
  "$pkg/usr/share/applications" "$pkg/usr/share/doc/upsbeep"
bins=("$root/target/release/upsbeepd" "$root/target/release/upsbeepctl" "$root/target/release/upsbeep-ui")
install -m 755 "${bins[@]}" "$pkg/usr/bin/"
sed "s|@BIN@|/usr/bin|" "$root/packaging/linux/upsbeepd.service.in" > "$pkg/usr/lib/systemd/user/upsbeepd.service"
sed "s|@BIN@|/usr/bin|" "$root/packaging/linux/upsbeep.desktop.in" > "$pkg/usr/share/applications/upsbeep.desktop"
install -m 644 "$root/README.md" "$pkg/usr/share/doc/upsbeep/README.md"

# Let dpkg work out the shared-library dependencies of the binaries.
shl="$(mktemp -d)"
trap 'rm -rf "$shl"' EXIT
mkdir "$shl/debian"
printf 'Source: upsbeep\n\nPackage: upsbeep\nArchitecture: any\n' > "$shl/debian/control"
depends="$(cd "$shl" && dpkg-shlibdeps -O "${bins[@]}" 2>/dev/null | sed 's/^shlibs:Depends=//')"

cat > "$pkg/DEBIAN/control" <<CONTROL
Package: upsbeep
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: $maintainer
Depends: $depends
Recommends: libnotify-bin, libxkbcommon0, libwayland-client0, libvulkan1 | libgl1
Description: Shut the PC down safely by listening to a UPS beep
 Detects the beep of a UPS that has no data connection through a microphone
 and powers the PC off after a configurable time on battery.
 Includes the upsbeepd service, the upsbeepctl command, and a calibration window.
CONTROL

cat > "$pkg/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
  echo "upsbeep installed. As your own user, run:"
  echo "  upsbeep-ui                                  # calibrate the beep tone"
  echo "  systemctl --user enable --now upsbeepd      # start monitoring (dry run until you disable it)"
fi
POSTINST
chmod 755 "$pkg/DEBIAN/postinst"

find "$pkg" -type d -exec chmod 755 {} +
find "$pkg/usr/share" "$pkg/usr/lib" -type f -exec chmod 644 {} +
dpkg-deb --root-owner-group --build "$pkg" >/dev/null
echo "built $pkg.deb"
