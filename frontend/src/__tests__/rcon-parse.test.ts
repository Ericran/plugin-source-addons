import { describe, expect, it } from 'vitest';

import {
    applyResolvedFile,
    isBadPasswordOutput,
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

describe('parseMetaVersion', () => {
    it('parses Metamod:Source 1.x output', () => {
        const output = [
            ' Metamod:Source version 1.12.0-dev+1219',
            ' Build ID: 1219:9e5f0a2',
            ' Loaded As: Valve Server Plugin',
            ' Compiled on: Aug 12 2026',
        ].join('\n');
        expect(parseMetaVersion(output)).toEqual({
            build: 'Metamod:Source',
            version: '1.12.0-dev+1219',
        });
    });

    it('returns null for unrelated output', () => {
        expect(parseMetaVersion('Unknown command "meta"')).toBeNull();
        expect(parseMetaVersion('')).toBeNull();
    });
});

describe('parseMetaList', () => {
    const output = [
        'Listing 3 plugins:',
        '  [01] SourceMod (1.12.0.7210) by AlliedModders LLC',
        '  [02] Stripper (1.2.2) by BAILOPAN',
        '  [03] <FAILED> Broken Thing (0.1) by Someone',
    ].join('\n');

    it('parses names, versions and authors', () => {
        const entries = parseMetaList(output);
        expect(entries).toHaveLength(3);
        expect(entries[0]).toEqual({
            name: 'SourceMod',
            version: '1.12.0.7210',
            author: 'AlliedModders LLC',
            state: null,
        });
        expect(entries[1].name).toBe('Stripper');
    });

    it('keeps a state marker out of the name', () => {
        const entries = parseMetaList(output);
        expect(entries[2].name).toBe('Broken Thing');
        expect(entries[2].state).toBe('FAILED');
    });

    it('finds SourceMod for the platform card', () => {
        expect(smVersionFromMetaList(parseMetaList(output))).toEqual({
            build: 'SourceMod',
            version: '1.12.0.7210',
        });
        expect(smVersionFromMetaList(parseMetaList('Listing 0 plugins:'))).toBeNull();
    });
});

describe('parseSmVersion', () => {
    it('parses the sm version block', () => {
        const output = [
            'SourceMod Version Information:',
            '    SourceMod Version: 1.12.0.7210',
            '    SourcePawn Engine: 1.12.0.7210, jit-x86 (build 1.12.0.7210)',
            '    Built on: Aug 12 2026',
        ].join('\n');
        expect(parseSmVersion(output)).toEqual({ build: 'SourceMod', version: '1.12.0.7210' });
    });
});

describe('parseSmPlugins', () => {
    const output = [
        '[SM] Listing 5 plugins:',
        '  01 "Admin File Reader" (1.12.0.7210) by AlliedModders LLC',
        '  02 "Fun Commands" (1.12.0.7210) by AlliedModders LLC',
        '  03 <!> "Roll The Dice" (2.3.1) by Phil25',
        '  04 <Failed> sbpp_main.smx',
        '  05 "No Author Plugin" (0.1)',
    ].join('\n');

    it('parses loaded plugins with their index', () => {
        const plugins = parseSmPlugins(output);
        expect(plugins).toHaveLength(5);
        expect(plugins[0]).toEqual({
            name: 'Admin File Reader',
            index: 1,
            file: null,
            version: '1.12.0.7210',
            author: 'AlliedModders LLC',
            status: 'running',
            rawStatus: 'Running',
        });
        expect(plugins[4].author).toBeNull();
    });

    it('reads the <!> marker as paused, not as part of the name', () => {
        const rtd = parseSmPlugins(output)[2];
        expect(rtd.name).toBe('Roll The Dice');
        expect(rtd.status).toBe('paused');
    });

    it('takes the file name from a failed plugin, which is all it prints', () => {
        const failed = parseSmPlugins(output)[3];
        expect(failed.file).toBe('sbpp_main.smx');
        expect(failed.name).toBe('sbpp_main');
        expect(failed.status).toBe('error');
        expect(failed.rawStatus).toBe('Failed');
    });

    it('ignores headers and junk', () => {
        expect(parseSmPlugins('[SM] Listing 0 plugins:')).toHaveLength(0);
        expect(parseSmPlugins('Unknown command "sm"')).toHaveLength(0);
    });

    it('survives a version containing parentheses', () => {
        const plugins = parseSmPlugins('  01 "RockTheVote" (1.9.6 (RELEASE)) by Author');
        expect(plugins).toHaveLength(1);
        expect(plugins[0].version).toBe('1.9.6');
    });
});

describe('parseSmPluginFile', () => {
    it('reads the filename out of sm plugins info', () => {
        const output = [
            '[SM] Filename: admin-flatfile.smx',
            '[SM] Title: Admin File Reader (Reads admin config files)',
            '[SM] Author: AlliedModders LLC',
            '[SM] Status: running',
        ].join('\n');
        expect(parseSmPluginFile(output)).toBe('admin-flatfile.smx');
        expect(parseSmPluginFile('[SM] Plugin index 9 not found.')).toBeNull();
    });
});

describe('matchRuntimeToFiles', () => {
    it('pairs a failed plugin by the file the console printed', () => {
        const runtime = parseSmPlugins('  04 <Failed> sbpp_main.smx');
        const { matched, unmatched } = matchRuntimeToFiles(['funcommands', 'sbpp_main'], runtime);
        expect(matched[0]).toBeNull();
        expect(matched[1]?.status).toBe('error');
        expect(unmatched).toHaveLength(0);
    });

    it('pairs SourceMod bundled plugins whose titles do not resemble their files', () => {
        const runtime = parseSmPlugins(
            [
                '  01 "Admin File Reader" (1.12.0.7210) by AlliedModders LLC',
                '  02 "Basic Ban Commands" (1.12.0.7210) by AlliedModders LLC',
            ].join('\n'),
        );
        const { matched } = matchRuntimeToFiles(['admin-flatfile', 'basebans'], runtime);
        expect(matched[0]?.name).toBe('Admin File Reader');
        expect(matched[1]?.name).toBe('Basic Ban Commands');
    });

    it('pairs a title that normalizes onto its file name', () => {
        const runtime = parseSmPlugins('  01 "Map Chooser Extended" (1.12) by Powerlord');
        const { matched } = matchRuntimeToFiles(['mapchooser_extended'], runtime);
        expect(matched[0]?.name).toBe('Map Chooser Extended');
    });

    it('reports what it could not place instead of guessing', () => {
        // "Roll The Dice" is rtd.smx, and nothing in the string says so.
        const runtime = parseSmPlugins('  03 "Roll The Dice" (2.3.1) by Phil25');
        const { matched, unmatched } = matchRuntimeToFiles(['rtd'], runtime);
        expect(matched[0]).toBeNull();
        expect(unmatched).toHaveLength(1);
        expect(unmatched[0].index).toBe(3);
    });

    it('never binds one runtime entry to two files', () => {
        const runtime = parseSmPlugins('  01 "Fun Commands" (1.0) by A');
        const { matched } = matchRuntimeToFiles(['funcommands', 'funcommands2'], runtime);
        expect(matched.filter(Boolean)).toHaveLength(1);
    });
});

describe('applyResolvedFile', () => {
    it('folds an sm plugins info answer into the pairing', () => {
        const runtime = parseSmPlugins('  03 "Roll The Dice" (2.3.1) by Phil25');
        const files = ['funcommands', 'rtd'];
        const { matched } = matchRuntimeToFiles(files, runtime);
        const resolved = applyResolvedFile(files, matched, runtime[0], 'rtd.smx');
        expect(resolved[1]?.name).toBe('Roll The Dice');
        expect(resolved[0]).toBeNull();
    });

    it('leaves the pairing alone when the answer names an unknown file', () => {
        const runtime = parseSmPlugins('  03 "Something" (1.0) by A');
        const files = ['funcommands'];
        const { matched } = matchRuntimeToFiles(files, runtime);
        expect(applyResolvedFile(files, matched, runtime[0], 'elsewhere.smx')).toEqual(matched);
    });
});

describe('console output classification', () => {
    it('recognizes an unknown command', () => {
        expect(isUnknownCommandOutput('Unknown command "sm"')).toBe(true);
        expect(isUnknownCommandOutput('[SM] Listing 2 plugins:')).toBe(false);
    });

    it('recognizes a bad password', () => {
        expect(isBadPasswordOutput('Bad rcon_password.')).toBe(true);
        expect(isBadPasswordOutput('hostname: server')).toBe(false);
    });

    it('recognizes a successful hot action', () => {
        expect(isSmActionSuccess('[SM] Loaded plugin rtd.smx successfully.')).toBe(true);
        expect(isSmActionSuccess('[SM] Plugin unloaded successfully.')).toBe(true);
        expect(isSmActionSuccess('[SM] Plugin reloaded successfully.')).toBe(true);
        // Pause and unpause are confirmed differently.
        expect(isSmActionSuccess('[SM] Plugin has been unpaused.')).toBe(true);
    });

    it('does not read a failure that mentions the verb as success', () => {
        expect(
            isSmActionSuccess('[SM] Plugin rtd.smx failed to load: Native "X" was not found.'),
        ).toBe(false);
        expect(isSmActionSuccess('[SM] Plugin rtd.smx is not loaded.')).toBe(false);
        expect(isSmActionSuccess('')).toBe(false);
    });
});
