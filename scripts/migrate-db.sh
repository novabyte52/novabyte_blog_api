#!/usr/bin/env bash
# Applies every rollout manifest under database/rollouts/ that prod hasn't
# completed yet, in filename (timestamp) order. Run from the repo/deploy root
# — expects database/ and a SurrealKit .env.local (SURREALDB_* vars, prod
# creds) to already be present alongside it. Invoked by the Jenkinsfile
# "Migrate DB" stage, before the app image is deployed.
#
# UNVERIFIED against the real prod droplet — written and reasoned about, not
# run there. Confirm `surrealkit` is on PATH on the droplet (or swap the
# invocation for a container on the same Docker network as `surrealdb` if
# the droplet host itself can't resolve that hostname) before trusting this
# in a real deploy.
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
