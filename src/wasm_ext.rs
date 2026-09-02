//! Hand-rolled wasm exports for the one panel service the Rust SDK does not
//! wrap yet: the ScheduledTaskHandler behind the Metamod loader sweep and the
//! nightly update check, plus the gameap-scheduler host import it needs.
//!
//! Export names and the ABI envelope mirror what `register_plugin!` emits for
//! the core service; the panel discovers each optional service by probing for
//! its `<service>_api_version` export.
//!
//! Note what is NOT here: a ProtocolService. Source 1 speaks the classic
//! Source RCON protocol that the panel's built-in client already implements
//! correctly — the tolerant replacement its Source 2 sibling ships exists only
//! because CS2 breaks that protocol's assumptions.

#![cfg(target_arch = "wasm32")]

use gameap_plugin_sdk::abi::{guest_call, host_call};
use gameap_plugin_sdk::proto::gameap::plugin::sdk::scheduler;
use gameap_plugin_sdk::{HostError, host};

use crate::host_api::WasmHost;

const LOADER_SWEEP_TASK: &str = "srcaddons-metamod-loader-sweep";
const UPDATE_CHECK_TASK: &str = "srcaddons-update-check";
const LOADER_SWEEP_INTERVAL_MS: i64 = 6 * 60 * 60 * 1000;
const UPDATE_CHECK_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;
const TASK_TIMEOUT_MS: i64 = 120 * 1000;

mod sys_scheduler {
    #[link(wasm_import_module = "gameap-scheduler")]
    unsafe extern "C" {
        pub fn add_task(ptr: u32, size: u32) -> u64;
    }
}

fn scheduler_add_task(
    req: &scheduler::AddTaskRequest,
) -> Result<scheduler::AddTaskResponse, HostError> {
    unsafe { host_call(sys_scheduler::add_task, req) }
}

/// Registers the recurring tasks; called from `initialize`. Failures are
/// logged, never fatal — the tab works fine without the background sweeps.
pub fn register_scheduled_tasks() {
    for (name, interval_ms) in [
        (LOADER_SWEEP_TASK, LOADER_SWEEP_INTERVAL_MS),
        (UPDATE_CHECK_TASK, UPDATE_CHECK_INTERVAL_MS),
    ] {
        let result = scheduler_add_task(&scheduler::AddTaskRequest {
            name: name.to_string(),
            interval_ms,
            error_policy: None,
            timeout_ms: TASK_TIMEOUT_MS,
        });
        match result {
            Ok(resp) if resp.success => host::log::info(format!("registered task {name}")),
            Ok(resp) => host::log::error(format!(
                "task {name} not registered: {}",
                resp.error.unwrap_or_default()
            )),
            Err(err) => host::log::error(format!("task {name} registration failed: {err}")),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn scheduled_task_handler_api_version() -> u64 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn scheduled_task_handler_handle_scheduled_task(ptr: u32, size: u32) -> u64 {
    guest_call(ptr, size, |req: scheduler::HandleScheduledTaskRequest| {
        let mut host_api = WasmHost;
        match req.task_name.as_str() {
            LOADER_SWEEP_TASK => {
                let stats = crate::maintenance::loader_sweep(&mut host_api);
                host::log::info(format!(
                    "metamod loader sweep: {} checked, {} repaired, {} failed",
                    stats.checked, stats.repaired, stats.failed
                ));
            }
            UPDATE_CHECK_TASK => match crate::handlers::updates::refresh_cache(&mut host_api) {
                Ok(_) => host::log::info("update check refreshed"),
                Err(err) => host::log::error(format!("update check failed: {err}")),
            },
            other => host::log::error(format!("unknown scheduled task {other}")),
        }
        Ok(scheduler::HandleScheduledTaskResponse {})
    })
}
