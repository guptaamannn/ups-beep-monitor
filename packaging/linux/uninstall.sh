#!/usr/bin/env bash
# Stops and removes the upsbeepd user service and binary. Leaves the config in place.
set -euo pipefail

systemctl --user disable --now upsbeepd.service 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/upsbeepd.service" "$HOME/.local/bin/upsbeepd"
systemctl --user daemon-reload
echo "removed"
