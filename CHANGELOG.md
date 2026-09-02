# Changelog

All notable changes to this plugin. Versions follow the plugin's four version
strings, which must agree — `Cargo.toml`, `frontend/package.json`,
`frontend/package-lock.json` (two root entries) and `frontend/src/index.ts`;
`cargo test` fails if any of them drifts.

> **On this repository's history.** The plugin was written outside version
> control and this repository was created at 0.1.2. The three commits below are
> reconstructions: each tree was rebuilt from the released state, and each was
> verified to build and pass its own test suite (99, 101 and 104 backend tests
> respectively, 53 frontend tests throughout) before being committed. The split
> between 0.1.1 and 0.1.2 follows the file modification times of the two editing
> sessions that produced them, and the test counts corroborate it — but these
> are not the original commits, and the documentation in each is written at
> feature level rather than recovered verbatim.

## [0.1.2] — 2026-08-31

### Added

- **Declares the permissions GameAP 4.5 grants.** 4.5 gates the privileged host
  libraries behind grants a plugin names in its manifest. This one declares
  three — `node_commands` (nodecmd), `files` (the nodefs writes) and
  `manage_servers` (servercontrol restart) — and nothing else: the read-only
  calls on `gameap-nodes` and `gameap-servers` are ungated, and `gameap-http`,
  `gameap-storage`, `gameap-scheduler`, `gameap-games` and `gameap-log` are not
  gated at all.

  The list is not a judgement call: the panel derives what a plugin *uses* from
  its wasm import section and flags anything used-but-undeclared. `files` is
  declared without `files_read` because a broader grant satisfies a narrower one
  and the panel's own derivation drops the subsumed entry.

  Three unit tests back this, because `ParsePluginPermissions` silently **drops**
  a name it does not recognise — a typo would not fail an install, it would grant
  less than intended and surface much later as a denied call. The tests pin the
  declared list against the panel's twelve known names, against what `get_info`
  actually reports, and against declaring both a grant and one it covers.

  Nothing is enforced yet: `PLUGINS_PERMISSIONS_ENFORCE` defaults to `false` in
  4.5, and a panel older than 4.5 ignores the field entirely.

- A **GameAP 4.5 section in the README**, covering the permissions, the 32 MiB
  nodefs ceiling, the `PLUGIN_*` → `PLUGINS_*` variable rename in `v4.5.0-rc.1`
  (the old spelling works for one more release), and the new default-on rate
  limits — flagging that the rate limits are the one figure there that has not
  been measured against a live panel.

### Known limitation

- The log viewer downloads a whole log file and keeps only its tail. Under 4.5
  that is now a ceiling on the feature, not just a heap guard: a log grown past
  `PLUGINS_NODEFS_MAX_INLINE` is refused outright rather than truncated, taking
  the Doctor's load-failure check with it. 4.5 also shipped the fix —
  `offset`/`length` on the nodefs `DownloadRequest` — but the pinned
  `gameap-proto` commit does not declare those fields yet, so the plugin cannot
  reach them. Documented in `src/handlers/logs.rs` for whoever gets there first.

## [0.1.1] — 2026-08-28

### Changed

- **Archive installs now honour GameAP 4.5's per-call nodefs limit.** 4.5 caps
  what one `gameap-nodefs` download or upload may carry
  (`PLUGINS_NODEFS_MAX_INLINE`, `32M` by default). The archive install's own size
  gate — previously an unrelated 32MB — is now the panel's number to the byte:
  the panel's `ByteSize` parser reads every suffix as binary, so both refuse at
  33,554,433. The plugin stats the file first, so its own message (which says to
  use the file manager instead) is what you see rather than the panel's generic
  *file too large*.

### Fixed

- **A single oversized member of an otherwise acceptable archive** would have
  been refused by the panel partway through the install, leaving a half-installed
  plugin behind. It is reachable: extraction bounds an archive's *total*
  uncompressed size at twice the inline limit and bounds no individual member.
  `write_archive_entries` now pre-flights the whole entry list and refuses with
  `ENTRY_TOO_LARGE` before writing a single byte.

  The boundary is pinned in both directions — an entry at exactly the limit is
  written, one byte over is refused — because the panel's own comparison is
  strict and the two would otherwise disagree at precisely one size.

  Note the margin is exactly zero: lowering `PLUGINS_NODEFS_MAX_INLINE` puts the
  panel's limit *under* these gates, and the refusal then comes from the panel
  with less to say. Nothing here can detect that — the panel does not report its
  limit to the guest.

## [0.1.0] — 2026-08-19

Initial release: a **Plugins** tab for Source 1 servers, managing
Metamod:Source and SourceMod on Counter-Strike: Source, Garry's Mod, Team
Fortress 2, Left 4 Dead / 2, Day of Defeat: Source, Half-Life 2: Deathmatch,
Black Mesa, Synergy and legacy CS:GO.

A Source 1 sibling of
[plugin-cs2-addons](https://github.com/booskibro/plugin-cs2-addons), itself
based on [gameap/plugin-goldsrc-addons](https://github.com/gameap/plugin-goldsrc-addons).

### Added

- Plugin table with live statuses from `sm plugins list`, persistent
  enable/disable by moving the `.smx` in and out of `plugins/disabled/`, and hot
  load / unload / reload / unpause over RCON.
- Per-plugin comments and groups in `plugins_meta.json`, inline-editable.
- Uploads of a `.smx`, or a whole release `.zip` **or `.tar.gz`** — the layout
  (`addons/`-rooted, `plugins/`-rooted, loose `.smx`) is detected and unpacked
  to the right place.
- One-click installs of Metamod:Source and SourceMod from the AlliedModders
  drops, with update badges when a newer build ships.
- A loader repair that writes `addons/metamod.vdf`, plus a 6-hour sweep that
  re-checks every Source server.
- Both Metamod plugin mechanisms in one list: the `addons/metamod/*.vdf` aliases
  and the `metaplugins.ini` lines.
- A catalog of well-known projects installable from their GitHub releases;
  snapshots with automatic pre-operation safety copies; a SourceMod admins
  editor that preserves the file's comments; the SourceMod logs with filtering
  and following; an audit history; a restart banner; and a Doctor dialog.

### Notes on the port

Four things are not a rename from the CS2 plugin:

1. **The tab gate is `checkGame.codes`, not `engines`.** The panel's gate is
   *engines OR codes* with no version component, and the engine string is too
   coarse in both directions: `cs2` is also engine `"Source"` (Source 2), and
   `ark` is mislabeled `"Source"` in the stock game list while being Unreal.
   `src/consistency.rs` fails the build if the game-code list, the version or the
   plugin id drift between the Rust and TypeScript halves.
2. **No `ProtocolService`.** Source 1 speaks the classic Source RCON the panel
   already implements correctly, so the tolerant-RCON engine and the
   `gameap-net` import are both gone.
3. **`sm plugins list` prints plugin titles, not filenames** — "Admin File
   Reader" is `admin-flatfile.smx`, and only *failed* plugins are printed by
   file. Rows are paired in three passes and whatever is left is resolved with
   `sm plugins info <#>`. Nothing is guessed: a wrong pairing would show one
   plugin's version and status on another plugin's row.
4. **The Metamod loader is `addons/metamod.vdf`, not `gameinfo.txt`.** Repair
   writes the vdf, choosing `bin/server` or `bin/linux64/server` from what is
   actually installed, and the sweep leaves a gameinfo-only install alone rather
   than starting Metamod twice.

Smaller ones: the archive extractor takes `.tar.gz` because AlliedModders ships
tarballs; snapshots tar from the mod directory because `cfg/sourcemod` lives
outside `addons/`; and the mod directory resolves from the launch command's
`-game` first, because `hl2/gameinfo.txt` exists in every Source install and
installing addons there is a silent no-op.

[0.1.2]: https://github.com/Ericran/plugin-source-addons/releases/tag/v0.1.2
[0.1.1]: https://github.com/Ericran/plugin-source-addons/releases/tag/v0.1.1
[0.1.0]: https://github.com/Ericran/plugin-source-addons/releases/tag/v0.1.0
