#!/usr/bin/env bash
# Deploys the landing page and judge demo to Vercel (D-53). It ships the apps/demo/dist that
# ci's e2e job built and tested on a green push to main; nothing is rebuilt here. The Vercel
# CLI uses its own login; this script never reads or prints a token.
#
#   scripts/deploy-vercel.sh [ci-run-id]    (default: the latest green ci run on main)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
repo="AmirmLotfy/dormbit"
project="dormbit"
link="$root/.scratch/vercel/project.json"
site="$root/.scratch/vercel-site"

run="${1:-$(gh run list --repo "$repo" --workflow ci --branch main --event push --status success \
  --limit 1 --json databaseId --jq '.[0].databaseId')}"
meta="$(gh run view "$run" --repo "$repo" --json conclusion,headBranch,event,headSha \
  --jq '"\(.conclusion) \(.headBranch) \(.event) \(.headSha)"')"
read -r conclusion branch event sha <<<"$meta"
if [ "$conclusion" != success ] || [ "$branch" != main ] || [ "$event" != push ]; then
  echo "deploy-vercel: run $run is not a green push to main ($meta)" >&2
  exit 1
fi

rm -rf "$site"
mkdir -p "$site"
gh run download "$run" --repo "$repo" --name demo-dist --dir "$site"
if ! grep -q "Preview build ${sha:0:7}" "$site/index.html"; then
  echo "deploy-vercel: the downloaded build doesn't carry commit ${sha:0:7}" >&2
  exit 1
fi
cp "$root/apps/demo/vercel.json" "$site/vercel.json"

cd "$site"
if [ -f "$link" ]; then
  mkdir -p .vercel
  cp "$link" .vercel/project.json
else
  vercel link --yes --project "$project"
  mkdir -p "$(dirname "$link")"
  cp .vercel/project.json "$link"
fi
url="$(vercel deploy --prod --yes)"
echo "deploy-vercel: deployed ${sha:0:7} from ci run $run to $url"
