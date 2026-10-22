#!/usr/bin/env bash
# Builds upsbeepd, installs it for the current user, and starts it as a systemd user service.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
unit_dir="$HOME/.config/systemd/user"
app_dir="$HOME/.local/share/applications"
bin_dir="$HOME/.local/bin"

cargo build --release --bins --manifest-path "$root/Cargo.toml"
mkdir -p "$bin_dir" "$unit_dir" "$app_dir"
install -m 755 "$root/target/release/upsbeepd" "$root/target/release/upsbeepctl" "$root/target/release/upsbeep-ui" "$bin_dir/"
install -m 644 "$root/packaging/linux/upsbeepd.service" "$unit_dir/upsbeepd.service"
sed "s|@BIN@|$bin_dir|" "$root/packaging/linux/upsbeep.desktop.in" > "$app_dir/upsbeep.desktop"

# Create a default config (dry_run = true) only if none exists.
"$bin_dir/upsbeepd" --list-devices >/dev/null 2>&1 || true
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/upsbeep/config.toml"
[ -f "$cfg" ] || "$bin_dir/upsbeepd" --write-default-config

systemctl --user daemon-reload
systemctl --user enable upsbeepd.service
systemctl --user restart upsbeepd.service
echo "installed. logs: journalctl --user -u upsbeepd -f"
echo "config: $cfg (dry_run is true until you change it, then: systemctl --user restart upsbeepd)"
