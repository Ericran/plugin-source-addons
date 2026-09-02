# GameAP Source Addons plugin

A [GameAP](https://gameap.com) panel plugin that adds a **Plugins** tab to
**Source engine (Source 1)** servers — Counter-Strike: Source, Garry's Mod,
Team Fortress 2, Left 4 Dead / 2, Day of Defeat: Source, Half-Life 2:
Deathmatch, Black Mesa, Synergy and legacy CS:GO — for managing
**Metamod:Source** and **SourceMod**.

List plugins with live statuses, enable/disable them persistently, hot
load/unload/reload without a restart, edit per-plugin comments, group plugins,
upload new ones, and edit their configs. Plus:

- **one-click installs** of Metamod:Source and SourceMod (the current
  AlliedModders build, downloaded and unpacked on the node) with an update
  button when a newer build ships;
- a **loader repair**: writes `addons/metamod.vdf` naming the Metamod binary
  actually on disk, and re-checks every Source server on a 6-hour schedule —
  a Steam validate or a hand-edited `gameinfo.txt` otherwise leaves Metamod
  sitting on disk with nothing starting it;
- **both Metamod plugin mechanisms** in one list: the `addons/metamod/*.vdf`
  aliases *and* the `metaplugins.ini` lines, each with the same on/off switch;
- a **catalog** of well-known projects installable straight from their GitHub
  releases, with **update badges** on installed rows (versions checked nightly
  and cached);
- **archive installs** of `.zip` **and `.tar.gz`** — the layout
  (`addons/`-rooted, `plugins/`-rooted, loose `.smx`) is detected and unpacked
  to the right place;
- **automatic safety snapshots** before every platform install, catalog
  install, archive install and restore — every overwrite is reversible;
- a **SourceMod admins editor** for `admins_simple.ini` that preserves the
  file's comments, plus a raw editor for `admins.cfg`;
- the **SourceMod logs** with filtering and following, switchable between the
  error log and the general log;
- **snapshots** of the whole SourceMod setup (tarballs on the server, newest 5
  kept) with one-click restore — also the transfer format for copying a setup
  between servers;
- an **audit history** of panel actions and a **restart banner** whenever
  changes are waiting for one;
- a **Doctor** dialog running every health check in one pass.

## Credits

This is a Source 1 port of
**[plugin-cs2-addons](https://github.com/booskibro/plugin-cs2-addons)**, which
is itself based on **[gameap/plugin-goldsrc-addons](https://github.com/gameap/plugin-goldsrc-addons)**
by the GameAP project — the equivalent panel plugin for GoldSource (CS 1.6)
servers managing Metamod and AMX Mod X. The architecture (Rust/WASM backend on
the gameap-plugin-sdk + embedded Vue 3 frontend), the route design, the UI and
most of the code structure come from them. All credit for the concept and
design goes to the GameAP authors.

## Quick start

1. Get `source-addons.wasm` — download the `source-addons-wasm` artifact from
   the latest [Actions run](../../actions), or build it yourself (see
   [Build](#build)).
2. In your GameAP panel: **Administration → Plugins → upload** the `.wasm`
   (or copy it into the panel's plugins directory by hand).
3. Restart the panel.
4. Open any Source 1 server — a **Plugins** tab appears next to the server's
   other tabs (admins only).

## Requirements

**Panel:** a GameAP installation recent enough to support WASM panel plugins
AND the `gameap-scheduler` host module (current
[gameap/gameap](https://github.com/gameap/gameap) main). Plugin HTTP must allow
`https` — release lookups query `api.github.com`, `sm.alliedmods.net` and
`mms.alliedmods.net`.

**Node:** platform installs and snapshots run `curl`/`wget` and `tar` on a
**linux** node. Both AlliedModders drops are plain `.tar.gz`, so no `unzip` is
needed. A stock Debian/Ubuntu box has everything.

**Game server** (for full functionality; the tab degrades gracefully without):

- A Source 1 server created in GameAP with one of the stock games (see
  [Which games](#which-games)).
- [Metamod:Source 1.12](https://www.sourcemm.net/downloads.php) in
  `<mod>/addons/metamod`, loaded by `<mod>/addons/metamod.vdf`.
- [SourceMod 1.12](https://www.sourcemod.net/downloads.php) in
  `<mod>/addons/sourcemod`.
- An **RCON password** configured on the server in GameAP. Without it the tab
  still works — you just lose live data (versions, Running/Paused states, hot
  load/unload); everything file-based keeps working and a hint line explains
  what's unavailable.

Unlike its CS2 sibling this plugin registers **no RCON protocol**: Source 1
speaks the classic Source RCON that the panel's built-in client already
implements correctly.

## Which games

The tab is shown for these stock GameAP game codes:

`cssource`, `cssv34`, `csgo`, `dods`, `garrysmod`, `hl2mp`, `l4d`, `l4d2`,
`tf2`, `bms`, `synergy`

They are listed explicitly rather than gated on the engine name, because the
panel's tab gate is *engines OR codes* with no version component and the engine
string alone is too coarse in both directions: `cs2` is also engine "Source"
(Source 2 — that is the [CS2 Addons](https://github.com/booskibro/plugin-cs2-addons)
plugin's job) and `ark` is mislabeled "Source" in the stock game list while
being Unreal Engine.

To use this plugin with a **custom** Source 1 game entry, add its code to
`SOURCE_GAME_CODES` in `frontend/src/index.ts` and `src/maintenance.rs` and
rebuild — a test fails if the two lists drift. The backend gate is
version-based (engine `source`, version not starting with `2`) and already
accepts any such entry.

The mod directory is found from the launch command's `-game <dir>` first, then
from a table of the stock games' mod dirs, then by scanning for a
`gameinfo.txt` — skipping the base-content directories (`hl2/`, `platform/`, …)
that every Source install ships with a `gameinfo.txt` of their own.

## What the tab shows

- **Platform cards** — Metamod:Source (installed / **not active** when
  `addons/metamod` exists but nothing loads it, version via RCON `meta version`,
  plus which loader method is in force) and SourceMod (installed / **not
  active** when unpacked but not registered with Metamod, version via
  `meta list` or `sm version`).
- **Metamod plugin list** on the Metamod card, merging the `.vdf` aliases and
  the `metaplugins.ini` lines. Each row says which file registers it, and
  SourceMod's own entry carries a *platform* badge.
- **Plugin table** — every `.smx` found on disk
  (`addons/sourcemod/plugins/*`), merged with live runtime state from RCON
  `sm plugins list`:

  | Status | Meaning |
  |---|---|
  | Running | on disk, loaded and running |
  | Paused | loaded but paused (`sm plugins pause`) — resumable in one click |
  | Enabled / Stopped | file state, when the console is unreachable |
  | Awaiting load | on disk but not loaded (or disabled but still in memory) |
  | Error | SourceMod reports the plugin failed |
  | File missing | tracked in `plugins_meta.json` but the `.smx` is gone |

- Plugins are **grouped** by their `Group` from `plugins_meta.json`, with
  inline-editable per-plugin **comments**.

### How rows are matched to the console

`sm plugins list` prints a loaded plugin by its **title** — free text chosen by
the author, with no mechanical relationship to its file name ("Admin File
Reader" is `admin-flatfile.smx`). A *failed* plugin is printed by its file
instead, because there is no loaded plugin to ask for a title.

So rows are paired in three passes: the file the console named outright, then a
table of SourceMod's ~25 bundled plugin titles, then an exact normalized
title↔file match. Whatever is left is asked about directly with
`sm plugins info <#>`, which names the file — one round trip per unplaced
plugin, cached for the session. Nothing is guessed: a wrong pairing would show
one plugin's version and status on another plugin's row.

## What the tab does

| Action | How it works |
|---|---|
| Enable / disable (persistent) | Moves the `.smx` between `plugins/` and `plugins/disabled/` — SourceMod's own convention, since it never descends into a subdirectory |
| Load / Unload / Reload / Unpause (hot) | `sm plugins load / unload / reload / unpause` — no restart needed |
| Install | Upload a `.smx`, or a whole release `.zip` / `.tar.gz` — the layout is detected and unpacked to the right place |
| Delete | Removes the `.smx`; configs, translations and gamedata are kept |
| Comments / groups | Stored in `addons/sourcemod/configs/gameap/plugins_meta.json` |
| Config editing | Modal editor for `cfg/sourcemod/<name>.cfg`, or `addons/sourcemod/configs/<name>.cfg` |
| Admins | Structured editor for `admins_simple.ini`, raw editor for `admins.cfg` |

**The two switches mean different things:**

- The **On/Off switch** on a row is *persistent*: it moves the `.smx` in or out
  of `plugins/disabled/`. It does not touch the running server — after
  switching, either use Load/Unload for an immediate effect or let the next
  restart apply it.
- The **Load / Unload button** is *immediate but temporary*: it runs
  `sm plugins load/unload` over RCON. An unloaded plugin comes back at the next
  restart unless you also switch it off.

## The Metamod loader

Source 1 supports two ways of starting Metamod, and this plugin understands
both:

1. **`addons/metamod.vdf`** — a Valve server-plugin manifest naming Metamod's
   loader binary. This is what the official installer generates, it is the only
   file involved, and a game update does not touch it. **Repairs write this
   one**, choosing `bin/server` or `bin/linux64/server` to match the binary
   actually installed.
2. **The `gameinfo.txt` search path** — a `Game |gameinfo_path|addons/metamod`
   line in the SearchPaths block. Detected so an existing install is never
   reported broken, and patchable on request (`{"gameinfo": true}` on the
   repair route).

Either one is enough. The scheduled sweep deliberately leaves a gameinfo-only
install alone rather than adding a vdf on top of it.

## HTTP routes (backend)

All admin-only, under `/api/plugins/onzggylemrxw4`:

```
GET    /servers/{id}/state               assembled Metamod/SourceMod state
POST   /servers/{id}/plugins/toggle      {name, enabled}   file move
POST   /servers/{id}/plugins/attributes  {name, comment, group}
POST   /servers/{id}/plugins             {name, force?}    register upload
DELETE /servers/{id}/plugins             {name} or ?name=  delete
POST   /servers/{id}/metamod/repair      {gameinfo?}       write the loader
POST   /servers/{id}/metamod/toggle      {name, enabled, source, line?, force?}
GET    /servers/{id}/logs                tail (?kind=errors|general)
POST   /servers/{id}/restart             restart via servercontrol
GET    /servers/{id}/updates             latest upstream versions (?refresh=1)
GET    /servers/{id}/catalog             curated catalog for this game
POST   /servers/{id}/catalog/install     {key}             install from GitHub
POST   /servers/{id}/platform/install    {kind: metamod|sourcemod}
POST   /servers/{id}/snapshots           create snapshot
GET    /servers/{id}/snapshots           list snapshots
POST   /servers/{id}/snapshots/restore   {name}
DELETE /servers/{id}/snapshots           {name}
GET    /servers/{id}/audit               recent panel actions
POST   /servers/{id}/plugins/install-archive  {path, force?}
                                                  (.zip or .tar.gz, 32 MiB max, and no
                                                   single file inside it over that)
GET    /servers/{id}/doctor              server-side health checks
```

Beyond HTTP the plugin exports one optional panel service, a
**ScheduledTaskHandler** with two tasks: `srcaddons-metamod-loader-sweep` (6h)
and `srcaddons-update-check` (24h).

## Snapshots

A snapshot tars these, relative to the mod directory, into
`addons/sourcemod/backups/`:

```
addons/sourcemod/plugins       addons/sourcemod/gamedata
addons/sourcemod/configs       addons/sourcemod/translations
addons/sourcemod/data          addons/sourcemod/extensions
cfg/sourcemod
```

`cfg/sourcemod` is why the tar runs from the mod directory rather than from
`addons/sourcemod`: SourceMod's generated per-plugin configs live outside
`addons/` entirely.

`gamedata` and `extensions` are included because a plugin's release ships them
alongside its `.smx` and it will not run without them. The cost: restoring a
snapshot taken *before* a SourceMod update also reverts the gamedata that
update brought — re-run the SourceMod install afterwards if a game update is
involved.

## Troubleshooting

- **The Plugins tab doesn't appear** — check, in order: the plugin is listed
  under Administration → Plugins and the panel was restarted; the server's game
  code is one of the [supported ones](#which-games); your user has admin rights
  (the `plugin:onzggylemrxw4:manage` ability is granted to admins
  automatically).
- **"could not locate the mod directory"** (422 on load) — no `gameinfo.txt`
  was found in the server directory. The server probably isn't installed yet,
  or its launch command has no `-game` and its game code is not a stock one.
- **Metamod shows "Not active"** — `addons/metamod` exists but nothing loads
  it. Press *Fix loader*; it writes `addons/metamod.vdf` for the binary you
  have.
- **SourceMod shows "Not active"** — it is unpacked but not registered with
  Metamod. Re-enable its entry on the Metamod card, or reinstall SourceMod.
  This is the single most common "SourceMod does nothing" cause, and the Doctor
  names it too.
- **A grey hint line about RCON** — live statuses are unavailable for the
  stated reason (offline / no password / wrong password / empty response). Fix
  the RCON password in the server settings; file operations work regardless.
- **A row shows "Awaiting load" for a plugin that is clearly running** — the
  console could not be paired with the file. Check the Doctor for a load
  failure; if there is none, the plugin's title resolution may have hit the
  `sm plugins info` cap (12 per refresh).
- **Toggle fails with "already exists; refusing to overwrite"** — the same
  `.smx` sits in both `plugins/` and `plugins/disabled/`. Delete one of them in
  the file manager. The Doctor flags this too.
- **A catalog install fails with NO_MATCHING_ASSET** — that release has no
  `.zip` or `.tar.gz` matching the entry's pattern. Download it from the linked
  release page and use *Upload file*, which takes the same archives.
- **Platform install fails with DOWNLOAD_FAILED or EXTRACT_FAILED** — the node
  is missing `curl`/`wget` or `tar`; the error message names what was tried.
  Both features require a linux node.
- **I uploaded a plugin and nothing happened** — SourceMod loads `.smx` files
  only. A `.sp` is source; compile it with
  `addons/sourcemod/scripting/compile.sh` first. The upload dialog and the
  Doctor both say so.
- **Where is the manifest?**
  `<mod>/addons/sourcemod/configs/gameap/plugins_meta.json` — plain JSON, safe
  to hand-edit.

## Uninstall

Remove the plugin in Administration → Plugins (or delete the `.wasm` from the
panel's plugins directory) and restart the panel. Nothing on the game servers
is changed by uninstalling; anything you disabled stays in `plugins/disabled/`
until you move it back.

## Repository layout

```
src/                        Rust backend (compiled to wasm32-wasip1)
├── lib.rs                  plugin entry: info, routes, frontend bundle
├── router.rs               route table + dispatch
├── consistency.rs          tests that the Rust and TS halves still agree
├── handlers/               one file per route (+ tests.rs against a mock node)
├── source/                 Source 1 domain: mod dir, metamod.vdf, metaplugins.ini,
│                           gameinfo.txt, plugins_meta.json, archives, catalog
├── host_api.rs             trait over the SDK host calls + MockHost for native tests
└── http.rs                 JSON response/error helpers
frontend/                   Vue 3 frontend (vite lib build, embedded into the wasm)
├── src/index.ts            plugin definition, translations (en/ru), tab registration
├── src/components/         ModsTab, PlatformCard, PluginList, and the modals
├── src/lib/                rcon-parse, status, admins (all unit-tested)
└── src/api/                clients for the wasm routes and existing panel endpoints
```

Tests: `cargo test` (104 backend tests run natively against the mock node — no
panel needed) and `cd frontend && npm test` (53 vitest tests for the parsers,
status logic and the admins round-trip).

## Build

Requirements: Rust (the version pinned in `rust-toolchain.toml`, with the
`wasm32-wasip1` target), Node.js 22+, and a checkout of
[gameap/gameap](https://github.com/gameap/gameap) as a **sibling directory
named `gameap-api`** (the frontend depends on its `web/plugin-sdk`). The Rust
side pulls `gameap-plugin-sdk` as a cargo git dependency — no sibling checkout
needed for it.

```sh
# one-time: SDK build (sibling directory)
git clone https://github.com/gameap/gameap ../gameap-api
cd ../gameap-api/web/plugin-sdk
npm ci && npm install --no-save ../frontend/packages/gameap-ui && npx vite build
cd -

make build   # frontend (vite) + wasm (cargo, wasm32-wasip1) → source-addons.wasm
make test    # cargo test + vitest
make lint    # clippy, both targets, -D warnings
```

On Windows, `.\build.ps1` does the same without `make` (`-Test` also runs both
suites, `-SkipFrontend` builds only the wasm).

### Versioning

The version lives in **four** places that must agree: `Cargo.toml`,
`frontend/package.json`, `frontend/package-lock.json` (two root entries) and
`frontend/src/index.ts`. `cargo test` fails if any of them drifts — `npm ci`
does not check the root version field, so the lockfile is the copy most likely
to be left behind.

## License

MIT, same as the original.
