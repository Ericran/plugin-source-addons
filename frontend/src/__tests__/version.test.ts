import { describe, expect, it } from 'vitest';

import { versionKey, versionsMatch } from '../lib/version';

describe('versionKey', () => {
    it('strips prefixes, hashes and separators down to the numbers', () => {
        expect(versionKey('v1.12.0.7210')).toBe('1.12.0.7210');
        expect(versionKey('1.12.0.7210 @ 3923c5d')).toBe('1.12.0.7210');
        expect(versionKey('1.12.0-dev+1219')).toBe('1.12.0.1219');
        expect(versionKey('1.12.0-git1219')).toBe('1.12.0.1219');
    });
});

describe('versionsMatch', () => {
    it('treats differently-labeled identical builds as equal', () => {
        // The two false-badge field reports, verbatim.
        expect(versionsMatch('1.12.0.7210 @ 3923c5d', '1.12.0.7210')).toBe(true);
        expect(versionsMatch('1.12.0-dev+1219', '1.12.0-git1219')).toBe(true);
        expect(versionsMatch('v0.9.1', '0.9.1')).toBe(true);
    });

    it('still detects real updates', () => {
        expect(versionsMatch('1.12.0.7210 @ 3923c5d', '1.12.0.7211')).toBe(false);
        expect(versionsMatch('1.12.0-dev+1219', '1.12.0-git1220')).toBe(false);
        expect(versionsMatch('0.9.1', '0.10.0')).toBe(false);
    });

    it('falls back to plain comparison for number-free strings', () => {
        expect(versionsMatch('latest', 'Latest')).toBe(true);
        expect(versionsMatch('latest', 'stable')).toBe(false);
    });
});
