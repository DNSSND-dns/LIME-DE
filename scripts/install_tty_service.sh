#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

if [[ "$(id -u)" != "0" ]]; then
  printf 'Run with sudo: sudo %s\n' "$0" >&2
  exit 1
fi

install -d /usr/local/bin
install -d /etc/lime-de
if [[ ! -f /etc/lime-de/native.env ]]; then
  tee /etc/lime-de/native.env >/dev/null <<'EOF'
# Native2 layers are opt-in. Keep these at 0 unless testing the next layer.
LIME_NATIVE2_ENABLE_DRM=0
EOF
fi
tee /usr/local/bin/lime-de-run-tty >/dev/null <<EOF
#!/usr/bin/env bash
printf 'wrapper start: %s tty=%s uid=%s pwd=%s\\n' "\$(date -Is)" "\$(tty || true)" "\$(id)" "\$(pwd)" >> /home/dns/.local/state/lime-de/systemd-run.log
exec "$PROJECT_ROOT/scripts/run_tty.sh" "\$@"
EOF
chmod 755 /usr/local/bin/lime-de-run-tty
install -Dm644 "$PROJECT_ROOT/packaging/lime-de-tty@.service" /etc/systemd/system/lime-de-tty@.service
systemctl daemon-reload

printf 'Installed lime-de-tty@.service.\n'
printf 'Build once as user, then start on tty5:\n'
printf '  cargo build --release --features native_tty\n'
printf '  sudo systemctl start lime-de-tty@tty5.service\n'
printf 'Logs:\n'
printf '  tail -120 /home/dns/.local/state/lime-de/tty.log\n'
printf 'Native2 layer config:\n'
printf '  /etc/lime-de/native.env\n'
