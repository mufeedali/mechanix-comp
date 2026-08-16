#!/bin/bash
# Cross-build (optional), copy, and hot-reload the compositor on the test device.
# Usage:
#   ./scripts/push-compositor.sh          # push the existing cross binary + restart
#   ./scripts/push-compositor.sh build    # cross-build first, then push + restart
set -euo pipefail

DEV="mecha@172.16.42.1"
SSH_OPTS=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10)
PW="$(cut -d= -f2 < /home/mufeed/Projects/mecha/mecha-make/mkosi.env)"
BIN="target/aarch64-unknown-linux-gnu/release/compositor"

if [ "${1:-}" = "build" ]; then
    export PATH="$HOME/.local/share/cargo/bin:$PATH"
    export CROSS_CONTAINER_ENGINE=podman
    cross build --release --target aarch64-unknown-linux-gnu
fi

sshpass -p "$PW" scp "${SSH_OPTS[@]}" "$BIN" "$DEV:/tmp/compositor"
sshpass -p "$PW" ssh "${SSH_OPTS[@]}" "$DEV" \
    "printf '%s\n' '$PW' | sudo -S sh -c 'install -m 0755 /tmp/compositor /usr/local/bin/compositor && rm -f /tmp/compositor'"

# The session script runs the compositor in a loop, so killing it picks up the
# new binary without a reboot.
sshpass -p "$PW" ssh "${SSH_OPTS[@]}" "$DEV" "pkill -x compositor 2>/dev/null || true"

echo "pushed + compositor restarted (session loop relaunches it)"
