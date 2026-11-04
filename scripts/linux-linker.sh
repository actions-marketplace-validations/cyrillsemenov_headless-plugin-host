#!/bin/sh
if [ "$(uname -s)" = "Linux" ]; then
  exec cc "$@"
fi
sync
exec docker exec -w "$PWD" hph-linux-box sh -c '
  OUT=""
  PREV=""
  for arg in "$@"; do
    if [ "$PREV" = "-o" ]; then
      OUT="$arg"
      break
    fi
    PREV="$arg"
  done
  if [ -n "$OUT" ]; then
    TMP_OUT="/tmp/hph_link_$$.out"
    set -- "$@"
    i=1
    n=$#
    while [ $i -le $n ]; do
      arg="$1"
      shift
      if [ "$arg" = "-o" ]; then
        set -- "$@" "-o" "$TMP_OUT"
        shift
        i=$((i + 2))
      else
        set -- "$@" "$arg"
        i=$((i + 1))
      fi
    done
    gcc "$@" && rm -f "$OUT" && cp "$TMP_OUT" "$OUT" && rm -f "$TMP_OUT"
  else
    gcc "$@"
  fi
' sh "$@"
