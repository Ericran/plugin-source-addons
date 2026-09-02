<template>
    <div>
        <!-- Fallback guard: the tab itself is game-gated via checkGame -->
        <n-card v-if="!isSourceOne" size="small">
            <div class="py-10">
                <n-empty :description="trans('not_source1')" size="small" />
            </div>
        </n-card>

        <template v-else>
            <Loading v-if="loading && !state" />

            <n-card v-else-if="loadError" size="small">
                <div class="py-8 text-center">
                    <div class="text-sm text-red-500 dark:text-red-400 mb-3">{{ loadError }}</div>
                    <GButton color="white" size="small" @click="refreshAll">
                        <GIcon name="refresh" /><span class="ml-1">{{ trans('retry') }}</span>
                    </GButton>
                </div>
            </n-card>

            <template v-else-if="state">
                <!-- rcon hint (left, height always reserved) + management toolbar -->
                <div class="mb-1 flex flex-wrap items-center gap-2">
                    <div
                        class="flex items-center gap-2 text-xs text-stone-400 dark:text-stone-500 min-w-0"
                        :style="{ visibility: rconHint ? 'visible' : 'hidden' }"
                    >
                        <GIcon name="info" size="sm" />
                        <span class="truncate">{{ rconHint || ' ' }}</span>
                    </div>
                    <div class="ml-auto flex flex-wrap items-center gap-1.5">
                        <GButton
                            v-if="updatableCatalog.length"
                            color="orange"
                            size="small"
                            :disabled="mutating"
                            @click="onUpdateAll"
                        >
                            <i class="fa-solid fa-arrow-up-from-bracket"></i>
                            <span class="ml-1">{{ trans('update_all', { count: updatableCatalog.length }) }}</span>
                        </GButton>
                        <!-- These buttons render icon-only on small screens, so
                             each carries its label as a hover tooltip. Same
                             label text, so en/ru stay in step. -->
                        <n-tooltip trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="catalogOpen = true">
                                    <i class="fa-solid fa-shapes"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_catalog') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_catalog') }}
                        </n-tooltip>
                        <n-tooltip trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="snapshotsOpen = true">
                                    <i class="fa-solid fa-box-archive"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_snapshots') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_snapshots') }}
                        </n-tooltip>
                        <n-tooltip v-if="state.sourcemod.installed" trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="adminsOpen = true">
                                    <i class="fa-solid fa-user-shield"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_admins') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_admins') }}
                        </n-tooltip>
                        <n-tooltip v-if="state.sourcemod.installed" trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="openLogs()">
                                    <i class="fa-solid fa-file-waveform"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_logs') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_logs') }}
                        </n-tooltip>
                        <n-tooltip trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="auditOpen = true">
                                    <i class="fa-solid fa-clock-rotate-left"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_history') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_history') }}
                        </n-tooltip>
                        <n-tooltip trigger="hover">
                            <template #trigger>
                                <GButton color="white" size="small" @click="doctorOpen = true">
                                    <i class="fa-solid fa-stethoscope"></i>
                                    <span class="ml-1 hidden sm:inline">{{ trans('toolbar_doctor') }}</span>
                                </GButton>
                            </template>
                            {{ trans('toolbar_doctor') }}
                        </n-tooltip>
                    </div>
                </div>

                <!-- pending changes want a restart -->
                <div
                    v-if="restartSuggested"
                    class="mb-2 flex items-center gap-3 px-3 py-2 rounded border border-orange-200 dark:border-orange-900 bg-orange-50 dark:bg-orange-950 text-sm text-orange-800 dark:text-orange-200"
                >
                    <i class="fa-solid fa-rotate"></i>
                    <span class="min-w-0 flex-1">{{ trans('restart_pending') }}</span>
                    <GButton color="white" size="small" :disabled="mutating" @click="onRestart">
                        <i class="fa-solid fa-power-off"></i>
                        <span class="ml-1">{{ trans('restart_now') }}</span>
                    </GButton>
                </div>

                <!-- platform status cards; only SourceMod carries the list -->
                <div class="platform-cards grid md:grid-cols-2 gap-3 mb-3">
                    <PlatformCard
                        kind="metamod"
                        :state="state"
                        :version="metaVersion"
                        :rows="[]"
                        :update-version="metamodLatest"
                        :busy="platformBusy"
                        @install="onPlatformInstall('metamod')"
                        @repair="onRepair"
                        @toggle-mm="onToggleMetamodPlugin"
                    />
                    <PlatformCard
                        kind="sourcemod"
                        :state="state"
                        :version="smVersion"
                        :rows="rows"
                        :update-version="sourcemodLatest"
                        :busy="platformBusy"
                        active
                        @install="onPlatformInstall('sourcemod')"
                    />
                </div>

                <!-- plugin list -->
                <n-card size="small" class="plugins-panel">
                    <template v-if="nothingInstalled">
                        <div class="py-12 text-center">
                            <i class="fa-solid fa-puzzle-piece fa-2x text-stone-300 dark:text-stone-600"></i>
                            <div class="mt-3 font-medium text-stone-700 dark:text-stone-200">
                                {{ trans('nothing_installed_title') }}
                            </div>
                            <div class="mt-1 text-sm text-stone-500 dark:text-stone-400 max-w-md mx-auto">
                                {{ trans('nothing_installed_text') }}
                            </div>
                        </div>
                    </template>

                    <template v-else>
                        <PluginList
                            :rows="rows"
                            :installed="state.sourcemod.installed"
                            :plugins-path="state.paths.sm_plugins_dir"
                            :busy="mutating"
                            :updates="updatesByPlugin"
                            @toggle="onToggle"
                            @hot-action="onHotAction"
                            @set-comment="onSetComment"
                            @remove="onDelete"
                            @bulk="onBulk"
                            @install="installOpen = true"
                            @configure="openConfig"
                            @open-files="openFileManager"
                        />
                    </template>
                </n-card>

                <InstallModal
                    v-model:show="installOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                    :paths="state.paths"
                    :existing-names="rows.map((row) => row.name)"
                    @installed="onInstalled"
                />
                <ConfigModal
                    v-model:show="configOpen"
                    :server-id="serverId"
                    :row="configRow"
                />
                <CatalogModal
                    v-model:show="catalogOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                    :installed-plugins="rows.map((row) => row.name)"
                    @installed="onCatalogInstalled"
                />
                <SnapshotsModal
                    v-model:show="snapshotsOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                    @restored="onSnapshotRestored"
                />
                <AdminsModal
                    v-model:show="adminsOpen"
                    :server-id="serverId"
                    :configs-dir="state.paths.sm_configs_dir"
                />
                <LogsModal
                    v-model:show="logsOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                    :initial-filter="logsFilter"
                />
                <AuditModal
                    v-model:show="auditOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                />
                <DoctorModal
                    v-model:show="doctorOpen"
                    :server-id="serverId"
                    :plugin-id="pluginId"
                    :frontend-checks="frontendChecks"
                />
            </template>
        </template>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { NCard, NEmpty, NTooltip } from 'naive-ui';
import { providePluginTrans } from '@gameap/plugin-sdk';

import AdminsModal from './AdminsModal.vue';
import AuditModal from './AuditModal.vue';
import CatalogModal from './CatalogModal.vue';
import ConfigModal from './ConfigModal.vue';
import DoctorModal from './DoctorModal.vue';
import InstallModal from './InstallModal.vue';
import LogsModal from './LogsModal.vue';
import PlatformCard from './PlatformCard.vue';
import PluginList from './PluginList.vue';
import SnapshotsModal from './SnapshotsModal.vue';
import { RconError, rcon, smPluginInfo, smPluginsCommand } from '../api/gameap';
import {
    apiErrorMessage,
    deletePlugin,
    getLogs,
    getState,
    getUpdates,
    installCatalogEntry,
    installPlatform,
    repairMetamod,
    restartServer,
    setAttributes,
    toggleMetamodPlugin,
    togglePlugin,
} from '../api/plugin';
import {
    applyResolvedFile,
    isSmActionSuccess,
    isUnknownCommandOutput,
    matchRuntimeToFiles,
    parseMetaList,
    parseMetaVersion,
    parseSmPluginFile,
    parseSmPlugins,
    parseSmVersion,
    smVersionFromMetaList,
} from '../lib/rcon-parse';
import { prettyName } from '../lib/naming';
import { computeRowStatus } from '../lib/status';
import { versionsMatch } from '../lib/version';
import type {
    DoctorCheck,
    MetamodPluginEntry,
    PlatformVersion,
    PluginRow,
    PluginUpdateInfo,
    RuntimePluginInfo,
    ServerTabProps,
    StateResponse,
    UpdatesResponse,
} from '../types';

const props = defineProps<ServerTabProps>();
const { trans } = providePluginTrans(props.pluginId);

const state = ref<StateResponse | null>(null);
const loading = ref(false);
const loadError = ref<string | null>(null);
const mutating = ref(false);

type RconAvailability =
    | 'unknown'
    | 'ok'
    | 'offline'
    | 'no-rcon'
    | 'bad-password'
    | 'empty'
    | 'no-metamod'
    | 'no-sourcemod'
    | 'error';
const rconAvailability = ref<RconAvailability>('unknown');
// Backend explanation for the generic 'error' reason, shown next to the hint.
const rconErrorDetail = ref<string | null>(null);
const metaVersion = ref<PlatformVersion | null>(null);
const smVersion = ref<PlatformVersion | null>(null);
const smRuntime = ref<RuntimePluginInfo[]>([]);
/** Title→file answers from `sm plugins info`, so each is asked for once. */
const resolvedFiles = ref<Record<string, string>>({});

const installOpen = ref(false);
const configOpen = ref(false);
const configRow = ref<PluginRow | null>(null);
const catalogOpen = ref(false);
const snapshotsOpen = ref(false);
const adminsOpen = ref(false);
const logsOpen = ref(false);
const logsFilter = ref('');
const auditOpen = ref(false);
const doctorOpen = ref(false);

const updatesData = ref<UpdatesResponse | null>(null);
const platformBusy = ref(false);
/** A change happened since the last restart — it waits for one. */
const restartDirty = ref(false);

const serverGame = computed(() => {
    return (
        props.server as unknown as
            | { game?: { engine?: string; engine_version?: string } }
            | undefined
    )?.game;
});
const isSourceOne = computed(() => {
    const engine = serverGame.value?.engine;
    // While the server object is still loading, trust the tab-level checkGame gate.
    if (!engine) {
        return true;
    }
    if (engine.toLowerCase() !== 'source') {
        return false;
    }
    // Source 2 (CS2) is the CS2 Addons plugin's territory.
    return !(serverGame.value?.engine_version ?? '').trim().startsWith('2');
});

const serverOnline = computed(() => Boolean(props.server?.process_active));

const nothingInstalled = computed(() => {
    if (!state.value) {
        return false;
    }
    const metamodPresent = state.value.metamod.installed || state.value.metamod.dir_present;
    return !metamodPresent && !state.value.sourcemod.installed;
});

/** The launch command has no rcon_password, so srcds opens no RCON listener at
 * all — a connect-level failure is then a configuration problem with a known
 * fix rather than a mystery. */
const rconPasswordMissing = computed(() => {
    const command = props.server?.start_command ?? '';
    return command.trim() !== '' && !/rcon_password/i.test(command);
});

const rconHint = computed(() => {
    switch (rconAvailability.value) {
        case 'offline':
            return trans('rcon_unavailable_offline');
        case 'no-rcon':
            return trans('rcon_unavailable_norcon');
        case 'bad-password':
            return trans('rcon_unavailable_badpass');
        case 'empty':
            return trans('rcon_unavailable_empty');
        case 'no-metamod':
            return trans('rcon_metamod_not_loaded');
        case 'no-sourcemod':
            return trans('rcon_sm_not_loaded');
        case 'error': {
            if (rconPasswordMissing.value) {
                return trans('rcon_password_missing');
            }
            return rconErrorDetail.value
                ? `${trans('rcon_unavailable_error')} (${rconErrorDetail.value})`
                : trans('rcon_unavailable_error');
        }
        default:
            return null;
    }
});

const rconOk = computed(() => rconAvailability.value === 'ok');

const metamodLatest = computed(() => updatesData.value?.metamod?.version ?? null);
const sourcemodLatest = computed(() => updatesData.value?.sourcemod?.version ?? null);

const updatesByPlugin = computed<Record<string, PluginUpdateInfo>>(() => {
    const map: Record<string, PluginUpdateInfo> = {};
    for (const info of updatesData.value?.plugins ?? []) {
        if (info.plugin) {
            map[info.plugin] = info;
        }
    }
    return map;
});

/** Catalog plugins whose installed runtime version trails the latest release. */
const updatableCatalog = computed<PluginUpdateInfo[]>(() =>
    (updatesData.value?.plugins ?? []).filter((info) => {
        const row = rows.value.find((item) => item.name === info.plugin);
        if (!row?.version) {
            return false;
        }
        return !versionsMatch(row.version, info.version);
    }),
);

/** Checks only the frontend can make; prepended to the backend doctor list. */
const frontendChecks = computed<DoctorCheck[]>(() => {
    const checks: DoctorCheck[] = [];
    const command = props.server?.start_command ?? '';
    if (command.trim() !== '') {
        checks.push(
            rconPasswordMissing.value
                ? { id: 'rconpass', status: 'warn', detail: trans('doctor_rconpass_missing') }
                : { id: 'rconpass', status: 'ok', detail: trans('doctor_rconpass_ok') },
        );
    }
    if (!serverOnline.value) {
        checks.push({ id: 'rcon', status: 'warn', detail: trans('rcon_unavailable_offline') });
    } else if (rconOk.value) {
        checks.push({ id: 'rcon', status: 'ok', detail: trans('doctor_rcon_ok') });
    } else if (rconAvailability.value !== 'unknown') {
        checks.push({ id: 'rcon', status: 'fail', detail: rconHint.value ?? '' });
    }
    // Installed on disk but absent from the running server — the state the
    // plugin table cannot show, because every row falls back to file state.
    if (state.value?.sourcemod.installed && serverOnline.value) {
        if (rconAvailability.value === 'no-sourcemod') {
            checks.push({ id: 'smloaded', status: 'fail', detail: trans('doctor_sm_not_loaded') });
        } else if (rconOk.value) {
            checks.push({ id: 'smloaded', status: 'ok', detail: trans('doctor_sm_loaded_ok') });
        }
    }
    return checks;
});

/**
 * A restart applies pending work: a change made here, or a plugin switched off
 * on disk that is still loaded in the running server.
 *
 * Deliberately NOT "any row awaiting load". A plugin can sit enabled-on-disk
 * and unloaded indefinitely — it failed to load, or the console never named a
 * file we could pair it with — and the banner would then claim changes were
 * waiting when nothing had changed and no restart would help.
 */
const restartSuggested = computed(
    () =>
        serverOnline.value &&
        (restartDirty.value || rows.value.some((row) => !row.enabled && row.runtime !== null)),
);

const rows = computed<PluginRow[]>(() => {
    if (!state.value) {
        return [];
    }
    const entries = state.value.sourcemod.plugins;
    const files = entries.map((entry) => entry.name);
    let { matched } = matchRuntimeToFiles(files, smRuntime.value);
    // Fold in whatever `sm plugins info` has answered so far.
    for (const runtime of smRuntime.value) {
        const file = resolvedFiles.value[String(runtime.index)];
        if (file) {
            matched = applyResolvedFile(files, matched, runtime, file);
        }
    }
    return entries.map((entry, index) => {
        const runtime = matched[index];
        const { status, detail } = computeRowStatus({
            enabled: entry.enabled,
            missing: entry.missing,
            runtime,
            rconOk: rconOk.value,
        });
        return {
            // Enabled state is part of the key: a plugin present in BOTH
            // plugins/ and plugins/disabled/ (a broken state) must show as two
            // rows, not have one silently swallow the other in the keyed diff.
            key: `sm:${entry.name}:${entry.enabled ? 'on' : 'off'}`,
            name: entry.name,
            file: entry.file,
            displayName: runtime?.name ?? prettyName(entry.name),
            version: runtime?.version ?? null,
            author: runtime?.author ?? null,
            enabled: entry.enabled,
            comment: entry.comment,
            missing: entry.missing,
            runtime,
            hasConfig: entry.has_config,
            configPath: entry.config_path,
            status,
            statusDetail: detail,
            groupIndex: entry.group_index,
            groupTitle: entry.group_title,
        };
    });
});

async function refreshState(): Promise<void> {
    loading.value = true;
    loadError.value = null;
    try {
        state.value = await getState(props.pluginId, props.serverId);
    } catch (error) {
        loadError.value = apiErrorMessage(error, trans('load_failed'));
    } finally {
        loading.value = false;
    }
}

async function refreshRcon(): Promise<void> {
    if (!serverOnline.value) {
        rconAvailability.value = 'offline';
        metaVersion.value = null;
        smVersion.value = null;
        smRuntime.value = [];
        return;
    }
    try {
        const metaVersionOut = await rcon(props.serverId, 'meta version', { allowEmpty: true });
        if (metaVersionOut.trim() === '' || isUnknownCommandOutput(metaVersionOut)) {
            // `meta` is Metamod's own command, so the console not knowing it
            // means Metamod is not loaded in the running server (installed
            // while it was up, restart pending). Prove the console itself
            // works with a command that always prints.
            await rcon(props.serverId, 'status');
            rconAvailability.value = 'no-metamod';
            rconErrorDetail.value = null;
            metaVersion.value = null;
            smVersion.value = null;
            smRuntime.value = [];
            return;
        }
        metaVersion.value = parseMetaVersion(metaVersionOut);

        const metaListOut = await rcon(props.serverId, 'meta list', { allowEmpty: true });
        smVersion.value = smVersionFromMetaList(parseMetaList(metaListOut));

        // `sm` is registered by SourceMod, so the console not knowing it means
        // SourceMod is not loaded — which the files on disk cannot tell us.
        // Metamod answering while SourceMod does not is the giveaway, and
        // Metamod's own version stays on show.
        const smPluginsOut = await rcon(props.serverId, 'sm plugins list', { allowEmpty: true });
        if (isUnknownCommandOutput(smPluginsOut) || smPluginsOut.trim() === '') {
            rconAvailability.value = 'no-sourcemod';
            rconErrorDetail.value = null;
            smVersion.value = null;
            smRuntime.value = [];
            return;
        }
        // `sm version` is more precise than the meta list entry; either is fine.
        if (!smVersion.value) {
            smVersion.value = parseSmVersion(
                await rcon(props.serverId, 'sm version', { allowEmpty: true }),
            );
        }
        smRuntime.value = parseSmPlugins(smPluginsOut);
        rconAvailability.value = 'ok';
        await resolveUnmatchedPlugins();
    } catch (error) {
        applyRconFailure(error);
    }
}

/**
 * `sm plugins list` prints a loaded plugin by its *title* — free text with no
 * mechanical relationship to its file name ("Admin File Reader" is
 * admin-flatfile.smx). Name matching places most rows; the ones it cannot are
 * asked about directly with `sm plugins info <#>`, which names the file. That
 * costs one round trip per unplaced plugin, so the answers are cached for the
 * session and only ever fetched once.
 */
const MAX_INFO_LOOKUPS = 12;

async function resolveUnmatchedPlugins(): Promise<void> {
    if (!state.value) {
        return;
    }
    const files = state.value.sourcemod.plugins.map((entry) => entry.name);
    const { unmatched } = matchRuntimeToFiles(files, smRuntime.value);
    const pending = unmatched
        .filter((runtime) => resolvedFiles.value[String(runtime.index)] === undefined)
        .slice(0, MAX_INFO_LOOKUPS);
    for (const runtime of pending) {
        try {
            const file = parseSmPluginFile(await smPluginInfo(props.serverId, runtime.index));
            if (file) {
                resolvedFiles.value = { ...resolvedFiles.value, [String(runtime.index)]: file };
            }
        } catch {
            return; // console went away mid-sweep; the rows just stay unpaired
        }
    }
}

/** Mark the console unavailable after a failed RCON call and drop runtime data. */
function applyRconFailure(error: unknown): void {
    const reason = error instanceof RconError ? error.reason : 'error';
    rconAvailability.value = reason;
    rconErrorDetail.value =
        reason === 'error' && error instanceof Error ? error.message : null;
    metaVersion.value = null;
    smVersion.value = null;
    smRuntime.value = [];
}

async function refreshAll(): Promise<void> {
    await Promise.all([refreshState(), refreshRcon(), refreshUpdates()]);
}

/** Update info is an enhancement — its failure never blocks the tab. */
async function refreshUpdates(): Promise<void> {
    try {
        updatesData.value = await getUpdates(props.pluginId, props.serverId);
    } catch {
        updatesData.value = null;
    }
}

function toast(type: 'success' | 'error' | 'info', text: string): void {
    window.$message?.[type]?.(text);
}

async function onToggle(row: PluginRow, value: boolean): Promise<void> {
    mutating.value = true;
    try {
        await togglePlugin(props.pluginId, props.serverId, row.name, value);
        restartDirty.value = true;
        toast('success', trans(value ? 'toggled_on' : 'toggled_off', { name: row.displayName }));
        await refreshState();
    } catch (error) {
        toast('error', apiErrorMessage(error, trans('op_failed')));
    } finally {
        mutating.value = false;
    }
}

/**
 * Hot load/unload/reload/unpause over RCON — the SourceMod analogue of the
 * GoldSource plugin's `amxx pause/unpause`. SourceMod confirms each on the
 * console, so its own answer decides success rather than a re-list race.
 */
async function onHotAction(
    row: PluginRow,
    action: 'load' | 'unload' | 'reload' | 'unpause',
): Promise<void> {
    mutating.value = true;
    try {
        const output = await smPluginsCommand(props.serverId, action, row.file);
        if (isUnknownCommandOutput(output)) {
            rconAvailability.value = 'no-sourcemod';
            toast('error', trans('rcon_sm_not_loaded'));
            return;
        }
        const succeeded = isSmActionSuccess(output);
        if (succeeded) {
            const key = {
                load: 'loaded_ok',
                unload: 'unloaded_ok',
                reload: 'reloaded_ok',
                unpause: 'unpaused_ok',
            }[action];
            toast('success', trans(key, { name: row.displayName }));
        } else {
            // SourceMod states the reason on the console ("Native X was not
            // found"), and puts the same line in the error log. Show whichever
            // we have rather than a red toast with no cause.
            const detail = output.trim() || (await lastLogError(row.name));
            toast(
                'error',
                detail !== ''
                    ? detail
                    : trans(action === 'unload' ? 'unload_failed' : 'load_failed_named', {
                          name: row.displayName,
                      }),
            );
        }
        // Re-read the runtime either way: a failed load can still have changed
        // the list, and a successful one certainly did.
        smRuntime.value = parseSmPlugins(
            await rcon(props.serverId, 'sm plugins list', { allowEmpty: true }),
        );
        await resolveUnmatchedPlugins();
    } catch (error) {
        applyRconFailure(error);
        toast(
            'error',
            error instanceof RconError && error.reason === 'bad-password'
                ? trans('rcon_unavailable_badpass')
                : apiErrorMessage(error, trans('op_failed')),
        );
    } finally {
        mutating.value = false;
    }
}

/** Newest error line in the SourceMod error log, preferring one that names the
 * plugin. Best-effort: on any failure the caller's generic message stands. */
async function lastLogError(pluginName: string): Promise<string> {
    try {
        const { lines } = await getLogs(props.pluginId, props.serverId, 'errors');
        const errors = lines.filter((line) => /error|unable to|exception|fail/i.test(line));
        const named = errors.filter((line) => line.includes(pluginName));
        const line = (named.length > 0 ? named : errors).at(-1);
        return line ? line.trim().slice(0, 240) : '';
    } catch {
        return '';
    }
}

async function onSetComment(row: PluginRow, text: string): Promise<void> {
    const comment = text.trim() || null;
    // A comment is cosmetic — skip a no-op write.
    if (comment === (row.comment ?? null)) {
        return;
    }
    mutating.value = true;
    try {
        await setAttributes(props.pluginId, props.serverId, row.name, comment, row.groupTitle);
        toast('success', trans('comment_saved', { name: row.displayName }));
        await refreshState();
    } catch (error) {
        toast('error', apiErrorMessage(error, trans('op_failed')));
    } finally {
        mutating.value = false;
    }
}

function onDelete(row: PluginRow): void {
    window.$dialog?.success({
        title: trans('delete_title', { name: row.displayName }),
        content: trans('delete_text'),
        positiveText: trans('yes'),
        negativeText: trans('no'),
        closable: false,
        onPositiveClick: async () => {
            mutating.value = true;
            try {
                await deletePlugin(props.pluginId, props.serverId, row.name);
                toast('success', trans('deleted', { name: row.displayName }));
                await refreshState();
            } catch (error) {
                toast('error', apiErrorMessage(error, trans('op_failed')));
            } finally {
                mutating.value = false;
            }
        },
    });
}

async function applyBulkToggle(bulkRows: PluginRow[], value: boolean): Promise<void> {
    mutating.value = true;
    let changed = 0;
    try {
        for (const row of bulkRows) {
            if (row.enabled === value || row.missing) {
                continue;
            }
            await togglePlugin(props.pluginId, props.serverId, row.name, value);
            changed += 1;
        }
        if (changed > 0) {
            restartDirty.value = true;
            toast('success', trans(value ? 'bulk_enabled' : 'bulk_disabled', { count: changed }));
            await refreshState();
        }
    } catch (error) {
        toast('error', apiErrorMessage(error, trans('op_failed')));
        await refreshState();
    } finally {
        mutating.value = false;
    }
}

function onBulk(action: 'enable' | 'disable' | 'delete', bulkRows: PluginRow[]): void {
    if (action !== 'delete') {
        void applyBulkToggle(bulkRows, action === 'enable');
        return;
    }
    if (bulkRows.length === 0) {
        return;
    }
    window.$dialog?.success({
        title: trans('bulk_delete_title', { count: bulkRows.length }),
        content: trans('bulk_delete_text'),
        positiveText: trans('yes'),
        negativeText: trans('no'),
        closable: false,
        onPositiveClick: async () => {
            mutating.value = true;
            let deleted = 0;
            try {
                for (const row of bulkRows) {
                    await deletePlugin(props.pluginId, props.serverId, row.name);
                    deleted += 1;
                }
                toast('success', trans('bulk_deleted', { count: deleted }));
            } catch (error) {
                toast('error', apiErrorMessage(error, trans('op_failed')));
            } finally {
                await refreshState();
                mutating.value = false;
            }
        },
    });
}

async function onInstalled(): Promise<void> {
    await refreshState();
}

async function onCatalogInstalled(): Promise<void> {
    restartDirty.value = true;
    await refreshState();
}

async function onSnapshotRestored(): Promise<void> {
    restartDirty.value = true;
    snapshotsOpen.value = false;
    await refreshState();
}

async function onRepair(): Promise<void> {
    mutating.value = true;
    try {
        const result = await repairMetamod(props.pluginId, props.serverId);
        toast('success', trans(result.changed ? 'loader_repaired' : 'loader_already_ok'));
        restartDirty.value = restartDirty.value || result.changed;
        await refreshState();
    } catch (error) {
        toast('error', apiErrorMessage(error, trans('op_failed')));
    } finally {
        mutating.value = false;
    }
}

function onToggleMetamodPlugin(plugin: MetamodPluginEntry, enabled: boolean): void {
    // Switching SourceMod's own registration off unloads the platform. It sits
    // in the same list as ordinary binary plugins, so it gets an explicit
    // confirmation rather than behaving like one.
    if (plugin.platform && !enabled) {
        window.$dialog?.warning({
            title: trans('mm_platform_title'),
            content: trans('mm_platform_text'),
            positiveText: trans('yes'),
            negativeText: trans('no'),
            closable: false,
            onPositiveClick: () => void applyMetamodToggle(plugin, false, true),
        });
        return;
    }
    void applyMetamodToggle(plugin, enabled, false);
}

async function applyMetamodToggle(
    plugin: MetamodPluginEntry,
    enabled: boolean,
    force: boolean,
): Promise<void> {
    mutating.value = true;
    try {
        await toggleMetamodPlugin(props.pluginId, props.serverId, {
            name: plugin.name,
            enabled,
            source: plugin.source,
            line: plugin.line,
            force,
        });
        restartDirty.value = true;
        toast('success', trans(enabled ? 'mm_enabled' : 'mm_disabled', { name: plugin.name }));
        await refreshState();
    } catch (error) {
        toast('error', apiErrorMessage(error, trans('op_failed')));
    } finally {
        mutating.value = false;
    }
}

function onPlatformInstall(kind: 'metamod' | 'sourcemod'): void {
    const name = kind === 'metamod' ? 'Metamod:Source' : 'SourceMod';
    window.$dialog?.warning({
        title: trans('platform_install_title', { name }),
        content: trans('platform_install_text', { name }),
        positiveText: trans('yes'),
        negativeText: trans('no'),
        onPositiveClick: async () => {
            platformBusy.value = true;
            try {
                const result = await installPlatform(props.pluginId, props.serverId, kind);
                toast('success', trans('platform_installed', { name, version: result.version }));
                restartDirty.value = true;
                await refreshAll();
            } catch (error) {
                toast('error', apiErrorMessage(error, trans('op_failed')));
            } finally {
                platformBusy.value = false;
            }
        },
    });
}

function onRestart(): void {
    window.$dialog?.warning({
        title: trans('restart_title'),
        content: trans('restart_text'),
        positiveText: trans('yes'),
        negativeText: trans('no'),
        onPositiveClick: async () => {
            mutating.value = true;
            try {
                await restartServer(props.pluginId, props.serverId);
                restartDirty.value = false;
                toast('success', trans('restart_sent'));
                // Give the server a moment to come back before re-querying.
                window.setTimeout(() => void refreshAll(), 5000);
            } catch (error) {
                toast('error', apiErrorMessage(error, trans('op_failed')));
            } finally {
                mutating.value = false;
            }
        },
    });
}

function openLogs(filter = ''): void {
    logsFilter.value = filter;
    logsOpen.value = true;
}

/** Reinstall every catalog plugin wearing an update badge, one by one. */
function onUpdateAll(): void {
    const targets = updatableCatalog.value;
    if (targets.length === 0) {
        return;
    }
    window.$dialog?.warning({
        title: trans('update_all_title', { count: targets.length }),
        content: trans('update_all_text'),
        positiveText: trans('yes'),
        negativeText: trans('no'),
        onPositiveClick: async () => {
            mutating.value = true;
            let updated = 0;
            const failed: string[] = [];
            try {
                for (const target of targets) {
                    try {
                        await installCatalogEntry(props.pluginId, props.serverId, target.key);
                        updated += 1;
                    } catch {
                        failed.push(target.plugin);
                    }
                }
            } finally {
                mutating.value = false;
            }
            if (failed.length) {
                toast('error', trans('update_all_partial', { count: updated, failed: failed.join(', ') }));
            } else {
                toast('success', trans('update_all_done', { count: updated }));
            }
            restartDirty.value = true;
            await refreshAll();
        },
    });
}

function openConfig(row: PluginRow): void {
    configRow.value = row;
    configOpen.value = true;
}

function openFileManager(): void {
    window.location.hash = '#files';
}

// The server object can arrive after the tab mounts (async store load) —
// re-query the console once the server turns out to be online.
watch(serverOnline, (online, wasOnline) => {
    if (online && !wasOnline) {
        void refreshRcon();
    }
});

onMounted(() => {
    if (isSourceOne.value) {
        void refreshAll();
    }
});
</script>

<style scoped>
/* On md+ screens the platform cards sit flush above this panel and the
 * SourceMod card reads as a tab merged into it (see PlatformCard.vue). */
@media (min-width: 768px) {
    .platform-cards {
        margin-bottom: 0;
    }

    .plugins-panel.plugins-panel {
        border-top: none;
        border-top-left-radius: 0;
        border-top-right-radius: 0;
    }
}
</style>
