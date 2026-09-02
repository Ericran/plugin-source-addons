//! GET /servers/{id}/doctor — every cheap server-side health check in one
//! pass. The frontend adds its own checks (RCON reachability, rcon_password in
//! the launch command) on top.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::handlers::logs::{self, LogKind};
use crate::host_api::HostApi;
use crate::http::{ApiResult, json_response};
use crate::model::{DoctorCheck, DoctorResponse};
use crate::source::{self, gameinfo, paths};

fn ok(id: &str, detail: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id: id.into(),
        status: "ok".into(),
        detail: detail.into(),
    }
}

fn warn(id: &str, detail: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id: id.into(),
        status: "warn".into(),
        detail: detail.into(),
    }
}

fn fail(id: &str, detail: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id: id.into(),
        status: "fail".into(),
        detail: detail.into(),
    }
}

pub fn handle<H: HostApi>(host: &mut H, params: &HashMap<String, String>) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let mut checks = Vec::new();

    // --- Metamod dir ---
    let metamod_abs = paths::join(&ctx.mod_abs, source::METAMOD_DIR);
    let metamod_dir = host
        .stat(ctx.node_id, &metamod_abs)?
        .is_some_and(|s| s.is_dir);
    checks.push(if metamod_dir {
        ok("metamod", ctx.rel(source::METAMOD_DIR))
    } else {
        warn("metamod", "addons/metamod is not installed")
    });

    // --- The loader: either mechanism counts, neither is a hard failure ---
    let vdf_abs = paths::join(&ctx.mod_abs, source::METAMOD_VDF);
    let vdf_present = match host.stat(ctx.node_id, &vdf_abs)? {
        Some(stat) if !stat.is_dir => {
            gameinfo::is_metamod_vdf(&host.download(ctx.node_id, &vdf_abs)?)
        }
        _ => false,
    };
    let gameinfo_abs = paths::join(&ctx.mod_abs, source::GAMEINFO_FILE);
    let gameinfo_wired = match host.stat(ctx.node_id, &gameinfo_abs)? {
        Some(stat) if !stat.is_dir => {
            gameinfo::is_gameinfo_wired(&host.download(ctx.node_id, &gameinfo_abs)?)
        }
        _ => false,
    };
    checks.push(match (metamod_dir, vdf_present, gameinfo_wired) {
        (_, true, true) => ok("loader", "loaded via addons/metamod.vdf and gameinfo.txt"),
        (_, true, false) => ok("loader", "loaded via addons/metamod.vdf"),
        (_, false, true) => ok("loader", "loaded via the gameinfo.txt search path"),
        (true, false, false) => fail(
            "loader",
            "Metamod is installed but nothing loads it - no addons/metamod.vdf and no gameinfo.txt search path. Use Fix on the Metamod card",
        ),
        (false, false, false) => ok("loader", "no Metamod to load"),
    });

    // A vdf naming a binary that is not there loads nothing, silently.
    if vdf_present {
        let content = host.download(ctx.node_id, &vdf_abs)?;
        let text = String::from_utf8_lossy(&content).into_owned();
        checks.push(match loader_target(&text) {
            None => warn("loaderbin", "addons/metamod.vdf has no readable \"file\" value"),
            Some(rel) => {
                let found = ["so", "dll"].into_iter().any(|ext| {
                    let abs = paths::join(&ctx.mod_abs, &format!("{rel}.{ext}"));
                    host.stat(ctx.node_id, &abs)
                        .ok()
                        .flatten()
                        .is_some_and(|s| !s.is_dir)
                });
                if found {
                    ok("loaderbin", format!("{rel} is present"))
                } else {
                    fail(
                        "loaderbin",
                        format!(
                            "addons/metamod.vdf points at {rel}, which does not exist - Metamod will not start. Use Fix to rewrite it for the binary that is installed"
                        ),
                    )
                }
            }
        });
    }

    // --- SourceMod ---
    let sm_abs = super::sm_abs(&ctx);
    let sm_dir = host.stat(ctx.node_id, &sm_abs)?.is_some_and(|s| s.is_dir);
    checks.push(if sm_dir {
        ok("sourcemod", ctx.rel(source::SM_DIR))
    } else {
        warn("sourcemod", "SourceMod is not installed")
    });

    if sm_dir {
        let bin_present = host
            .stat(ctx.node_id, &paths::join(&ctx.mod_abs, source::SM_BIN_DIR))?
            .is_some_and(|s| s.is_dir);
        checks.push(if bin_present {
            ok("smbin", "SourceMod binaries present")
        } else {
            fail(
                "smbin",
                "addons/sourcemod exists but addons/sourcemod/bin does not - the install is incomplete (configs left over from a previous one?). Reinstall SourceMod",
            )
        });

        // SourceMod is itself a Metamod plugin. Without a registration it
        // never loads, however complete its own directory is - the single most
        // common "SourceMod does nothing" cause.
        let registered = super::state::collect_metamod_plugins(host, &ctx, &metamod_abs)?
            .into_iter()
            .any(|entry| entry.platform && entry.enabled);
        checks.push(if registered {
            ok("smregistered", "registered with Metamod")
        } else {
            fail(
                "smregistered",
                "SourceMod is not registered with Metamod - no live addons/metamod/sourcemod.vdf and no metaplugins.ini line. Reinstall SourceMod, or re-enable its entry on the Metamod card",
            )
        });

        // Same .smx present in both plugins/ and plugins/disabled/.
        let enabled_names = smx_names(host, &ctx, &super::sm_plugins_abs(&ctx))?;
        let disabled_names = smx_names(host, &ctx, &super::sm_disabled_abs(&ctx))?;
        let duplicates: Vec<&String> = enabled_names
            .iter()
            .filter(|name| {
                disabled_names
                    .iter()
                    .any(|other| other.eq_ignore_ascii_case(name))
            })
            .collect();
        checks.push(if duplicates.is_empty() {
            ok("duplicates", "no plugin is both enabled and disabled")
        } else {
            fail(
                "duplicates",
                format!(
                    "in both plugins/ and plugins/disabled/: {} - delete one copy",
                    duplicates
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        });

        // Manifest entries whose .smx is gone.
        let manifest = super::read_manifest(host, &ctx)?;
        let orphans: Vec<String> = manifest
            .names()
            .filter(|name| {
                !enabled_names.iter().any(|n| n.eq_ignore_ascii_case(name))
                    && !disabled_names.iter().any(|n| n.eq_ignore_ascii_case(name))
            })
            .map(str::to_string)
            .collect();
        checks.push(if orphans.is_empty() {
            ok("orphans", "plugins_meta.json matches the files on disk")
        } else {
            warn(
                "orphans",
                format!("tracked but the .smx is gone: {}", orphans.join(", ")),
            )
        });

        // cfg/sourcemod is where SourceMod writes each plugin's generated cvar
        // config. A missing directory means those configs are being lost.
        let cfg_abs = paths::join(&ctx.mod_abs, source::SM_CFG_DIR);
        checks.push(
            if host.stat(ctx.node_id, &cfg_abs)?.is_some_and(|s| s.is_dir) {
                ok("cfgdir", ctx.rel(source::SM_CFG_DIR))
            } else {
                warn(
                    "cfgdir",
                    "cfg/sourcemod does not exist - per-plugin configs cannot be generated there. It appears once the server runs with SourceMod loaded",
                )
            },
        );

        // Source files shipped without their build output: a common
        // "I uploaded the plugin and nothing happened".
        let scripting_abs = paths::join(&ctx.mod_abs, "addons/sourcemod/scripting");
        let unbuilt: Vec<String> = host
            .read_dir(ctx.node_id, &scripting_abs)?
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| !entry.is_dir && entry.name.to_ascii_lowercase().ends_with(".sp"))
            .map(|entry| paths::file_stem(&entry.name).to_string())
            .filter(|stem| {
                !enabled_names.iter().any(|n| n.eq_ignore_ascii_case(stem))
                    && !disabled_names.iter().any(|n| n.eq_ignore_ascii_case(stem))
            })
            .take(10)
            .collect();
        checks.push(if unbuilt.is_empty() {
            ok("scripting", "every uploaded source has a compiled plugin")
        } else {
            warn(
                "scripting",
                format!(
                    "source uploaded but never compiled to a .smx: {} - SourceMod loads .smx files only, compile with addons/sourcemod/scripting/compile.sh",
                    unbuilt.join(", ")
                ),
            )
        });

        // A plugin that failed to load says why in the error log, and nowhere
        // in the console - so this is the only place the reason surfaces.
        checks.push(match logs::newest_log_tail(host, &ctx, LogKind::Errors)? {
            None => ok("loadfail", "no SourceMod error log to read yet"),
            Some(tail) => {
                let failures = logs::find_load_failures(&tail.lines);
                if failures.is_empty() {
                    ok("loadfail", format!("no load failures in {}", tail.file_rel))
                } else {
                    let detail = failures
                        .iter()
                        .map(|failure| match &failure.reason {
                            Some(reason) => format!("{}: {reason}", failure.plugin),
                            None => format!("{} failed to load", failure.plugin),
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    fail("loadfail", detail)
                }
            }
        });
    }

    if metamod_dir {
        // A vdf that exists both live and parked loads unpredictably.
        let vdfs = source::vdf::list(host, ctx.node_id, &metamod_abs)?;
        let mut seen: Vec<String> = Vec::new();
        let mut ambiguous: Vec<String> = Vec::new();
        for plugin in &vdfs {
            let lower = plugin.name.to_ascii_lowercase();
            if seen.contains(&lower) {
                if !ambiguous.contains(&plugin.name) {
                    ambiguous.push(plugin.name.clone());
                }
            } else {
                seen.push(lower);
            }
        }
        checks.push(if ambiguous.is_empty() {
            ok("vdf", "Metamod plugin aliases are unambiguous")
        } else {
            fail(
                "vdf",
                format!(
                    "both .vdf and .vdf.disabled exist for: {} - delete one",
                    ambiguous.join(", ")
                ),
            )
        });

        // metaplugins.ini entries whose binary is missing.
        let ini_abs = paths::join(&ctx.mod_abs, source::METAPLUGINS_INI);
        if host.stat(ctx.node_id, &ini_abs)?.is_some_and(|s| !s.is_dir) {
            let content = host.download(ctx.node_id, &ini_abs)?;
            let mut missing = Vec::new();
            for entry in source::metaplugins::parse(&content) {
                if !entry.enabled || paths::sanitize_rel_path(&entry.path).is_err() {
                    continue;
                }
                let abs = paths::join(&ctx.mod_abs, &entry.path);
                if host.stat(ctx.node_id, &abs)?.is_none_or(|s| s.is_dir) {
                    missing.push(entry.path);
                }
            }
            checks.push(if missing.is_empty() {
                ok("metaplugins", "every metaplugins.ini entry exists on disk")
            } else {
                fail(
                    "metaplugins",
                    format!(
                        "metaplugins.ini lists binaries that are not there: {} - Metamod logs an error for each at startup",
                        missing.join(", ")
                    ),
                )
            });
        }
    }

    // Leftover download scratch from an interrupted platform install.
    let scratch_abs = paths::join(&ctx.root_abs, source::DOWNLOAD_SCRATCH_DIR);
    if let Some(entries) = host.read_dir(ctx.node_id, &scratch_abs)?
        && !entries.is_empty()
    {
        checks.push(warn(
            "scratch",
            format!(
                "{} leftover download(s) in {}/ from an interrupted install - safe to delete",
                entries.len(),
                source::DOWNLOAD_SCRATCH_DIR
            ),
        ));
    } else {
        checks.push(ok("scratch", "no leftover downloads"));
    }

    Ok(json_response(200, &DoctorResponse { checks }))
}

/// The mod-relative loader path out of a metamod.vdf `"file" "../mod/…"` line.
fn loader_target(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase().replace('\\', "/");
        if !lower.contains("addons/metamod/") {
            continue;
        }
        // The value is the last quoted token on the line.
        let normalized = line.replace('\\', "/");
        let value = normalized.rsplit('"').nth(1)?.trim().to_string();
        // "../cstrike/addons/metamod/bin/server" → "addons/metamod/bin/server"
        let start = value.to_ascii_lowercase().find("addons/metamod/")?;
        return Some(value[start..].to_string());
    }
    None
}

/// Plugin file stems (`.smx` only) directly inside a directory.
fn smx_names<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    dir_abs: &str,
) -> Result<Vec<String>, crate::host_api::HostApiError> {
    let Some(entries) = host.read_dir(ctx.node_id, dir_abs)? else {
        return Ok(Vec::new());
    };
    Ok(entries
        .into_iter()
        .filter(|e| !e.is_dir && e.name.to_ascii_lowercase().ends_with(source::SMX_EXT))
        .map(|e| paths::file_stem(&e.name).to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_loader_path_out_of_a_vdf() {
        let vdf = "\"Metamod Plugin\"\n{\n\t\"file\"\t\"../cstrike/addons/metamod/bin/server\"\n}\n";
        assert_eq!(
            loader_target(vdf).as_deref(),
            Some("addons/metamod/bin/server")
        );
        let vdf64 =
            "\"Metamod Plugin\"\n{\n\t\"file\" \"../left4dead2/addons/metamod/bin/linux64/server\"\n}\n";
        assert_eq!(
            loader_target(vdf64).as_deref(),
            Some("addons/metamod/bin/linux64/server")
        );
        assert_eq!(loader_target("\"Plugin\"\n{\n}\n"), None);
    }
}
