// Existing GameAP panel endpoints the plugin frontend uses directly:
// RCON for versions/runtime status, file-manager for uploads and configs.

import axios from 'axios';

import { asHttpError, httpBodyMessage, httpStatus } from '../lib/http-error';
import { isBadPasswordOutput } from '../lib/rcon-parse';

export type RconFailure = 'offline' | 'no-rcon' | 'bad-password' | 'empty' | 'error';

export class RconError extends Error {
    reason: RconFailure;

    constructor(reason: RconFailure, message: string) {
        super(message);
        this.reason = reason;
    }
}

/** POST /api/servers/{id}/rcon — synchronous command output. */
export async function rcon(
    serverId: number,
    command: string,
    options?: { allowEmpty?: boolean },
): Promise<string> {
    try {
        const response = await axios.post(`/api/servers/${serverId}/rcon`, { command });
        const output = String(response.data?.output ?? '');
        if (isBadPasswordOutput(output)) {
            throw new RconError('bad-password', 'wrong rcon password');
        }
        if (output.trim() === '' && !options?.allowEmpty) {
            throw new RconError('empty', 'empty rcon output');
        }
        return output;
    } catch (error) {
        throw toRconError(error);
    }
}

export type SmPluginAction = 'load' | 'unload' | 'reload' | 'pause' | 'unpause';

/**
 * `sm plugins <action> <file>` — hot load, unload, reload, pause or unpause a
 * plugin without a restart.
 *
 * SourceMod answers every one of these on the console ("[SM] Loaded plugin
 * rtd.smx successfully."), so an empty reply is a real failure rather than the
 * normal outcome — no allowEmpty here.
 */
export async function smPluginsCommand(
    serverId: number,
    action: SmPluginAction,
    file: string,
): Promise<string> {
    return rcon(serverId, `sm plugins ${action} "${file}"`);
}

/** `sm plugins info <#>` — the authoritative plugin-title-to-file mapping. */
export async function smPluginInfo(serverId: number, index: number): Promise<string> {
    return rcon(serverId, `sm plugins info ${index}`);
}

function toRconError(error: unknown): RconError {
    if (error instanceof RconError) {
        return error;
    }
    const status = httpStatus(error);
    if (status === 503) {
        return new RconError('offline', 'server is offline');
    }
    if (status === 412) {
        return new RconError('no-rcon', 'rcon password is not configured');
    }
    if (status === 422) {
        return new RconError('bad-password', 'rcon authentication failed');
    }
    const http = asHttpError(error);
    if (http) {
        // The panel body's "message" says what actually broke (e.g. "failed to
        // execute rcon command: response ID mismatch"); the transport message
        // is only the HTTP status line.
        return new RconError('error', httpBodyMessage(error) ?? http.message ?? 'request failed');
    }
    return new RconError('error', error instanceof Error ? error.message : String(error));
}

/** POST /api/file-manager/{id}/update-file — multipart write into a directory. */
export async function fmUploadFile(
    serverId: number,
    directory: string,
    file: File,
    onProgress?: (percent: number) => void,
): Promise<void> {
    const form = new FormData();
    form.append('disk', 'server');
    form.append('path', directory);
    form.append('file', file);
    await axios.post(`/api/file-manager/${serverId}/update-file`, form, {
        onUploadProgress: (event) => {
            if (onProgress && event.total) {
                onProgress(Math.round((event.loaded / event.total) * 100));
            }
        },
    });
}

/** GET /api/file-manager/{id}/download — raw file content as text. */
export async function fmDownloadText(serverId: number, path: string): Promise<string> {
    const response = await axios.get(`/api/file-manager/${serverId}/download`, {
        params: { disk: 'server', path },
        responseType: 'text',
        transformResponse: [(data: unknown) => data],
    });
    return String(response.data ?? '');
}

