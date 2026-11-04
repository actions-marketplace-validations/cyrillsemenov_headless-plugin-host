#!/bin/sh
if [ "$(uname -s)" = "Linux" ]; then
  if [ -z "$DISPLAY" ] && command -v xvfb-run >/dev/null 2>&1; then
    exec xvfb-run -a "$@"
  fi
  exec "$@"
fi
exec docker exec -w "$PWD" hph-linux-box xvfb-run -a "$@"
