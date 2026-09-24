#!/usr/bin/env bash
# Applies every rollout manifest under database/rollouts/ that prod hasn't
# completed yet, in filename (timestamp) order. Run from the repo root on
# the Jenkins agent itself — connects straight to prod over
# wss://db.novabyte.blog (SURREALDB_* vars from .env.local, see the
# "Migrate DB" Jenkinsfile stage), no SSH involved. Requires `surrealkit` on
# PATH (see Dockerfile.agent).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

status="$(surrealkit rollout status)"

for manifest in database/rollouts/*.toml; do
    [ -e "$manifest" ] || continue
    id="$(basename "$manifest" .toml)"

    if grep -qF "${id} [completed]" <<<"$status"; then
        echo "skip ${id}: already completed"
        continue
    fi

    echo "applying ${id}..."
    surrealkit rollout start "$id"
    surrealkit rollout complete "$id"
done
