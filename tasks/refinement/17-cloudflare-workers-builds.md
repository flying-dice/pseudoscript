# T17 — Cloudflare Workers Builds for web-ide + web-landing (#ops)

## Summary (1-2 lines)
Deploy both front-ends via **Cloudflare Workers Builds** (CF-native Git CI): production deploy on push to `main`, per-branch/PR **preview URLs** for MRs. No GitHub Actions, no API-token secret — Builds uses the connected account's auth. The vendored WASM (`web-ide/src/lib/pds-wasm/`, committed on purpose) means the build needs **no Rust toolchain** — just `npm ci && npm run build`.

## Current state
- Two Workers existed (`pdscript-ide`, `pdscript-landing`) from manual `wrangler deploy`; **both deleted** so Builds owns them from scratch. Custom domains (`pdscript.dev`, `ide.pdscript.dev`) detach with the Worker and re-attach on the first Builds deploy (the `routes` are pinned in each `wrangler.jsonc`).
- `web-landing/wrangler.jsonc` — static-assets Worker, name `pdscript-landing`, `custom_domain pdscript.dev`.
- `web-ide/wrangler.jsonc` — adapter-cloudflare Worker, name `pdscript-ide`, `custom_domain ide.pdscript.dev`. `npm run build` runs a `postbuild` that writes `.svelte-kit/cloudflare/.assetsignore` (`_worker.js`, `_routes.json`).
- The old GitHub Actions `deploy.yml` and the `CLOUDFLARE_*` repo secrets were **removed** — superseded by this.

## Prerequisites
1. Commit the web-landing app + both `wrangler.jsonc` files to `main` (Builds reads `wrangler.jsonc` from the root directory at build time).
2. A **workers.dev subdomain** must be claimed on the account (Workers & Pages → one-time subdomain prompt). Preview URLs hang off it.

## Setup — do this twice, once per Worker
Dashboard → **Workers & Pages → Create → Workers → Connect to Git** → pick `flying-dice/pseudoscript`. The Worker name is taken from `wrangler.jsonc` `name`, so it must match exactly.

### Worker A — `pdscript-ide`
| Field | Value |
|---|---|
| Git repository | `flying-dice/pseudoscript` |
| Production branch | `main` |
| Root directory | `web-ide` |
| Build command | `npm ci && npm run build` |
| Deploy command | `npx wrangler deploy` |
| Non-production branch deploy command | `npx wrangler versions upload` |
| Build watch paths (path filtering) | `web-ide/*` |
| Build variable | `NODE_VERSION = 22` |

### Worker B — `pdscript-landing`
| Field | Value |
|---|---|
| Git repository | `flying-dice/pseudoscript` |
| Production branch | `main` |
| Root directory | `web-landing` |
| Build command | `npm ci && npm run build` |
| Deploy command | `npx wrangler deploy` |
| Non-production branch deploy command | `npx wrangler versions upload` |
| Build watch paths (path filtering) | `web-landing/*` |
| Build variable | `NODE_VERSION = 22` |

## How previews work
- Push to `main` → Build runs the **deploy command** (`wrangler deploy`) → promotes to the active deployment on the custom domain.
- Push to any other branch / open a PR → Build runs the **non-production deploy command** (`wrangler versions upload`) → uploads a version and emits a `https://<version-prefix>-<worker>.<subdomain>.workers.dev` **preview URL** (no promotion, custom domain untouched). The Build logs print the URL; the GitHub check/deployment surfaces it on the PR.
- Path filtering means a commit touching only `web-ide/**` builds only `pdscript-ide`, and vice-versa.

## Acceptance criteria
- Push to `main` deploys both Workers; `https://pdscript.dev/` and `https://ide.pdscript.dev/` return 200.
- A PR touching `web-ide/**` produces a working preview URL and does **not** change production.
- No GitHub Actions deploy workflow and no `CLOUDFLARE_*` secrets remain.

## Notes
- Worker-name ↔ `wrangler.jsonc` `name` mismatch is the most common failure — Builds errors out if they differ.
- `npx wrangler` in the deploy commands is fine (Builds provides wrangler); pin by adding `wrangler` to each `devDependencies` if a reproducible version is wanted.
- Rollback to GitHub Actions: the deleted `deploy.yml` is recoverable from git history if Builds is ever abandoned.
