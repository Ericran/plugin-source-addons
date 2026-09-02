// Parsers of Source 1 console command output obtained via RCON.

import type { PlatformVersion, RuntimePluginInfo } from '../types';

/**
 * `meta version` →
 *   Metamod:Source version 1.12.0-dev+1219
 *   Build ID: 1219:9e5f0a2
 *   ...
 */
export function parseMetaVersion(output: string): PlatformVersion | null {
    const match = /Metamod:Source\s+version\s+v?([0-9][\w.+-]*)/i.exec(output);
    if (!match) {
        return null;
    }
    return { build: 'Metamod:Source', version: match[1].replace(/[.,]+$/, '') };
}

/**
 * `meta list` →
 *   Listing 2 plugins:
 *     [01] SourceMod (1.12.0.7210) by AlliedModders LLC
 *     [02] Stripper (1.2.2) by BAILOPAN
 *
 * A plugin that failed or was paused carries a state marker before its name
 * ("[03] <FAILED> Foo (1.0) by Bar"), which is captured separately so it does
 * not end up glued to the name.
 */
const META_LIST_LINE =
    /^\s*\[\s*(\d+)\]\s+(?:<(?<state>[^>]*)>\s+)?(?<name>.+?)\s+\(v?(?<vers>.*)\)(?:\s+by\s+(?<author>.+))?$/;

export interface MetaListEntry {
    name: string;
    version: string | null;
    author: string | null;
    state: string | null;
}

export function parseMetaList(output: string): MetaListEntry[] {
    const result: MetaListEntry[] = [];
    for (const line of output.split('\n')) {
        const match = META_LIST_LINE.exec(line.trimEnd());
        if (!match?.groups) {
            continue;
        }
        const { name, vers, author, state } = match.groups;
        result.push({
            name: name.trim(),
            version: normalizeVersion(vers),
            author: author?.trim() || null,
            state: state?.trim() || null,
        });
    }
    return result;
}

/**
 * `sm version` →
 *   SourceMod Version Information:
 *      SourceMod Version: 1.12.0.7210
 *      SourcePawn Engine: 1.12.0.7210, jit-x86 (build 1.12.0.7210)
 *      ...
 *
 * Falls back to SourceMod's `meta list` entry, which carries the same number
 * and is one command cheaper when both are being read anyway.
 */
export function parseSmVersion(output: string): PlatformVersion | null {
    const match = /SourceMod\s+Version:\s*v?([0-9][\w.+-]*)/i.exec(output);
    if (!match) {
        return null;
    }
    return { build: 'SourceMod', version: match[1].replace(/[.,]+$/, '') };
}

export function smVersionFromMetaList(entries: MetaListEntry[]): PlatformVersion | null {
    const sm = entries.find((entry) => /^sourcemod$/i.test(entry.name));
    if (!sm) {
        return null;
    }
    return { build: 'SourceMod', version: sm.version ?? '' };
}

/**
 * `sm plugins list` →
 *   [SM] Listing 4 plugins:
 *     01 "Admin File Reader" (1.12.0.7210) by AlliedModders LLC
 *     02 "Fun Commands" (1.12.0.7210) by AlliedModders LLC
 *     03 <!> "Roll The Dice" (2.3.1) by Phil25
 *     04 <Failed> broken.smx
 *
 * Two shapes, and they carry different things. A plugin SourceMod managed to
 * load is printed by its *title* — free text chosen by the author, with no
 * mechanical relationship to its file name. One that failed is printed by its
 * *file* instead, because there is no loaded plugin to ask for a title. So the
 * rows that most need identifying are the ones the console names precisely,
 * and the rest have to be matched by name (see `matchRuntimeToFiles`).
 */
const SM_LISTED_LINE =
    /^\s*(?<index>\d+)\s+(?:<(?<state>[^>]*)>\s+)?"(?<name>.*)"\s+\((?<vers>.*)\)(?:\s+by\s+(?<author>.+))?$/;
const SM_FAILED_LINE = /^\s*(?<index>\d+)\s+<(?<state>[^>]*)>\s+(?<file>\S+\.smx)\s*$/i;

export function parseSmPlugins(output: string): RuntimePluginInfo[] {
    const result: RuntimePluginInfo[] = [];
    for (const raw of output.split('\n')) {
        const line = raw.trimEnd();
        const failed = SM_FAILED_LINE.exec(line);
        if (failed?.groups) {
            const { index, state, file } = failed.groups;
            result.push({
                name: fileStemOf(file),
                index: Number(index),
                file,
                version: null,
                author: null,
                status: 'error',
                rawStatus: state.trim() || 'Failed',
            });
            continue;
        }
        const listed = SM_LISTED_LINE.exec(line);
        if (!listed?.groups) {
            continue;
        }
        const { index, state, name, vers, author } = listed.groups;
        result.push({
            name: name.trim(),
            index: Number(index),
            file: null,
            version: normalizeVersion(vers),
            author: author?.trim() || null,
            status: normalizeSmStatus(state),
            rawStatus: state?.trim() || 'Running',
        });
    }
    return result;
}

/**
 * `sm plugins info <#>` →
 *   [SM] Filename: admin-flatfile.smx
 *   [SM] Title: Admin File Reader (Reads admin config files)
 *   [SM] Author: AlliedModders LLC
 *   ...
 *
 * The authoritative title→file mapping, used to resolve the rows name matching
 * could not place.
 */
export function parseSmPluginFile(output: string): string | null {
    const match = /Filename:\s*(\S+\.smx)/i.exec(output);
    return match ? match[1] : null;
}

/** Titles SourceMod's own bundled plugins carry, mapped to their file stems.
 *
 * These ship with every install and none of their titles resemble their file
 * names ("Admin File Reader" is admin-flatfile.smx), so without this table the
 * default set of ~25 plugins would need a `sm plugins info` round trip each,
 * every time the tab loads. */
const BUNDLED_TITLES: Record<string, string> = {
    'admin file reader': 'admin-flatfile',
    'admin help': 'adminhelp',
    'admin menu': 'adminmenu',
    'sql admin manager': 'sql-admin-manager',
    'sql admins (prefetch)': 'admin-sql-prefetch',
    'sql admins (threaded)': 'admin-sql-threaded',
    'anti-flood': 'antiflood',
    'basecommands': 'basecommands',
    'basic ban commands': 'basebans',
    'basic chat': 'basechat',
    'basic comm control': 'basecomm',
    'basic commands': 'basecommands',
    'basic info triggers': 'funcommands',
    'basic votes': 'basevotes',
    'client preferences': 'clientprefs',
    'fun commands': 'funcommands',
    'fun votes': 'funvotes',
    'map nominations': 'nominations',
    'mapchooser': 'mapchooser',
    'nextmap': 'nextmap',
    'player commands': 'playercommands',
    'reserved slots': 'reservedslots',
    'rock the vote': 'rockthevote',
    'sound commands': 'soundcommands',
};

function normalizeKey(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9]/g, '');
}

function fileStemOf(file: string): string {
    const base = file.split(/[\\/]/).pop() ?? file;
    const dot = base.lastIndexOf('.');
    return dot > 0 ? base.slice(0, dot) : base;
}

/**
 * Pairs plugin file stems with runtime entries from `sm plugins list`.
 *
 * Returns the pairings it is sure of plus the runtime entries it could not
 * place — the caller resolves those with `sm plugins info <#>`, which names the
 * file outright. Guessing further would be worse than asking: a wrong pairing
 * shows one plugin's version and status on another plugin's row.
 */
export function matchRuntimeToFiles(
    files: string[],
    runtimeList: RuntimePluginInfo[],
): { matched: (RuntimePluginInfo | null)[]; unmatched: RuntimePluginInfo[] } {
    const matched: (RuntimePluginInfo | null)[] = files.map(() => null);
    const claimed = new Set<number>();

    const claim = (fileIndex: number, runtimeIndex: number): void => {
        matched[fileIndex] = runtimeList[runtimeIndex];
        claimed.add(runtimeIndex);
    };

    // 1. The console named the file itself (failed plugins). Exact, so first.
    files.forEach((file, fileIndex) => {
        runtimeList.forEach((runtime, runtimeIndex) => {
            if (matched[fileIndex] || claimed.has(runtimeIndex) || !runtime.file) {
                return;
            }
            if (fileStemOf(runtime.file).toLowerCase() === file.toLowerCase()) {
                claim(fileIndex, runtimeIndex);
            }
        });
    });

    // 2. SourceMod's own plugins, by their known titles.
    files.forEach((file, fileIndex) => {
        if (matched[fileIndex]) {
            return;
        }
        runtimeList.forEach((runtime, runtimeIndex) => {
            if (matched[fileIndex] || claimed.has(runtimeIndex) || runtime.file) {
                return;
            }
            const known = BUNDLED_TITLES[runtime.name.trim().toLowerCase()];
            if (known && known.toLowerCase() === file.toLowerCase()) {
                claim(fileIndex, runtimeIndex);
            }
        });
    });

    // 3. Title and file stem agree once punctuation and case are ignored
    //    ("Roll The Dice" ↔ rtd fails here, but "SourceBans++" ↔ sourcebans
    //    is not a guess worth making either — only an exact normalized match
    //    counts).
    const fileKeys = files.map(normalizeKey);
    files.forEach((_, fileIndex) => {
        if (matched[fileIndex] || fileKeys[fileIndex] === '') {
            return;
        }
        runtimeList.forEach((runtime, runtimeIndex) => {
            if (matched[fileIndex] || claimed.has(runtimeIndex) || runtime.file) {
                return;
            }
            if (normalizeKey(runtime.name) === fileKeys[fileIndex]) {
                claim(fileIndex, runtimeIndex);
            }
        });
    });

    const unmatched = runtimeList.filter((_, index) => !claimed.has(index));
    return { matched, unmatched };
}

/** Folds a resolved title→file answer into an existing pairing. */
export function applyResolvedFile(
    files: string[],
    matched: (RuntimePluginInfo | null)[],
    runtime: RuntimePluginInfo,
    file: string,
): (RuntimePluginInfo | null)[] {
    const stem = fileStemOf(file).toLowerCase();
    const index = files.findIndex(
        (candidate, position) => matched[position] === null && candidate.toLowerCase() === stem,
    );
    if (index < 0) {
        return matched;
    }
    const next = [...matched];
    next[index] = { ...runtime, file };
    return next;
}

/**
 * `Unknown command "sm"` — the console does not have that command, which for
 * `sm` means SourceMod is not loaded in the running server (its files on disk
 * say nothing about that).
 */
export function isUnknownCommandOutput(output: string): boolean {
    return /unknown\s+command/i.test(output);
}

/** Source RCON answers with a bad-password notice on auth failure. Note the
 * underscore: srcds prints the cvar name, "Bad rcon_password." */
export function isBadPasswordOutput(output: string): boolean {
    return /bad\s*(rcon[_\s]*)?password/i.test(output);
}

/**
 * Whether SourceMod's answer to a hot action reports success.
 *
 * It confirms most actions with "… successfully." but pause and unpause with
 * "Plugin has been paused." instead, so both shapes count. The failure guard
 * comes first because a failure message can still mention the verb ("Plugin
 * rtd.smx failed to load"), and reading that as success would leave a broken
 * plugin wearing a green toast.
 */
export function isSmActionSuccess(output: string): boolean {
    if (output.trim() === '') {
        return false;
    }
    if (/\b(fail(ed|ure)?|error|unable to|not found|no plugin)\b/i.test(output)) {
        return false;
    }
    return /successfully|has been (paused|unpaused)/i.test(output);
}

function normalizeVersion(raw: string): string | null {
    // "1.2.2 (RELEASE)" → "1.2.2": the decoration is noise in the column and
    // would never compare equal to a release tag when checking for updates.
    const cleaned = raw.replace(/^v/i, '').trim().split(/\s+/)[0] ?? '';
    return /^[0-9]/.test(cleaned) ? cleaned : null;
}

function normalizeSmStatus(state: string | undefined): RuntimePluginInfo['status'] {
    const marker = (state ?? '').trim().toLowerCase();
    if (marker === '') {
        return 'running';
    }
    // "<!>" is SourceMod's generic "not running" marker; paused plugins are
    // still loaded and resumable, everything else is broken.
    if (marker === 'paused' || marker === '!') {
        return 'paused';
    }
    return 'error';
}
