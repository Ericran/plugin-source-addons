//! Native handler tests against the MockHost fake node.

use std::collections::HashMap;

use serde_json::Value;

use crate::host_api::mock::MockHost;
use crate::http::ApiError;
use crate::router;

fn params(id: &str) -> HashMap<String, String> {
    HashMap::from([("id".to_string(), id.to_string())])
}

fn query(pairs: &[(&str, &str)]) -> HashMap<String, gameap_plugin_sdk::proto::gameap::plugin::QueryParamValues> {
    pairs
        .iter()
        .map(|(key, value)| {
            (
                (*key).to_string(),
                gameap_plugin_sdk::proto::gameap::plugin::QueryParamValues {
                    values: vec![(*value).to_string()],
                },
            )
        })
        .collect()
}

fn body_json(result: Result<gameap_plugin_sdk::proto::gameap::plugin::HttpResponse, ApiError>)
-> (i32, Value) {
    let resp = result.unwrap_or_else(|err| err.into_response());
    let value: Value = serde_json::from_slice(&resp.body).expect("json body");
    (resp.status_code, value)
}

const MOD: &str = MockHost::MOD_ABS;
const MANIFEST_ABS: &str =
    "/srv/gameap/servers/css/cstrike/addons/sourcemod/configs/gameap/plugins_meta.json";

fn with_sourcemod(host: &mut MockHost) {
    host.add_dir(&format!("{MOD}/addons/sourcemod"));
    host.add_dir(&format!("{MOD}/addons/sourcemod/plugins"));
    host.add_dir(&format!("{MOD}/addons/sourcemod/bin"));
    // SourceMod ships its own Metamod alias; without it nothing loads.
    host.add_file(&format!("{MOD}/addons/metamod/sourcemod.vdf"), b"\"Metamod Plugin\"\n");
}

fn with_metamod(host: &mut MockHost) {
    host.add_dir(&format!("{MOD}/addons/metamod"));
    host.add_file(&format!("{MOD}/addons/metamod/bin/server.so"), b"\x7fELF");
    host.add_file(
        &format!("{MOD}/addons/metamod.vdf"),
        b"\"Metamod Plugin\"\n{\n\t\"file\"\t\"../cstrike/addons/metamod/bin/server\"\n}\n",
    );
}

fn add_plugin(host: &mut MockHost, name: &str) {
    host.add_file(
        &format!("{MOD}/addons/sourcemod/plugins/{name}.smx"),
        b"SP\xff\x01",
    );
}

fn add_disabled_plugin(host: &mut MockHost, name: &str) {
    host.add_file(
        &format!("{MOD}/addons/sourcemod/plugins/disabled/{name}.smx"),
        b"SP\xff\x01",
    );
}

// ---------------------------------------------------------------- state

#[test]
fn state_assembles_platforms_and_plugins() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    with_sourcemod(&mut host);
    add_plugin(&mut host, "funcommands");
    add_plugin(&mut host, "sbpp_main");
    add_disabled_plugin(&mut host, "rtd");
    host.add_file(&format!("{MOD}/cfg/sourcemod/sbpp_main.cfg"), b"// cvars");
    host.add_file(
        MANIFEST_ABS,
        br#"{"sbpp_main": {"Comment": "shared bans", "Group": "admin"},
             "gone": {"Comment": "", "Group": ""}}"#,
    );

    let (status, body) = body_json(crate::handlers::state::handle(&mut host, &params("3")));
    assert_eq!(status, 200);
    assert_eq!(body["engine"], "Source");
    assert_eq!(body["mod_dir"], "cstrike");
    assert_eq!(body["metamod"]["installed"], true);
    assert_eq!(body["metamod"]["vdf_present"], true);
    assert_eq!(body["metamod"]["gameinfo_wired"], false);
    assert_eq!(body["metamod"]["sourcemod_registered"], true);
    assert_eq!(body["sourcemod"]["installed"], true);
    assert_eq!(body["sourcemod"]["bin_present"], true);
    assert_eq!(
        body["paths"]["sm_plugins_dir"],
        "cstrike/addons/sourcemod/plugins"
    );

    let plugins = body["sourcemod"]["plugins"]
        .as_array()
        .expect("plugins array");
    assert_eq!(plugins.len(), 4);

    let by_name = |name: &str| {
        plugins
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("{name} present"))
    };
    let sbpp = by_name("sbpp_main");
    assert_eq!(sbpp["file"], "sbpp_main.smx");
    assert_eq!(sbpp["enabled"], true);
    assert_eq!(sbpp["missing"], false);
    assert_eq!(sbpp["comment"], "shared bans");
    assert_eq!(sbpp["group"], "admin");
    assert_eq!(sbpp["has_config"], true);
    assert_eq!(sbpp["config_path"], "cstrike/cfg/sourcemod/sbpp_main.cfg");

    let rtd = by_name("rtd");
    assert_eq!(rtd["enabled"], false);
    assert_eq!(rtd["missing"], false);

    assert_eq!(by_name("gone")["missing"], true);

    let fun = by_name("funcommands");
    assert_eq!(fun["group_title"], Value::Null);
    assert_eq!(fun["group_index"], u32::MAX as u64);
    assert_eq!(fun["has_config"], false);
}

#[test]
fn state_finds_the_mod_dir_past_the_base_content_decoy() {
    // Every Source 1 install ships hl2/gameinfo.txt beside the real mod dir.
    // A custom game entry has no known mod dir, so the launch command decides.
    let mut host = MockHost::css();
    host.servers.get_mut(&3).expect("server").game_code = "custom".into();
    host.games.insert(
        "custom".into(),
        crate::host_api::GameInfo {
            code: "custom".into(),
            name: "Custom Source mod".into(),
            engine: "Source".into(),
            engine_version: "1".into(),
        },
    );
    with_sourcemod(&mut host);

    let (status, body) = body_json(crate::handlers::state::handle(&mut host, &params("3")));
    assert_eq!(status, 200);
    assert_eq!(body["mod_dir"], "cstrike", "hl2/ must not win");
}

#[test]
fn state_rejects_source2_and_non_source_servers() {
    for (engine, version) in [("source", "2"), ("GoldSource", "1"), ("unreal", "4")] {
        let mut host = MockHost::css();
        let game = host.games.get_mut("cssource").expect("game");
        game.engine = engine.into();
        game.engine_version = version.into();

        let (status, body) = body_json(crate::handlers::state::handle(&mut host, &params("3")));
        assert_eq!(status, 422, "{engine} v{version}");
        assert_eq!(body["code"], "UNSUPPORTED_ENGINE");
    }
}

// ---------------------------------------------------------------- toggle

#[test]
fn toggle_disable_moves_the_smx_into_disabled() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","enabled":false}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);
    assert_eq!(
        host.moves,
        vec![(
            format!("{MOD}/addons/sourcemod/plugins/rtd.smx"),
            format!("{MOD}/addons/sourcemod/plugins/disabled/rtd.smx"),
        )]
    );
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/disabled/rtd.smx"))
            .is_some()
    );
}

#[test]
fn toggle_enable_moves_it_back() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_disabled_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","enabled":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx"))
            .is_some()
    );
}

#[test]
fn toggle_is_idempotent() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","enabled":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], false);
    assert!(host.moves.is_empty());
}

#[test]
fn toggle_conflicts_when_both_copies_exist() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    add_disabled_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","enabled":false}"#,
        None,
    ));
    assert_eq!(status, 409);
    assert_eq!(body["code"], "TARGET_EXISTS");
}

#[test]
fn toggle_unknown_plugin_404s() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let (status, body) = body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"nope","enabled":false}"#,
        None,
    ));
    assert_eq!(status, 404);
    assert_eq!(body["code"], "PLUGIN_NOT_FOUND");
}

#[test]
fn toggle_rejects_escapes_extensions_and_the_disabled_dir() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    for name in ["../evil", "a/b", "disabled", "rtd.smx"] {
        let body = format!(r#"{{"name":"{name}","enabled":false}}"#);
        let (status, _) = body_json(crate::handlers::toggle::handle(
            &mut host,
            &params("3"),
            body.as_bytes(),
            None,
        ));
        assert_eq!(status, 400, "{name}");
    }
}

// ---------------------------------------------------------------- attributes

#[test]
fn attributes_write_the_manifest() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::attributes::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","comment":"fun night only","group":"fun"}"#,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);

    let written = host.file(MANIFEST_ABS).expect("manifest written");
    let manifest = crate::source::manifest::Manifest::parse(written);
    assert_eq!(manifest.comment("rtd").as_deref(), Some("fun night only"));
    assert_eq!(manifest.group("rtd").as_deref(), Some("fun"));
}

#[test]
fn attributes_noop_skips_the_write() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    host.add_file(MANIFEST_ABS, br#"{"rtd":{"Comment":"x","Group":""}}"#);

    let (_, body) = body_json(crate::handlers::attributes::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","comment":"x"}"#,
    ));
    assert_eq!(body["changed"], false);
    assert_eq!(
        host.file(MANIFEST_ABS).expect("untouched"),
        br#"{"rtd":{"Comment":"x","Group":""}}"#
    );
}

#[test]
fn attributes_reject_control_characters() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    let (status, _) = body_json(crate::handlers::attributes::handle(
        &mut host,
        &params("3"),
        b"{\"name\":\"rtd\",\"comment\":\"a\\nb\"}",
    ));
    assert_eq!(status, 400);
}

// ---------------------------------------------------------------- add / remove

#[test]
fn add_registers_an_uploaded_plugin() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");

    let (status, body) = body_json(crate::handlers::add::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd"}"#,
        Some("erica"),
    ));
    assert_eq!(status, 201);
    assert_eq!(body["replaced"], false);
    assert!(
        crate::source::manifest::Manifest::parse(host.file(MANIFEST_ABS).expect("manifest"))
            .contains("rtd")
    );
}

#[test]
fn add_requires_the_upload_and_force_for_a_replace() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);

    let (status, body) = body_json(crate::handlers::add::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd"}"#,
        None,
    ));
    assert_eq!(status, 422);
    assert_eq!(body["code"], "FILE_NOT_UPLOADED");

    add_plugin(&mut host, "rtd");
    body_json(crate::handlers::add::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd"}"#,
        None,
    ));
    let (status, body) = body_json(crate::handlers::add::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd"}"#,
        None,
    ));
    assert_eq!(status, 409);
    assert_eq!(body["code"], "ALREADY_REGISTERED");

    let (status, body) = body_json(crate::handlers::add::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","force":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["replaced"], true);
}

#[test]
fn remove_deletes_the_file_and_the_manifest_entry_but_keeps_configs() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    host.add_file(&format!("{MOD}/cfg/sourcemod/rtd.cfg"), b"// cvars");
    host.add_file(MANIFEST_ABS, br#"{"rtd":{"Comment":"","Group":""}}"#);

    let (status, body) = body_json(crate::handlers::remove::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd"}"#,
        &HashMap::new(),
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["file_deleted"], true);
    assert_eq!(body["entry_removed"], true);
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx"))
            .is_none()
    );
    assert!(
        host.file(&format!("{MOD}/cfg/sourcemod/rtd.cfg")).is_some(),
        "a tuned config survives the plugin"
    );
}

#[test]
fn remove_accepts_the_query_param_fallback() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");

    let (status, _) = body_json(crate::handlers::remove::handle(
        &mut host,
        &params("3"),
        b"",
        &query(&[("name", "rtd")]),
        None,
    ));
    assert_eq!(status, 200);
}

#[test]
fn remove_unknown_plugin_404s() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let (status, body) = body_json(crate::handlers::remove::handle(
        &mut host,
        &params("3"),
        br#"{"name":"nope"}"#,
        &HashMap::new(),
        None,
    ));
    assert_eq!(status, 404);
    assert_eq!(body["code"], "PLUGIN_NOT_FOUND");
}

// ---------------------------------------------------------------- metamod

#[test]
fn repair_writes_the_loader_vdf_for_the_installed_binary() {
    let mut host = MockHost::css();
    host.add_dir(&format!("{MOD}/addons/metamod"));
    // 64-bit only install: the loader lives one directory deeper.
    host.add_file(
        &format!("{MOD}/addons/metamod/bin/linux64/server.so"),
        b"\x7fELF",
    );

    let (status, body) = body_json(crate::handlers::repair::handle(
        &mut host,
        &params("3"),
        b"",
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);
    assert_eq!(body["method"], "vdf");

    let vdf = host
        .file(&format!("{MOD}/addons/metamod.vdf"))
        .expect("vdf written");
    assert!(
        String::from_utf8_lossy(vdf).contains("../cstrike/addons/metamod/bin/linux64/server")
    );

    // Idempotent.
    let (_, body) = body_json(crate::handlers::repair::handle(
        &mut host,
        &params("3"),
        b"",
        None,
    ));
    assert_eq!(body["changed"], false);
}

#[test]
fn repair_refuses_when_metamod_is_not_installed() {
    let mut host = MockHost::css();
    let (status, body) = body_json(crate::handlers::repair::handle(
        &mut host,
        &params("3"),
        b"",
        None,
    ));
    assert_eq!(status, 422);
    assert_eq!(body["code"], "METAMOD_NOT_INSTALLED");
}

#[test]
fn repair_can_also_patch_gameinfo_on_request() {
    let mut host = MockHost::css();
    with_metamod(&mut host);

    let (status, body) = body_json(crate::handlers::repair::handle(
        &mut host,
        &params("3"),
        br#"{"gameinfo":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    // The vdf was already good, so only gameinfo changed.
    assert_eq!(body["method"], "gameinfo");
    let gi = host
        .file(&format!("{MOD}/gameinfo.txt"))
        .expect("gameinfo");
    assert!(crate::source::gameinfo::is_gameinfo_wired(gi));
}

#[test]
fn metamod_toggle_renames_the_vdf_and_guards_sourcemod() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    with_sourcemod(&mut host);
    host.add_file(&format!("{MOD}/addons/metamod/stripper.vdf"), b"\"Metamod Plugin\"");

    let (status, body) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"stripper","enabled":false}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);
    assert!(
        host.file(&format!("{MOD}/addons/metamod/stripper.vdf.disabled"))
            .is_some()
    );

    // SourceMod's own alias needs an explicit force.
    let (status, body) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"sourcemod","enabled":false}"#,
        None,
    ));
    assert_eq!(status, 409);
    assert_eq!(body["code"], "PLATFORM_ENTRY");
    assert!(
        host.file(&format!("{MOD}/addons/metamod/sourcemod.vdf"))
            .is_some()
    );

    let (status, _) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"sourcemod","enabled":false,"force":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert!(
        host.file(&format!("{MOD}/addons/metamod/sourcemod.vdf.disabled"))
            .is_some()
    );
}

#[test]
fn metamod_toggle_comments_a_metaplugins_line() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    host.add_file(
        &format!("{MOD}/addons/metamod/metaplugins.ini"),
        b"; loaded plugins\naddons/stripper/bin/stripper_mm_i486.so\n",
    );

    let (status, body) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"stripper_mm_i486","enabled":false,"source":"ini","line":1}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["changed"], true);
    let ini = host
        .file(&format!("{MOD}/addons/metamod/metaplugins.ini"))
        .expect("ini");
    assert_eq!(
        String::from_utf8_lossy(ini),
        "; loaded plugins\n; addons/stripper/bin/stripper_mm_i486.so\n"
    );

    // A line number that is not a plugin entry is a 404, not a silent no-op.
    let (status, body) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"whatever","enabled":false,"source":"ini","line":0}"#,
        None,
    ));
    assert_eq!(status, 404);
    assert_eq!(body["code"], "METAPLUGINS_LINE_NOT_FOUND");
}

#[test]
fn metaplugins_registration_of_sourcemod_is_guarded_too() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    host.add_file(
        &format!("{MOD}/addons/metamod/metaplugins.ini"),
        b"addons/sourcemod/bin/sourcemod_mm_i486.so\n",
    );

    let (status, body) = body_json(crate::handlers::metamod::handle(
        &mut host,
        &params("3"),
        br#"{"name":"sourcemod_mm_i486","enabled":false,"source":"ini","line":0}"#,
        None,
    ));
    assert_eq!(status, 409);
    assert_eq!(body["code"], "PLATFORM_ENTRY");
}

#[test]
fn state_lists_both_metamod_mechanisms() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    with_sourcemod(&mut host);
    host.add_file(
        &format!("{MOD}/addons/metamod/stripper.vdf.disabled"),
        b"\"Metamod Plugin\"",
    );
    host.add_file(
        &format!("{MOD}/addons/metamod/metaplugins.ini"),
        b"addons/nextmap/nextmap.so\n",
    );

    let (_, body) = body_json(crate::handlers::state::handle(&mut host, &params("3")));
    let plugins = body["metamod"]["plugins"].as_array().expect("array");
    let by_name = |name: &str| {
        plugins
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("{name} present"))
    };

    let sm = by_name("sourcemod");
    assert_eq!(sm["enabled"], true);
    assert_eq!(sm["platform"], true);
    assert_eq!(sm["source"], "vdf");

    let stripper = by_name("stripper");
    assert_eq!(stripper["enabled"], false);
    assert_eq!(stripper["platform"], false);

    let nextmap = by_name("nextmap");
    assert_eq!(nextmap["source"], "ini");
    assert_eq!(nextmap["line"], 0);
    assert_eq!(nextmap["path"], "addons/nextmap/nextmap.so");
}

// ---------------------------------------------------------------- misc routes

#[test]
fn dispatch_serves_the_state_route() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let response = router::dispatch(
        &mut host,
        &gameap_plugin_sdk::proto::gameap::plugin::HttpRequest {
            method: "GET".into(),
            path: "/servers/3/state".into(),
            ..Default::default()
        },
    );
    assert_eq!(response.status_code, 200);
}

#[test]
fn restart_calls_servercontrol() {
    let mut host = MockHost::css();
    let (status, body) = body_json(crate::handlers::restart::handle(
        &mut host,
        &params("3"),
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["restarted"], true);
    assert_eq!(host.restarts, vec![3]);
}

#[test]
fn logs_serve_the_error_series_by_default() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let logs = format!("{MOD}/addons/sourcemod/logs");
    host.add_file(&format!("{logs}/L20260818.log"), b"old general\n");
    host.add_file(&format!("{logs}/L20260819.log"), b"new general\n");
    host.add_file(&format!("{logs}/errors_20260819.log"), b"an error\n");

    let (status, body) = body_json(crate::handlers::logs::handle(
        &mut host,
        &params("3"),
        &HashMap::new(),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "errors");
    assert_eq!(body["file"], "cstrike/addons/sourcemod/logs/errors_20260819.log");
    assert_eq!(body["lines"][0], "an error");

    // The general series is picked by name, and never mixed with errors_*.
    let (_, body) = body_json(crate::handlers::logs::handle(
        &mut host,
        &params("3"),
        &query(&[("kind", "general")]),
    ));
    assert_eq!(body["kind"], "general");
    assert_eq!(body["file"], "cstrike/addons/sourcemod/logs/L20260819.log");
    assert_eq!(body["lines"][0], "new general");
}

#[test]
fn logs_empty_without_a_logs_dir() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let (status, body) = body_json(crate::handlers::logs::handle(
        &mut host,
        &params("3"),
        &HashMap::new(),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["file"], Value::Null);
    assert_eq!(body["lines"].as_array().expect("array").len(), 0);
}

#[test]
fn audit_route_returns_recorded_entries() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    body_json(crate::handlers::toggle::handle(
        &mut host,
        &params("3"),
        br#"{"name":"rtd","enabled":false}"#,
        Some("erica"),
    ));

    let (status, body) = body_json(crate::handlers::audit::handle(&mut host, &params("3")));
    assert_eq!(status, 200);
    let entries = body["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["action"], "plugin-disable");
    assert_eq!(entries[0]["subject"], "rtd");
    assert_eq!(entries[0]["user"], "erica");
}

// ---------------------------------------------------------------- snapshots

#[test]
fn snapshot_create_list_restore_delete() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    host.add_dir(&format!("{MOD}/cfg/sourcemod"));

    let (status, body) = body_json(crate::handlers::snapshots::handle_create(
        &mut host,
        &params("3"),
        Some("erica"),
    ));
    assert_eq!(status, 200);
    let name = body["snapshot"]["name"].as_str().expect("name").to_string();
    assert!(name.starts_with("snap-"));

    // tar ran from the mod dir so cfg/sourcemod is reachable alongside addons/.
    let (command, work_dir) = host.execs.last().cloned().expect("tar ran");
    assert!(command.starts_with("tar -cf "));
    assert!(command.contains("addons/sourcemod/plugins"));
    assert!(command.contains("cfg/sourcemod"));
    assert_eq!(work_dir.as_deref(), Some(MOD));

    // The mock's exec does not actually write a tar, so place one to list.
    host.add_file(
        &format!("{MOD}/addons/sourcemod/backups/{name}.tar"),
        b"tarbytes",
    );
    let (_, body) = body_json(crate::handlers::snapshots::handle_list(
        &mut host,
        &params("3"),
    ));
    let snapshots = body["snapshots"].as_array().expect("snapshots");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0]["name"], name.as_str());

    let restore_body = format!(r#"{{"name":"{name}"}}"#);
    let (status, body) = body_json(crate::handlers::snapshots::handle_restore(
        &mut host,
        &params("3"),
        restore_body.as_bytes(),
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["restored"], true);
    // The live dirs are wiped before extracting, so a file deleted since the
    // snapshot cannot survive it.
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx"))
            .is_none()
    );
    // ...and the snapshot itself is never inside what gets wiped.
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/backups/{name}.tar"))
            .is_some()
    );

    let (status, _) = body_json(crate::handlers::snapshots::handle_delete(
        &mut host,
        &params("3"),
        restore_body.as_bytes(),
        None,
    ));
    assert_eq!(status, 200);
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/backups/{name}.tar"))
            .is_none()
    );
}

#[test]
fn snapshot_restore_rejects_bad_names() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    for name in ["../../etc/passwd", "snap-abc", "nope"] {
        let body = format!(r#"{{"name":"{name}"}}"#);
        let (status, _) = body_json(crate::handlers::snapshots::handle_restore(
            &mut host,
            &params("3"),
            body.as_bytes(),
            None,
        ));
        assert_eq!(status, 400, "{name}");
    }
}

// ---------------------------------------------------------------- archives

/// Builds a zip in memory, the shape a release ships.
fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        for (name, data) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .expect("start file");
            writer.write_all(data).expect("write");
        }
        writer.finish().expect("finish");
    }
    cursor.into_inner()
}

#[test]
fn archive_install_extracts_registers_and_cleans_up() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let archive = zip_of(&[
        ("addons/sourcemod/plugins/rtd.smx", b"SP"),
        ("addons/sourcemod/translations/rtd.phrases.txt", b"phrases"),
        ("addons/sourcemod/scripting/rtd.sp", b"source"),
        ("cfg/sourcemod/rtd.cfg", b"cvars"),
    ]);
    host.add_file(&format!("{MOD}/uploads/rtd.zip"), &archive);

    let (status, body) = body_json(crate::handlers::archive_install::handle(
        &mut host,
        &params("3"),
        br#"{"path":"cstrike/uploads/rtd.zip"}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["files_written"], 4);
    assert_eq!(body["plugins"], serde_json::json!(["rtd"]));

    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx"))
            .is_some()
    );
    assert!(host.file(&format!("{MOD}/cfg/sourcemod/rtd.cfg")).is_some());
    assert!(
        crate::source::manifest::Manifest::parse(host.file(MANIFEST_ABS).expect("manifest"))
            .contains("rtd")
    );
    // The uploaded archive is scratch.
    assert!(host.file(&format!("{MOD}/uploads/rtd.zip")).is_none());
}

#[test]
fn archive_install_conflicts_without_force_and_keeps_the_upload() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    add_plugin(&mut host, "rtd");
    let archive = zip_of(&[("addons/sourcemod/plugins/rtd.smx", b"NEW")]);
    host.add_file(&format!("{MOD}/uploads/rtd.zip"), &archive);

    let (status, body) = body_json(crate::handlers::archive_install::handle(
        &mut host,
        &params("3"),
        br#"{"path":"cstrike/uploads/rtd.zip"}"#,
        None,
    ));
    assert_eq!(status, 409);
    assert_eq!(body["code"], "ALREADY_REGISTERED");
    // Kept, so the frontend can retry with force against the same upload.
    assert!(host.file(&format!("{MOD}/uploads/rtd.zip")).is_some());

    let (status, _) = body_json(crate::handlers::archive_install::handle(
        &mut host,
        &params("3"),
        br#"{"path":"cstrike/uploads/rtd.zip","force":true}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx")),
        Some(b"NEW".as_slice())
    );
}

#[test]
fn archive_install_rejects_escaping_paths_and_wrong_types() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    for path in ["../../etc/cron.zip", "/etc/x.zip", "cstrike/uploads/rtd.rar"] {
        let body = format!(r#"{{"path":"{path}"}}"#);
        let (status, _) = body_json(crate::handlers::archive_install::handle(
            &mut host,
            &params("3"),
            body.as_bytes(),
            None,
        ));
        assert_eq!(status, 400, "{path}");
    }
}

#[test]
fn archive_install_takes_a_tar_gz_too() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);

    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    let data = b"SP";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "addons/sourcemod/plugins/rtd.smx", &data[..])
        .expect("append");
    let archive = builder
        .into_inner()
        .expect("tar")
        .finish()
        .expect("gz");
    host.add_file(&format!("{MOD}/uploads/rtd.tar.gz"), &archive);

    let (status, body) = body_json(crate::handlers::archive_install::handle(
        &mut host,
        &params("3"),
        br#"{"path":"cstrike/uploads/rtd.tar.gz"}"#,
        None,
    ));
    assert_eq!(status, 200);
    assert_eq!(body["plugins"], serde_json::json!(["rtd"]));
}

// ---------------------------------------------------------------- catalog / updates

#[test]
fn catalog_list_hides_entries_for_other_games() {
    let mut host = MockHost::css();
    let (status, body) = body_json(crate::handlers::catalog_routes::handle_list(
        &mut host,
        &params("3"),
    ));
    assert_eq!(status, 200);
    let keys: Vec<&str> = body["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry["key"].as_str())
        .collect();
    assert!(keys.contains(&"rtd"));
    assert!(!keys.contains(&"tf2items"), "tf2-only entry on a CS:S server");
}

#[test]
fn catalog_install_downloads_registers_and_snapshots_first() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);

    host.http_responses.insert(
        "https://api.github.com/repos/Phil25/RTD/releases/latest".into(),
        (
            200,
            br#"{"tag_name":"v2.3.1","html_url":"https://github.com/Phil25/RTD/releases/tag/v2.3.1",
                 "assets":[{"name":"rtd-2.3.1.zip","browser_download_url":"https://example.test/rtd.zip"}]}"#
                .to_vec(),
        ),
    );
    host.http_responses.insert(
        "https://example.test/rtd.zip".into(),
        (200, zip_of(&[("addons/sourcemod/plugins/rtd.smx", b"SP")])),
    );

    let (status, body) = body_json(crate::handlers::catalog_routes::handle_install(
        &mut host,
        &params("3"),
        br#"{"key":"rtd"}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["version"], "2.3.1");
    assert_eq!(body["plugins"], serde_json::json!(["rtd"]));
    assert!(
        host.file(&format!("{MOD}/addons/sourcemod/plugins/rtd.smx"))
            .is_some()
    );
    assert!(
        crate::source::manifest::Manifest::parse(host.file(MANIFEST_ABS).expect("manifest"))
            .contains("rtd")
    );
    // An overwrite is always preceded by a snapshot attempt.
    assert!(
        host.execs.iter().any(|(command, _)| command.starts_with("tar -cf")),
        "auto-snapshot ran before the install"
    );
}

#[test]
fn catalog_install_rejects_an_entry_not_offered_for_this_game() {
    let mut host = MockHost::css();
    with_sourcemod(&mut host);
    let (status, body) = body_json(crate::handlers::catalog_routes::handle_install(
        &mut host,
        &params("3"),
        br#"{"key":"tf2items"}"#,
        None,
    ));
    assert_eq!(status, 404);
    assert_eq!(body["code"], "CATALOG_KEY_UNKNOWN");
}

#[test]
fn updates_caches_and_serves_versions() {
    let mut host = MockHost::css();
    host.http_responses.insert(
        "https://mms.alliedmods.net/mmsdrop/1.12/mmsource-latest-linux".into(),
        (200, b"mmsource-1.12.0-git1219-linux.tar.gz".to_vec()),
    );
    host.http_responses.insert(
        "https://sm.alliedmods.net/smdrop/1.12/sourcemod-latest-linux".into(),
        (200, b"sourcemod-1.12.0-git7210-linux.tar.gz".to_vec()),
    );

    let (status, body) = body_json(crate::handlers::updates::handle(
        &mut host,
        &params("3"),
        &HashMap::new(),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["metamod"]["version"], "1.12.0-git1219");
    assert_eq!(
        body["metamod"]["download_url"],
        "https://mms.alliedmods.net/mmsdrop/1.12/mmsource-1.12.0-git1219-linux.tar.gz"
    );
    assert_eq!(body["sourcemod"]["version"], "1.12.0-git7210");

    // Second call is served from the cache: no new outbound requests.
    let before = host.http_calls.len();
    body_json(crate::handlers::updates::handle(
        &mut host,
        &params("3"),
        &HashMap::new(),
    ));
    assert_eq!(host.http_calls.len(), before);
}

// ---------------------------------------------------------------- platform

#[test]
fn platform_install_sourcemod_downloads_and_untars_on_the_node() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    host.http_responses.insert(
        "https://sm.alliedmods.net/smdrop/1.12/sourcemod-latest-linux".into(),
        (200, b"sourcemod-1.12.0-git7210-linux.tar.gz".to_vec()),
    );
    // The mock exec writes nothing, so stage what the tarball would produce.
    host.exec_results
        .insert("tar -xzf".into(), (0, String::new()));
    host.add_dir(&format!("{MOD}/addons/sourcemod"));
    // curl's output file has to exist and be non-empty for the download check.
    host.add_file("/srv/gameap/servers/css/.srcaddons/sourcemod.tar.gz", b"gz");

    let (status, body) = body_json(crate::handlers::platform::handle(
        &mut host,
        &params("3"),
        br#"{"kind":"sourcemod"}"#,
        Some("erica"),
    ));
    assert_eq!(status, 200);
    assert_eq!(body["version"], "1.12.0-git7210");

    let commands: Vec<&str> = host.execs.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        commands.iter().any(|c| c.contains(
            "curl -fsSL --retry 2 -o /srv/gameap/servers/css/.srcaddons/sourcemod.tar.gz https://sm.alliedmods.net/smdrop/1.12/sourcemod-1.12.0-git7210-linux.tar.gz"
        )),
        "downloads the drop the pointer file names: {commands:?}"
    );
    assert!(
        commands.iter().any(|c| c
            == &format!(
                "tar -xzf /srv/gameap/servers/css/.srcaddons/sourcemod.tar.gz -C {MOD}"
            )),
        "unpacks into the mod dir: {commands:?}"
    );
    // The scratch archive is removed afterwards.
    assert!(
        host.file("/srv/gameap/servers/css/.srcaddons/sourcemod.tar.gz")
            .is_none()
    );
}

#[test]
fn platform_install_refuses_paths_that_would_split_wrong() {
    let mut host = MockHost::css();
    host.servers.get_mut(&3).expect("server").dir = "servers/my css".into();
    host.http_responses.insert(
        "https://mms.alliedmods.net/mmsdrop/1.12/mmsource-latest-linux".into(),
        (200, b"mmsource-1.12.0-git1219-linux.tar.gz".to_vec()),
    );
    host.add_dir("/srv/gameap/servers/my css/cstrike");
    host.add_file("/srv/gameap/servers/my css/cstrike/gameinfo.txt", b"x");

    let (status, body) = body_json(crate::handlers::platform::handle(
        &mut host,
        &params("3"),
        br#"{"kind":"metamod"}"#,
        None,
    ));
    assert_eq!(status, 422);
    assert_eq!(body["code"], "UNSAFE_PATH");
    assert!(host.execs.is_empty(), "nothing runs on the node");
}

#[test]
fn platform_install_rejects_an_unknown_kind() {
    let mut host = MockHost::css();
    let (status, body) = body_json(crate::handlers::platform::handle(
        &mut host,
        &params("3"),
        br#"{"kind":"css"}"#,
        None,
    ));
    assert_eq!(status, 400);
    assert_eq!(body["code"], "INVALID_INPUT");
}

// ---------------------------------------------------------------- doctor

fn checks(body: &Value) -> HashMap<String, (String, String)> {
    body["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .map(|check| {
            (
                check["id"].as_str().unwrap_or_default().to_string(),
                (
                    check["status"].as_str().unwrap_or_default().to_string(),
                    check["detail"].as_str().unwrap_or_default().to_string(),
                ),
            )
        })
        .collect()
}

#[test]
fn doctor_all_green_on_a_healthy_server() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    with_sourcemod(&mut host);
    add_plugin(&mut host, "funcommands");
    host.add_dir(&format!("{MOD}/cfg/sourcemod"));

    let (status, body) = body_json(crate::handlers::doctor::handle(&mut host, &params("3")));
    assert_eq!(status, 200);
    let checks = checks(&body);
    for id in [
        "metamod",
        "loader",
        "loaderbin",
        "sourcemod",
        "smbin",
        "smregistered",
        "duplicates",
        "orphans",
        "cfgdir",
        "scripting",
        "loadfail",
        "vdf",
        "scratch",
    ] {
        assert_eq!(
            checks.get(id).map(|(status, _)| status.as_str()),
            Some("ok"),
            "{id}: {:?}",
            checks.get(id)
        );
    }
}

#[test]
fn doctor_catches_the_classic_sourcemod_failures() {
    let mut host = MockHost::css();
    // Metamod on disk with nothing loading it.
    host.add_dir(&format!("{MOD}/addons/metamod"));
    // SourceMod unpacked but never registered with Metamod, and no bin/.
    host.add_dir(&format!("{MOD}/addons/sourcemod/plugins"));
    add_plugin(&mut host, "rtd");
    add_disabled_plugin(&mut host, "rtd");
    host.add_file(
        &format!("{MOD}/addons/sourcemod/scripting/mything.sp"),
        b"#include <sourcemod>",
    );
    host.add_file(
        &format!("{MOD}/addons/sourcemod/logs/errors_20260819.log"),
        b"L 08/19/2026 - 12:00:01: [SM] Unable to load plugin \"rtd.smx\": Native \"X\" was not found\n",
    );

    let (_, body) = body_json(crate::handlers::doctor::handle(&mut host, &params("3")));
    let checks = checks(&body);

    assert_eq!(checks["loader"].0, "fail");
    assert_eq!(checks["smbin"].0, "fail");
    assert_eq!(checks["smregistered"].0, "fail");
    assert_eq!(checks["duplicates"].0, "fail");
    assert!(checks["duplicates"].1.contains("rtd"));
    assert_eq!(checks["cfgdir"].0, "warn");
    assert_eq!(checks["scripting"].0, "warn");
    assert!(checks["scripting"].1.contains("mything"));
    assert_eq!(checks["loadfail"].0, "fail");
    assert!(checks["loadfail"].1.contains("rtd"));
}

#[test]
fn doctor_flags_a_loader_vdf_pointing_at_a_missing_binary() {
    let mut host = MockHost::css();
    host.add_dir(&format!("{MOD}/addons/metamod"));
    // A 32-bit vdf against a 64-bit-only install: Metamod silently never starts.
    host.add_file(
        &format!("{MOD}/addons/metamod.vdf"),
        b"\"Metamod Plugin\"\n{\n\t\"file\"\t\"../cstrike/addons/metamod/bin/server\"\n}\n",
    );
    host.add_file(
        &format!("{MOD}/addons/metamod/bin/linux64/server.so"),
        b"\x7fELF",
    );

    let (_, body) = body_json(crate::handlers::doctor::handle(&mut host, &params("3")));
    let checks = checks(&body);
    assert_eq!(checks["loader"].0, "ok", "a vdf is present");
    assert_eq!(checks["loaderbin"].0, "fail");
    assert!(checks["loaderbin"].1.contains("addons/metamod/bin/server"));
}

#[test]
fn doctor_flags_metaplugins_entries_whose_binary_is_gone() {
    let mut host = MockHost::css();
    with_metamod(&mut host);
    host.add_file(
        &format!("{MOD}/addons/metamod/metaplugins.ini"),
        b"addons/stripper/bin/stripper_mm_i486.so\n; addons/gone/gone.so\n",
    );

    let (_, body) = body_json(crate::handlers::doctor::handle(&mut host, &params("3")));
    let checks = checks(&body);
    assert_eq!(checks["metaplugins"].0, "fail");
    assert!(checks["metaplugins"].1.contains("stripper"));
    assert!(
        !checks["metaplugins"].1.contains("gone"),
        "a commented-out line is not loaded, so it cannot be broken"
    );
}

// ------------------------------------------- archive entry size guard

/// An entry at exactly the panel's inline limit, and one byte over it.
fn sized_entry(path: &str, len: u64) -> crate::source::archive::ArchiveEntry {
    crate::source::archive::ArchiveEntry {
        path: path.to_string(),
        data: vec![0u8; len as usize],
        mode: 0o644,
    }
}

#[test]
fn an_entry_over_the_panel_inline_limit_is_refused_before_anything_is_written() {
    // Extraction bounds an archive's TOTAL uncompressed size at twice the
    // panel's per-call upload limit and bounds no single member, so one
    // oversized file is reachable from an archive that is itself acceptable.
    // Uploading entry by entry, the panel would refuse that one partway
    // through and leave a half-installed plugin behind; the guard turns it
    // into a refusal with nothing written.
    let mut host = MockHost::css();
    
    let ctx = crate::handlers::ctx::ServerCtx::resolve(&mut host, &params("3")).expect("ctx");
    let before = host.files.len();

    let entries = vec![
        sized_entry("ok.txt", 8),
        sized_entry("huge.bin", crate::handlers::PANEL_MAX_INLINE_BYTES + 1),
    ];

    let err = crate::handlers::write_archive_entries(&mut host, &ctx, &entries, &crate::source::archive::InstallRoot::ModDir)
        .expect_err("an entry over the inline limit must be refused");

    assert_eq!(err.status, 422);
    // Not even the small entry ahead of it — the check runs before the loop.
    assert_eq!(host.files.len(), before, "nothing may be written");
}

#[test]
fn an_entry_exactly_at_the_panel_inline_limit_is_written() {
    // The panel's own comparison is strict — its tests pin 4096 bytes against
    // a 4096 cap as an accepted upload — so the boundary value has to pass
    // here too, or the two disagree at exactly one size and the guard refuses
    // a file the panel would have taken.
    let mut host = MockHost::css();
    
    let ctx = crate::handlers::ctx::ServerCtx::resolve(&mut host, &params("3")).expect("ctx");

    let entries = vec![sized_entry("exact.bin", crate::handlers::PANEL_MAX_INLINE_BYTES)];

    let written = crate::handlers::write_archive_entries(&mut host, &ctx, &entries, &crate::source::archive::InstallRoot::ModDir)
        .expect("the boundary value is not over the limit");

    assert_eq!(written, 1);
}
