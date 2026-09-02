//! GameAP plugin: manage Metamod:Source and SourceMod on Source 1 servers.
//!
//! Backend of the "Plugins" server tab: assembles the Metamod:Source and
//! SourceMod picture of a Source engine server (Counter-Strike: Source,
//! Garry's Mod, Half-Life 2: Deathmatch, Team Fortress 2, Left 4 Dead 2, Day
//! of Defeat: Source, …) and performs plugin file moves, metaplugins.ini edits
//! and plugins_meta.json mutations through the nodefs/nodecmd host libraries.
//! The Vue frontend (embedded bundle) talks to these routes and to existing
//! panel endpoints (RCON for live statuses, file-manager for uploads and
//! configs).
//!
//! A Source 1 sibling of plugin-cs2-addons, itself a re-imagining of
//! gameap/plugin-goldsrc-addons.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod consistency;
pub mod handlers;
pub mod host_api;
pub mod http;
pub mod maintenance;
pub mod model;
pub mod router;
pub mod source;
pub mod wasm_ext;

use gameap_plugin_sdk::proto::gameap::plugin as pb;
use gameap_plugin_sdk::{Plugin, PluginError, register_plugin};

use crate::host_api::HostApi;

// The panel normalizes plugin ids (CompactPluginID): the id must decode as
// base32 (alphabet a-z2-7, no padding — so 2/4/5/7/8/10/12/13 chars) and
// re-encode to itself, or it is rewritten to an FNV hash, which breaks route
// paths and the plugin:<id>:manage ability the tab is gated on.
// "onzggylemrxw4" is base32("srcaddon") and is round-trip stable.
pub const PLUGIN_ID: &str = "onzggylemrxw4";

const FRONTEND_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plugin.js"));
const FRONTEND_CSS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plugin.css"));

pub struct SourceAddons<H> {
    host: H,
}

impl<H> SourceAddons<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: HostApi> Plugin for SourceAddons<H> {
    fn get_info(&mut self, _req: pb::GetInfoRequest) -> Result<pb::PluginInfo, PluginError> {
        Ok(pb::PluginInfo {
            id: PLUGIN_ID.into(),
            name: "Source Addons".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description:
                "Manage Metamod:Source and SourceMod plugins on Source engine servers".into(),
            author: "SilverSasquatchGameAPDev".into(),
            license: "MIT".into(),
            api_version: "1".into(),
            ..Default::default()
        })
    }

    fn initialize(
        &mut self,
        _req: pb::InitializeRequest,
    ) -> Result<pb::InitializeResponse, PluginError> {
        self.host.log_info("source-addons plugin initialized");
        #[cfg(target_arch = "wasm32")]
        crate::wasm_ext::register_scheduled_tasks();
        Ok(pb::InitializeResponse {
            result: Some(gameap_plugin_sdk::ok_result()),
        })
    }

    fn get_http_routes(
        &mut self,
        _req: pb::GetHttpRoutesRequest,
    ) -> Result<pb::GetHttpRoutesResponse, PluginError> {
        Ok(pb::GetHttpRoutesResponse {
            routes: router::http_routes(),
        })
    }

    fn handle_http_request(
        &mut self,
        req: pb::HttpRequest,
    ) -> Result<pb::HttpResponse, PluginError> {
        // Total dispatch: every failure becomes a JSON error response. An Err
        // here would surface as a plain-text host 500.
        Ok(router::dispatch(&mut self.host, &req))
    }

    fn get_server_abilities(
        &mut self,
        _req: pb::GetServerAbilitiesRequest,
    ) -> Result<pb::GetServerAbilitiesResponse, PluginError> {
        // Admins get plugin abilities automatically; the frontend tab is
        // gated on plugin:onzggylemrxw4:manage.
        Ok(pb::GetServerAbilitiesResponse {
            abilities: vec![pb::ServerAbility {
                name: "manage".into(),
                title: "Manage Source addons (Metamod:Source / SourceMod)".into(),
            }],
        })
    }

    fn get_frontend_bundle(
        &mut self,
        _req: pb::GetFrontendBundleRequest,
    ) -> Result<pb::GetFrontendBundleResponse, PluginError> {
        Ok(pb::GetFrontendBundleResponse {
            bundle: FRONTEND_JS.to_vec(),
            has_bundle: !FRONTEND_JS.is_empty(),
            styles: FRONTEND_CSS.to_vec(),
            has_styles: !FRONTEND_CSS.is_empty(),
        })
    }
}

register_plugin!(
    SourceAddons<host_api::WasmHost>,
    SourceAddons::new(host_api::WasmHost)
);

