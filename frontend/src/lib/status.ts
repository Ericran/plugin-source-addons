// Row status decision logic, kept pure so it can be unit-tested.

import type { RowStatus, RuntimePluginInfo } from '../types';

export interface RowStatusInput {
    enabled: boolean;
    missing: boolean;
    runtime: RuntimePluginInfo | null;
    rconOk: boolean;
}

export interface RowStatusResult {
    status: RowStatus;
    detail: string | null;
}

/**
 * Decides the row status from the file state and the runtime
 * (`sm plugins list`) state.
 *
 * Paused is SourceMod's own runtime state (`sm plugins pause`): the plugin is
 * loaded and resumable without a restart, so it is neither "running" nor
 * "awaiting load" — it has its own badge and its own action.
 */
export function computeRowStatus({
    enabled,
    missing,
    runtime,
    rconOk,
}: RowStatusInput): RowStatusResult {
    if (missing) {
        return { status: 'missing', detail: null };
    }
    if (runtime?.status === 'error') {
        return { status: 'error', detail: runtime.rawStatus };
    }
    if (!rconOk) {
        // Without console access there is no runtime to compare against.
        return { status: enabled ? 'enabled' : 'stopped', detail: null };
    }
    if (runtime?.status === 'paused') {
        return { status: 'paused', detail: runtime.rawStatus };
    }
    if (enabled && runtime?.status === 'running') {
        return { status: 'running', detail: null };
    }
    if (enabled && runtime === null) {
        // On disk but not loaded yet.
        return { status: 'pending', detail: null };
    }
    if (!enabled && runtime !== null) {
        // Disabled on disk but still in memory.
        return { status: 'pending', detail: null };
    }
    return { status: enabled ? 'enabled' : 'stopped', detail: null };
}

export function isPendingRow(status: RowStatus): boolean {
    return status === 'pending';
}

/**
 * Hot load/unload is a runtime action (`sm plugins load/unload` over RCON),
 * available only against live console state.
 */
export function hotActionForStatus(status: RowStatus, enabled: boolean): 'unload' | 'load' | null {
    if (status === 'running' || status === 'paused') return 'unload';
    if ((status === 'pending' || status === 'stopped') && enabled) return 'load';
    return null;
}

/**
 * The secondary action: reload a running plugin (the one-click way to apply a
 * config change), or resume a paused one.
 */
export function secondaryActionForStatus(status: RowStatus): 'reload' | 'unpause' | null {
    if (status === 'running') return 'reload';
    if (status === 'paused') return 'unpause';
    return null;
}
