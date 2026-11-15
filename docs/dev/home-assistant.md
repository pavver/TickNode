# Home Assistant add-on

Developer reference split out of [AGENTS.md](../../AGENTS.md).
Keep it current when the described code changes.

- `repository.yaml` (repo root) makes the repository addable in the HA add-on store; the add-on is `ha-addon/ticknode/` (`config.yaml`, `Dockerfile`, `run.sh`, `DOCS.md`, `README.md`, `CHANGELOG.md`). Ingress on port 8099 (HA strips the prefix and proxies WebSockets; the editor uses relative asset URLs and a relative `ws` URL, so no `X-Ingress-Path` handling), `homeassistant_api: true`, `init: false`, no exposed ports.
- HA builds repository add-ons with the add-on directory as the build context, so the Dockerfile's `source` stage clones `TICKNODE_REF` (default `master`) from GitHub. Locally/CI use the workspace: `docker build --build-context source=. -f ha-addon/ticknode/Dockerfile ha-addon/ticknode`. Trunk is pinned by `TRUNK_VERSION` and downloads the wasm-bindgen matching `Cargo.lock`.
- `image:` is deliberately omitted from `config.yaml` (commented) and `.github/workflows/addon.yml` has `push: false`: nothing is published to ghcr.io until release. Keep `version` in `config.yaml` in step with the workspace version.
- Bundled plugins live in the image at `/opt/ticknode/plugins` and are passed through `TICKNODE_BUNDLED_PLUGINS_DIR` (a generic server feature: scanned in addition to `<data>/plugins`, the user's same-named file wins; their config/data stay under the data dir). `run.sh` passes `SUPERVISOR_TOKEN` as `TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN`.
