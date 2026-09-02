import { describe, expect, it } from 'vitest';

import {
    computeRowStatus,
    hotActionForStatus,
    secondaryActionForStatus,
} from '../lib/status';
import type { RuntimePluginInfo } from '../types';

function runtime(
    status: RuntimePluginInfo['status'],
    rawStatus: string = status,
): RuntimePluginInfo {
    return {
        name: 'Roll The Dice',
        index: 3,
        file: null,
        version: '2.3.1',
        author: null,
        status,
        rawStatus,
    };
}

describe('computeRowStatus', () => {
    it('missing wins over everything', () => {
        expect(
            computeRowStatus({ enabled: true, missing: true, runtime: runtime('running'), rconOk: true }),
        ).toEqual({ status: 'missing', detail: null });
    });

    it('runtime error is reported with detail', () => {
        expect(
            computeRowStatus({
                enabled: true,
                missing: false,
                runtime: runtime('error', 'Failed'),
                rconOk: true,
            }),
        ).toEqual({ status: 'error', detail: 'Failed' });
    });

    it('falls back to file state without a console', () => {
        expect(
            computeRowStatus({ enabled: true, missing: false, runtime: null, rconOk: false }),
        ).toEqual({ status: 'enabled', detail: null });
        expect(
            computeRowStatus({ enabled: false, missing: false, runtime: null, rconOk: false }),
        ).toEqual({ status: 'stopped', detail: null });
    });

    it('paused is its own state, not a failure and not awaiting load', () => {
        expect(
            computeRowStatus({
                enabled: true,
                missing: false,
                runtime: runtime('paused', '!'),
                rconOk: true,
            }),
        ).toEqual({ status: 'paused', detail: '!' });
    });

    it('running needs both the file and the runtime', () => {
        expect(
            computeRowStatus({ enabled: true, missing: false, runtime: runtime('running'), rconOk: true }),
        ).toEqual({ status: 'running', detail: null });
    });

    it('marks the two out-of-step combinations as pending', () => {
        // On disk, not loaded.
        expect(
            computeRowStatus({ enabled: true, missing: false, runtime: null, rconOk: true }).status,
        ).toBe('pending');
        // Parked on disk, still in memory until a restart.
        expect(
            computeRowStatus({
                enabled: false,
                missing: false,
                runtime: runtime('running'),
                rconOk: true,
            }).status,
        ).toBe('pending');
    });
});

describe('hotActionForStatus', () => {
    it('offers unload for anything loaded', () => {
        expect(hotActionForStatus('running', true)).toBe('unload');
        expect(hotActionForStatus('paused', true)).toBe('unload');
    });

    it('offers load only for an enabled plugin that is not loaded', () => {
        expect(hotActionForStatus('pending', true)).toBe('load');
        expect(hotActionForStatus('stopped', true)).toBe('load');
        expect(hotActionForStatus('pending', false)).toBeNull();
        expect(hotActionForStatus('enabled', true)).toBeNull();
        expect(hotActionForStatus('missing', true)).toBeNull();
    });
});

describe('secondaryActionForStatus', () => {
    it('reloads a running plugin and resumes a paused one', () => {
        expect(secondaryActionForStatus('running')).toBe('reload');
        expect(secondaryActionForStatus('paused')).toBe('unpause');
        expect(secondaryActionForStatus('pending')).toBeNull();
        expect(secondaryActionForStatus('error')).toBeNull();
    });
});
