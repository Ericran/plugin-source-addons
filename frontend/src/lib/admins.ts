// Parser and serializer for SourceMod's addons/sourcemod/configs/admins_simple.ini.
//
// The format is one admin per line:
//
//   "STEAM_0:1:6708057"   "99:abcdefghijklmnopqrst"
//   "127.0.0.1"           "z"
//   "!Some Name"          "bc"    "password"
//
// Field 1 is the identity (a Steam ID, an IP, or a name prefixed with `!`),
// field 2 is the flags with an optional `immunity:` prefix, field 3 an
// optional password. Everything else in the file — the long comment header
// SourceMod ships, blank lines, an operator's own notes — is content this
// editor must not eat, so parsing keeps every original line and editing
// rewrites only the lines that are admin entries.

export interface AdminEntry {
    identity: string;
    flags: string;
    /** `null` when the line carries no immunity prefix. */
    immunity: number | null;
    password: string;
    /** Zero-based index of the line this came from, `null` for a new entry. */
    line: number | null;
}

export interface ParsedAdmins {
    /** Original file lines, kept verbatim for serialization. */
    lines: string[];
    entries: AdminEntry[];
    /** The file ended with a newline. */
    trailingNewline: boolean;
    /** The line ending the file uses. */
    newline: string;
}

/** Splits a line into its quoted fields; `null` when it is not an admin line. */
function quotedFields(line: string): string[] | null {
    const trimmed = line.trim();
    if (trimmed === '' || trimmed.startsWith('//') || trimmed.startsWith(';')) {
        return null;
    }
    const fields = [...trimmed.matchAll(/"([^"]*)"/g)].map((match) => match[1]);
    // An admin line is exactly identity + flags, optionally + password.
    if (fields.length < 2 || fields.length > 3) {
        return null;
    }
    if (fields[0] === '') {
        return null;
    }
    return fields;
}

export function parseAdmins(text: string): ParsedAdmins {
    const newline = text.includes('\r\n') ? '\r\n' : '\n';
    const trailingNewline = text.endsWith('\n');
    const lines = text.split(/\r?\n/);
    if (trailingNewline) {
        lines.pop(); // the empty string after the final newline
    }

    const entries: AdminEntry[] = [];
    lines.forEach((line, index) => {
        const fields = quotedFields(line);
        if (!fields) {
            return;
        }
        const [identity, flagField, password] = fields;
        const colon = flagField.indexOf(':');
        const immunityText = colon >= 0 ? flagField.slice(0, colon) : '';
        const immunity = /^\d+$/.test(immunityText) ? Number(immunityText) : null;
        entries.push({
            identity,
            flags: (immunity === null ? flagField : flagField.slice(colon + 1)).trim(),
            immunity,
            password: password ?? '',
            line: index,
        });
    });

    return { lines, entries, trailingNewline, newline };
}

function formatEntry(entry: AdminEntry): string {
    const flagField =
        entry.immunity === null ? entry.flags : `${entry.immunity}:${entry.flags}`;
    // Pad the identity so a hand-maintained file stays readable in a plain
    // editor, the way SourceMod's own example file is laid out.
    const identity = `"${entry.identity}"`.padEnd(24);
    const rest = entry.password.trim() === ''
        ? `"${flagField}"`
        : `"${flagField}"\t"${entry.password}"`;
    return `${identity}${rest}`;
}

/**
 * Writes the edited entries back into the original file: each entry that came
 * from a line replaces that line, deleted entries take their line with them,
 * and new ones are appended. Comments and spacing are untouched.
 */
export function serializeAdmins(parsed: ParsedAdmins, entries: AdminEntry[]): string {
    const byLine = new Map<number, AdminEntry>();
    const added: AdminEntry[] = [];
    for (const entry of entries) {
        if (entry.identity.trim() === '') {
            continue; // a blank row the user never filled in
        }
        if (entry.line === null) {
            added.push(entry);
        } else {
            byLine.set(entry.line, entry);
        }
    }

    const out: string[] = [];
    parsed.lines.forEach((line, index) => {
        const wasEntry = quotedFields(line) !== null;
        if (!wasEntry) {
            out.push(line);
            return;
        }
        const kept = byLine.get(index);
        if (kept) {
            out.push(formatEntry(kept));
        }
        // Dropped entries simply contribute no line.
    });
    for (const entry of added) {
        out.push(formatEntry(entry));
    }

    const text = out.join(parsed.newline);
    return parsed.trailingNewline || text === '' ? `${text}${parsed.newline}` : text;
}
