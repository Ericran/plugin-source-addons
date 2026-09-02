//! GET /servers/{id}/logs — tail of the newest SourceMod log.
//!
//! SourceMod writes two series into `addons/sourcemod/logs/`: `L<date>.log`
//! (everything plugins log) and `errors_<date>.log` (load failures and runtime
//! errors). `?kind=general` selects the former; the default is the error log,
//! because that is where a plugin explains why it will not run.
//!
//! The tail reader and the load-failure scanner are shared with the doctor
//! route, which reports plugins that failed to load.

use std::collections::HashMap;

use gameap_plugin_sdk::proto::gameap::plugin as pb;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::{HostApi, HostApiError};
use crate::http::{ApiResult, json_response};
use crate::model::LogsResponse;
use crate::source::{self, paths};

/// The whole log is downloaded and only its tail is kept: the cap below guards
/// the wasm heap, not the transfer.
///
/// That used to be the only thing worth saying. Since GameAP 4.5 it is also a
/// ceiling on the feature: a whole-file nodefs download over
/// PLUGINS_NODEFS_MAX_INLINE (32 MiB by default) is refused outright rather than
/// truncated, so a server whose newest log has grown past that loses this route
/// - and the doctor route with it, which shares the reader - both with the
/// panel's "file too large" error.
///
/// 4.5 also added the fix: `offset`/`length` on the nodefs DownloadRequest,
/// which would let this read only the tail it actually wants. The plugin cannot
/// reach it yet - those fields exist in the panel's own proto, while the
/// gameap-proto commit this crate pins (still the tip of its main branch)
/// declares DownloadRequest as node_id + path alone. When the SDK catches up,
/// this becomes a windowed read of the last MAX_TAIL_BYTES and the ceiling goes
/// away.
const MAX_LINES: usize = 400;
const MAX_TAIL_BYTES: usize = 256 * 1024;

const ERRORS_PREFIX: &str = "errors_";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogKind {
    Errors,
    General,
}

impl LogKind {
    fn as_str(self) -> &'static str {
        match self {
            LogKind::Errors => "errors",
            LogKind::General => "general",
        }
    }
}

pub(crate) struct LogTail {
    /// Server-relative path of the file the lines came from.
    pub file_rel: String,
    pub lines: Vec<String>,
}

/// Newest log of the requested series, tail-read and split into lines. `None`
/// when the server has no log directory or no matching file yet.
pub(crate) fn newest_log_tail<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    kind: LogKind,
) -> Result<Option<LogTail>, HostApiError> {
    let logs_abs = paths::join(&ctx.mod_abs, source::SM_LOGS_DIR);
    let Some(entries) = host.read_dir(ctx.node_id, &logs_abs)? else {
        return Ok(None);
    };

    // Both series are named with a YYYYMMDD date, so within one series
    // lexicographic order is date order. The two series must not be compared
    // against each other, hence the prefix filter.
    let newest = entries
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".log")
                && (lower.starts_with(ERRORS_PREFIX) == (kind == LogKind::Errors))
        })
        .max();
    let Some(file_name) = newest else {
        return Ok(None);
    };

    let file_abs = paths::join(&logs_abs, &file_name);
    let bytes = host.download(ctx.node_id, &file_abs)?;
    let tail_start = bytes.len().saturating_sub(MAX_TAIL_BYTES);
    let text = String::from_utf8_lossy(&bytes[tail_start..]);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    // A mid-line cut leaves a garbage first line; drop it when we truncated.
    if tail_start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let skip = lines.len().saturating_sub(MAX_LINES);
    let lines = lines.split_off(skip);

    Ok(Some(LogTail {
        file_rel: ctx.rel(&paths::join(source::SM_LOGS_DIR, &file_name)),
        lines,
    }))
}

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    query: &HashMap<String, pb::QueryParamValues>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let kind = match query
        .get("kind")
        .and_then(|values| values.values.first())
        .map(String::as_str)
    {
        Some("general") => LogKind::General,
        _ => LogKind::Errors,
    };

    let Some(tail) = newest_log_tail(host, &ctx, kind)? else {
        return Ok(json_response(
            200,
            &LogsResponse {
                file: None,
                kind: kind.as_str().into(),
                lines: Vec::new(),
            },
        ));
    };

    Ok(json_response(
        200,
        &LogsResponse {
            file: Some(tail.file_rel),
            kind: kind.as_str().into(),
            lines: tail.lines,
        },
    ))
}

/// A plugin SourceMod refused to load, as recorded in the error log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadFailure {
    /// Plugin file stem, taken from the .smx the log names.
    pub plugin: String,
    /// What the log gives as the reason, when it names one.
    pub reason: Option<String>,
}

const LOAD_FAILURE_MARKERS: &[&str] = &["Unable to load plugin", "Failed to load plugin"];

/// Scans log lines for plugin load failures, newest state per plugin: a plugin
/// that failed and later loaded fine still reports here, so callers should
/// present this as log history rather than live state.
pub(crate) fn find_load_failures(lines: &[String]) -> Vec<LoadFailure> {
    let mut failures: Vec<LoadFailure> = Vec::new();
    for line in lines {
        if !LOAD_FAILURE_MARKERS
            .iter()
            .any(|marker| line.contains(marker))
        {
            continue;
        }
        let Some((plugin, reason)) = parse_failure_line(line) else {
            continue;
        };
        // One entry per plugin — a later failure supersedes an earlier one.
        failures.retain(|existing| !existing.plugin.eq_ignore_ascii_case(&plugin));
        failures.push(LoadFailure { plugin, reason });
    }
    failures
}

/// `L 08/19/2026 - 12:00:01: [SM] Unable to load plugin "rtd.smx": Native
/// "SteamWorks_GetUserFlags" was not found` → ("rtd", the text after the colon).
fn parse_failure_line(line: &str) -> Option<(String, Option<String>)> {
    // The plugin is the quoted token ending in .smx; the reason is whatever
    // follows the closing quote (SourceMod separates it with ": ").
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    let quoted = &rest[..end];
    if !quoted.to_ascii_lowercase().ends_with(source::SMX_EXT) {
        return None;
    }
    let stem = paths::file_stem(paths::file_name(quoted));
    if stem.is_empty() {
        return None;
    }
    let reason = rest[end + 1..]
        .trim_start_matches([':', ' ', '\t'])
        .trim()
        .to_string();
    Some((stem.to_string(), (!reason.is_empty()).then_some(reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn finds_failure_and_reason() {
        let log = lines(&[
            "L 08/19/2026 - 12:00:00: SourceMod log file session started",
            "L 08/19/2026 - 12:00:01: [SM] Unable to load plugin \"rtd.smx\": Native \"SteamWorks_GetUserFlags\" was not found",
            "L 08/19/2026 - 12:00:02: [SM] Plugin funcommands.smx loaded",
        ]);
        let found = find_load_failures(&log);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].plugin, "rtd");
        assert_eq!(
            found[0].reason.as_deref(),
            Some("Native \"SteamWorks_GetUserFlags\" was not found")
        );
    }

    #[test]
    fn failure_without_a_stated_reason() {
        let log = lines(&["L 08/19/2026 - 12:00:01: [SM] Unable to load plugin \"broken.smx\""]);
        let found = find_load_failures(&log);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].plugin, "broken");
        assert_eq!(found[0].reason, None);
    }

    #[test]
    fn one_entry_per_plugin_and_none_when_clean() {
        let log = lines(&[
            "[SM] Unable to load plugin \"same.smx\": first reason",
            "[SM] Unable to load plugin \"same.smx\": second reason",
        ]);
        let found = find_load_failures(&log);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].reason.as_deref(), Some("second reason"));

        assert!(find_load_failures(&lines(&["[SM] All plugins loaded"])).is_empty());
    }

    #[test]
    fn ignores_a_marker_without_a_plugin_file() {
        // A message that merely mentions loading must not become a failure,
        // and the quote scan must not run off the end of the line.
        assert!(find_load_failures(&lines(&["[SM] Unable to load plugin"])).is_empty());
        assert!(
            find_load_failures(&lines(&["[SM] Unable to load plugin \"a config file\""]))
                .is_empty()
        );
    }
}
