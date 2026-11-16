# Home Assistant add-on

Developer reference split out of [AGENTS.md](../../AGENTS.md).
Keep it current when the described code changes.

- `repository.yaml` (repo root) makes the repository addable in the HA add-on store (the README has a my.home-assistant.io button for it); the add-on is `ha-addon/ticknode/` (`config.yaml`, `Dockerfile`, `run.sh`, `DOCS.md`, `README.md`, `CHANGELOG.md`). Ingress on port 8099 (HA strips the prefix and proxies WebSockets; the editor uses relative asset URLs and a relative `ws` URL, so no `X-Ingress-Path` handling), `homeassistant_api: true`, `init: false`, no exposed ports.
- Without `image:` HA would build the add-on itself with the add-on directory as the build context, so the Dockerfile's `source` stage clones `TICKNODE_REF` (default `master`) from GitHub. Locally/CI use the workspace: `docker build --build-context source=. -f ha-addon/ticknode/Dockerfile ha-addon/ticknode`. Trunk is pinned by `TRUNK_VERSION` and downloads the wasm-bindgen matching `Cargo.lock`.
- Releases: `scripts/release.sh X.Y.Z` (after the ub-mode pass in *Releases* of AGENTS.md) bumps every crate, `version` in `config.yaml` and the changelog, commits and tags `vX.Y.Z`; pushing the tag makes `.github/workflows/addon.yml` build `amd64` and `aarch64` natively and push `ghcr.io/pavver/ticknode-{arch}:X.Y.Z` (+ `latest`), which `image:` in `config.yaml` points to. Home Assistant offers the update when `version` grows. Pull requests and manual runs only build. The ghcr packages must be public.
- Bundled plugins live in the image at `/opt/ticknode/plugins` and are passed through `TICKNODE_BUNDLED_PLUGINS_DIR` (a generic server feature: scanned in addition to `<data>/plugins`, the user's same-named file wins; their config/data stay under the data dir). `run.sh` passes `SUPERVISOR_TOKEN` as `TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN`.

## Plugin state tracking

The integration stays in `ha-plugin`; the core and server do not subscribe to Home Assistant states.
`ha-plugin/src/hub/tracking.rs` owns one shared `Source::entity(id)` subscription per tracked entity, including attribute changes, with a `get_state` baseline after subscription and reconnect.
Condition properties track their entity dependencies (person and zone, or `sun.sun` for solar boundaries); action properties track every static target ID.
`ha/entity` nodes share those subscriptions and baselines while retaining independent ordered output queues, value types and unit warnings.
Property changes replace the node's dependencies, and dropping the node releases its ownership.
The last owner's departure starts a 60-second grace period before `Session::unsubscribe`; another owner arriving during that period reuses the subscription and loaded state.
Runtime action targets and lazy `Live::state` accesses retain a lazy owner for ten minutes after the last access, followed by the same grace period.
Disconnected or awaiting-baseline entities return an English not-loaded error to the condition's warning log, and actions always call HA rather than skip against an unloaded state.
Cache revisions still protect successive actions while their effects are pending; a call made without a loaded baseline conservatively waits for the command-settle timeout before skipping can resume.
The UTC offset template stream and HA configuration reads remain independent of entity tracking.

`ha-plugin/src/entities/catalog.rs` keeps only picker metadata, a derived icon and an inferred value type; it retains neither state values nor arbitrary attributes.
The plugin reads `get_states` once per connection and subscribes once to `entity_registry_updated`, debouncing registry events for ten seconds before another catalog read.
Failed catalog reads retry after thirty seconds; there is no periodic full-state polling or global `state_changed` subscription.
Action option builders and unit inference use tracked metadata when loaded, falling back to the slim catalog.
