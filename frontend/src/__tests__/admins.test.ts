import { describe, expect, it } from 'vitest';

import { parseAdmins, serializeAdmins } from '../lib/admins';

// The shape SourceMod ships, comment header and all.
const SAMPLE = [
    '/**',
    ' * Admins are defined here.',
    ' */',
    '',
    '"STEAM_0:1:6708057"     "99:z"',
    '"76561198000000000"     "abcdefg"',
    '"!Some Name"            "bc"    "hunter2"',
    '',
].join('\n');

describe('parseAdmins', () => {
    it('reads identity, flags, immunity and password', () => {
        const { entries } = parseAdmins(SAMPLE);
        expect(entries).toHaveLength(3);
        expect(entries[0]).toEqual({
            identity: 'STEAM_0:1:6708057',
            flags: 'z',
            immunity: 99,
            password: '',
            line: 4,
        });
        expect(entries[1].immunity).toBeNull();
        expect(entries[1].flags).toBe('abcdefg');
        expect(entries[2]).toMatchObject({
            identity: '!Some Name',
            flags: 'bc',
            password: 'hunter2',
        });
    });

    it('does not read comments or blank lines as admins', () => {
        const { entries } = parseAdmins('// "STEAM_0:1:1" "z"\n\n; also a comment\n');
        expect(entries).toHaveLength(0);
    });

    it('ignores lines that are not two or three quoted fields', () => {
        const { entries } = parseAdmins('"only one"\n"a" "b" "c" "d"\n');
        expect(entries).toHaveLength(0);
    });
});

describe('serializeAdmins', () => {
    it('round-trips an untouched file, comments included', () => {
        const parsed = parseAdmins(SAMPLE);
        const out = serializeAdmins(parsed, parsed.entries);
        expect(out).toContain(' * Admins are defined here.');
        const reparsed = parseAdmins(out);
        expect(reparsed.entries.map((entry) => entry.identity)).toEqual([
            'STEAM_0:1:6708057',
            '76561198000000000',
            '!Some Name',
        ]);
        expect(reparsed.entries[0].immunity).toBe(99);
        expect(reparsed.entries[2].password).toBe('hunter2');
    });

    it('edits in place and keeps the surrounding lines', () => {
        const parsed = parseAdmins(SAMPLE);
        const entries = parsed.entries.map((entry) =>
            entry.identity === '76561198000000000' ? { ...entry, flags: 'z', immunity: 50 } : entry,
        );
        const out = serializeAdmins(parsed, entries);
        expect(out).toContain('"50:z"');
        expect(out).toContain(' * Admins are defined here.');
        expect(parseAdmins(out).entries).toHaveLength(3);
    });

    it('drops removed entries and appends new ones', () => {
        const parsed = parseAdmins(SAMPLE);
        const entries = [
            ...parsed.entries.filter((entry) => entry.identity !== '!Some Name'),
            {
                identity: 'STEAM_0:1:99999',
                flags: 'abc',
                immunity: null,
                password: '',
                line: null,
            },
        ];
        const out = serializeAdmins(parsed, entries);
        const identities = parseAdmins(out).entries.map((entry) => entry.identity);
        expect(identities).toEqual([
            'STEAM_0:1:6708057',
            '76561198000000000',
            'STEAM_0:1:99999',
        ]);
        expect(out).not.toContain('Some Name');
    });

    it('skips a blank row the user never filled in', () => {
        const parsed = parseAdmins('');
        const out = serializeAdmins(parsed, [
            { identity: '  ', flags: 'z', immunity: null, password: '', line: null },
        ]);
        expect(parseAdmins(out).entries).toHaveLength(0);
    });

    it('keeps CRLF files as CRLF', () => {
        const parsed = parseAdmins('// header\r\n"STEAM_0:1:1"  "z"\r\n');
        const out = serializeAdmins(parsed, parsed.entries);
        expect(out).toContain('\r\n');
        expect(out.split('\r\n')[0]).toBe('// header');
    });
});
