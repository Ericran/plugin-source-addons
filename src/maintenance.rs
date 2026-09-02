//! Scheduled maintenance: the Metamod loader sweep.
//!
//! A Steam validate, a full reinstall or a hand-edited gameinfo.txt can leave
//! `addons/metamod` sitting on disk with nothing loading it — from the outside
//! the server just quietly runs with no plugins at all. This walks every
//! Source 1 server that has Metamod installed and puts the loader back.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::handlers::{audit, repair};
use crate::host_api::HostApi;
use crate::source::{self, gameinfo, paths};

/// Never let one giant installation stall the scheduler slot.
const MAX_SERVERS_PER_SWEEP: usize = 100;

/// Stock GameAP game codes on Source 1. Kept in step with the frontend's
/// `checkGame.codes` list — both answer "is this plugin's tab for this game".
pub const SOURCE_GAME_CODES: &[&str] = &[
    "cssource",
    "cssv34",
    "csgo",
    "dods",
    "garrysmod",
    "hl2mp",
    "l4d",
    "l4d2",
    "tf2",
    "bms",
    "synergy",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepStats {
    pub checked: u32,
    pub repaired: u32,
    pub failed: u32,
}

pub fn loader_sweep<H: HostApi>(host: &mut H) -> SweepStats {
    let mut stats = SweepStats::default();
    let mut seen: Vec<u64> = Vec::new();

    for game_code in SOURCE_GAME_CODES {
        let servers = match host.find_servers_by_game(game_code) {
            Ok(servers) => servers,
            Err(err) => {
                host.log_error(&format!("loader sweep: {game_code} listing failed: {err:?}"));
                continue;
            }
        };
        for server in servers {
            if seen.len() >= MAX_SERVERS_PER_SWEEP {
                return stats;
            }
            if seen.contains(&server.id) {
                continue;
            }
            seen.push(server.id);

            let mut params = HashMap::new();
            params.insert("id".to_string(), server.id.to_string());
            let Ok(ctx) = ServerCtx::resolve(host, &params) else {
                continue; // not installed yet, wrong engine, node gone — skip
            };
            stats.checked += 1;

            // Only repair where Metamod is actually present; a bare server
            // with no addons dir is not broken, it is just not set up.
            let metamod_abs = paths::join(&ctx.mod_abs, source::METAMOD_DIR);
            let has_metamod = match host.stat(ctx.node_id, &metamod_abs) {
                Ok(stat) => stat.is_some_and(|s| s.is_dir),
                Err(_) => false,
            };
            if !has_metamod {
                continue;
            }

            // The gameinfo.txt method is a complete install on its own. Adding
            // a vdf on top would start Metamod twice.
            if gameinfo_wired(host, &ctx) {
                continue;
            }

            match repair::repair_vdf(host, &ctx) {
                Ok(true) => {
                    stats.repaired += 1;
                    audit::record(
                        host,
                        ctx.server_id,
                        Some("auto-repair"),
                        "metamod-repair",
                        "vdf",
                    );
                    host.log_info(&format!(
                        "loader sweep: rewrote addons/metamod.vdf on server {}",
                        ctx.server_id
                    ));
                }
                Ok(false) => {}
                Err(err) => {
                    stats.failed += 1;
                    host.log_error(&format!(
                        "loader sweep: server {} failed: {}",
                        ctx.server_id, err.message
                    ));
                }
            }
        }
    }
    stats
}

fn gameinfo_wired<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> bool {
    let gameinfo_abs = paths::join(&ctx.mod_abs, source::GAMEINFO_FILE);
    match host.stat(ctx.node_id, &gameinfo_abs) {
        Ok(Some(stat)) if !stat.is_dir => match host.download(ctx.node_id, &gameinfo_abs) {
            Ok(content) => gameinfo::is_gameinfo_wired(&content),
            Err(_) => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::mock::MockHost;

    #[test]
    fn sweep_writes_the_loader_only_where_metamod_is_installed() {
        let mut host = MockHost::css();
        host.add_dir(&format!("{}/addons/metamod", MockHost::MOD_ABS));
        host.add_file(
            &format!("{}/addons/metamod/bin/server.so", MockHost::MOD_ABS),
            b"\x7fELF",
        );

        let stats = loader_sweep(&mut host);
        assert_eq!(stats.checked, 1);
        assert_eq!(stats.repaired, 1);
        assert_eq!(stats.failed, 0);

        let vdf = host
            .file(&format!("{}/addons/metamod.vdf", MockHost::MOD_ABS))
            .expect("vdf written");
        assert!(crate::source::gameinfo::is_metamod_vdf(vdf));
        assert!(
            String::from_utf8_lossy(vdf).contains("../cstrike/addons/metamod/bin/server"),
            "the mod dir and the installed binary decide the path"
        );

        // Second sweep is a no-op.
        assert_eq!(loader_sweep(&mut host).repaired, 0);
    }

    #[test]
    fn sweep_skips_servers_without_metamod() {
        let mut host = MockHost::css();
        let stats = loader_sweep(&mut host);
        assert_eq!(stats.checked, 1);
        assert_eq!(stats.repaired, 0);
        assert!(
            host.file(&format!("{}/addons/metamod.vdf", MockHost::MOD_ABS))
                .is_none()
        );
    }

    #[test]
    fn sweep_leaves_a_gameinfo_install_alone() {
        // Wiring Metamod through gameinfo.txt is a complete install; adding a
        // vdf on top would start it a second time.
        let mut host = MockHost::css();
        host.add_dir(&format!("{}/addons/metamod", MockHost::MOD_ABS));
        host.add_file(
            &format!("{}/gameinfo.txt", MockHost::MOD_ABS),
            b"SearchPaths\n{\n\tGame\t|gameinfo_path|addons/metamod\n\tGame\t|gameinfo_path|.\n}\n",
        );

        let stats = loader_sweep(&mut host);
        assert_eq!(stats.checked, 1);
        assert_eq!(stats.repaired, 0);
        assert!(
            host.file(&format!("{}/addons/metamod.vdf", MockHost::MOD_ABS))
                .is_none()
        );
    }
}
