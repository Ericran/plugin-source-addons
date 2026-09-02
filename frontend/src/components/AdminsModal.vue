<template>
    <GModal
        :show="show"
        :title="trans('admins_title')"
        :style="{ width: '900px' }"
        transform-origin="center"
        @update:show="(value: boolean) => $emit('update:show', value)"
    >
        <Loading v-if="loading" />
        <template v-else>
            <n-tabs v-model:value="tab" type="line" size="small">
                <n-tab-pane name="admins" :tab="trans('admins_tab_admins')">
                    <div class="mb-2 text-xs text-stone-400 dark:text-stone-500 font-mono">
                        {{ adminsPath }}
                    </div>
                    <n-data-table
                        :columns="adminColumns"
                        :data="admins"
                        :row-key="rowKey"
                        :bordered="false"
                        :single-line="true"
                        size="small"
                        :max-height="380"
                        :scroll-x="760"
                    >
                        <template #empty>
                            <n-empty :description="trans('admins_empty')" size="small" class="py-4" />
                        </template>
                    </n-data-table>
                    <GButton color="white" size="small" class="mt-2" @click="addAdmin">
                        <i class="fa-solid fa-plus"></i><span class="ml-1">{{ trans('admins_add') }}</span>
                    </GButton>
                    <div class="mt-2 text-xs text-stone-400 dark:text-stone-500">
                        {{ trans('admins_hint') }}
                    </div>
                </n-tab-pane>
                <n-tab-pane name="groups" :tab="trans('admins_tab_groups')">
                    <div class="mb-2 text-xs text-stone-400 dark:text-stone-500 font-mono">
                        {{ groupsPath }}
                    </div>
                    <n-input
                        v-model:value="groupsText"
                        type="textarea"
                        :rows="14"
                        class="font-mono"
                        placeholder="Groups { }"
                    />
                    <div class="mt-2 text-xs text-stone-400 dark:text-stone-500">
                        {{ trans('admins_groups_hint') }}
                    </div>
                </n-tab-pane>
            </n-tabs>
        </template>

        <template #footer>
            <GButton color="green" :disabled="loading || saving" @click="save">
                <GIcon name="save" class="mr-1" />
                {{ trans('save') }}
            </GButton>
        </template>
    </GModal>
</template>

<script setup lang="ts">
import { computed, h, ref, watch } from 'vue';
import {
    NDataTable,
    NEmpty,
    NInput,
    NInputNumber,
    NTabPane,
    NTabs,
    type DataTableColumns,
} from 'naive-ui';
import { usePluginTrans } from '@gameap/plugin-sdk';

import { fmDownloadText, fmUploadFile } from '../api/gameap';
import { apiErrorMessage } from '../api/plugin';
import { parseAdmins, serializeAdmins, type AdminEntry, type ParsedAdmins } from '../lib/admins';

/** An entry plus a stable table key: the identity is edited, so it cannot be one. */
interface AdminDraft extends AdminEntry {
    key: number;
}

const props = defineProps<{
    show: boolean;
    serverId: number;
    /** Server-dir-relative SourceMod configs dir, e.g. cstrike/addons/sourcemod/configs. */
    configsDir: string;
}>();

const emit = defineEmits<{
    'update:show': [value: boolean];
}>();

const { trans } = usePluginTrans();

const tab = ref<'admins' | 'groups'>('admins');
const admins = ref<AdminDraft[]>([]);
const groupsText = ref('');
const loading = ref(false);
const saving = ref(false);

/** The original file, kept so comments and layout survive a save. */
let parsed: ParsedAdmins = { lines: [], entries: [], trailingNewline: true, newline: '\n' };
let nextKey = 0;

const adminsPath = computed(() => `${props.configsDir}/admins_simple.ini`);
const groupsPath = computed(() => `${props.configsDir}/admins.cfg`);

const rowKey = (row: AdminDraft): number => row.key;

// Editable cells: naive-ui renders them, so the drafts stay plain objects and
// the save path is unchanged.
const adminColumns = computed<DataTableColumns<AdminDraft>>(() => [
    {
        title: trans('admins_col_identity'),
        key: 'identity',
        minWidth: 200,
        render: (row) =>
            h(NInput, {
                value: row.identity,
                size: 'small',
                class: 'font-mono',
                placeholder: 'STEAM_0:1:… / 76561198… / !Name',
                'onUpdate:value': (value: string) => {
                    row.identity = value;
                },
            }),
    },
    {
        title: trans('admins_col_flags'),
        key: 'flags',
        minWidth: 160,
        render: (row) =>
            h(NInput, {
                value: row.flags,
                size: 'small',
                class: 'font-mono',
                placeholder: 'abcdefg / z',
                'onUpdate:value': (value: string) => {
                    row.flags = value;
                },
            }),
    },
    {
        title: trans('admins_col_immunity'),
        key: 'immunity',
        width: 110,
        render: (row) =>
            h(NInputNumber, {
                value: row.immunity,
                size: 'small',
                min: 0,
                max: 100,
                clearable: true,
                showButton: false,
                placeholder: '—',
                'onUpdate:value': (value: number | null) => {
                    row.immunity = value;
                },
            }),
    },
    {
        title: trans('admins_col_password'),
        key: 'password',
        minWidth: 130,
        render: (row) =>
            h(NInput, {
                value: row.password,
                size: 'small',
                class: 'font-mono',
                placeholder: trans('admins_password_optional'),
                'onUpdate:value': (value: string) => {
                    row.password = value;
                },
            }),
    },
    {
        title: '',
        key: 'actions',
        width: 48,
        align: 'center',
        render: (row) =>
            h(
                'button',
                {
                    class: 'text-stone-400 hover:text-red-500',
                    title: trans('action_delete'),
                    onClick: () => removeAdmin(row.key),
                },
                h('i', { class: 'fa-solid fa-trash-can' }),
            ),
    },
]);

watch(
    () => props.show,
    async (shown) => {
        if (!shown) {
            return;
        }
        loading.value = true;
        try {
            parsed = parseAdmins(await downloadOr(adminsPath.value, ''));
            admins.value = parsed.entries.map((entry) => ({ ...entry, key: nextKey++ }));
            groupsText.value = await downloadOr(groupsPath.value, '');
        } finally {
            loading.value = false;
        }
    },
);

async function downloadOr(path: string, fallback: string): Promise<string> {
    try {
        return await fmDownloadText(props.serverId, path);
    } catch {
        return fallback; // missing file = empty config
    }
}

function addAdmin(): void {
    admins.value.push({
        key: nextKey++,
        identity: '',
        flags: 'z',
        immunity: null,
        password: '',
        line: null,
    });
}

function removeAdmin(key: number): void {
    admins.value = admins.value.filter((admin) => admin.key !== key);
}

async function save(): Promise<void> {
    saving.value = true;
    try {
        await uploadTo(adminsPath.value, serializeAdmins(parsed, admins.value));
        if (groupsText.value.trim() !== '') {
            await uploadTo(groupsPath.value, groupsText.value);
        }
        window.$message?.success(trans('admins_saved'));
        emit('update:show', false);
    } catch (error) {
        window.$message?.error(apiErrorMessage(error, trans('op_failed')));
    } finally {
        saving.value = false;
    }
}

async function uploadTo(path: string, content: string): Promise<void> {
    const idx = path.lastIndexOf('/');
    const directory = path.slice(0, idx);
    const name = path.slice(idx + 1);
    await fmUploadFile(props.serverId, directory, new File([content], name, { type: 'text/plain' }));
}
</script>
