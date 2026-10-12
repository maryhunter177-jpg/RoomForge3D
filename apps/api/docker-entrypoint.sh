#!/bin/sh
set -eu
if [ -n "${STORAGE_ENDPOINT:-}" ]; then
  node /app/tools/setup-storage.mjs
fi
exec "$@"
