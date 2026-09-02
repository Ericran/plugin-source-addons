import { describe, expect, it } from 'vitest';

import { fileExtension, fileStem, prettyName } from '../lib/naming';

describe('naming', () => {
    it('extracts extensions and stems', () => {
        expect(fileExtension('funcommands.smx')).toBe('smx');
        expect(fileExtension('noext')).toBe('');
        expect(fileStem('funcommands.smx')).toBe('funcommands');
        expect(fileStem('sbpp_main.smx')).toBe('sbpp_main');
        expect(fileStem('noext')).toBe('noext');
    });

    it('prettifies plugin file stems', () => {
        expect(prettyName('high_ping_kicker')).toBe('High Ping Kicker');
        expect(prettyName('admin-flatfile')).toBe('Admin Flatfile');
        expect(prettyName('sbpp_main')).toBe('Sbpp Main');
        expect(prettyName('rtd')).toBe('Rtd');
    });
});
