<template>
    <GModal
        :show="show"
        :title="trans('catalog_title')"
        :style="{ width: '720px' }"
        transform-origin="center"
        @update:show="(value: boolean) => $emit('update:show', value)"
    >
        <Loading v-if="loading" />
        <div v-else class="flex flex-col gap-2">
            <div
                v-for="entry in entries"
                :key="entry.key"
                class="flex items-start gap-3 p-3 rounded border border-stone-200 dark:border-stone-700"
            >
                <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 flex-wrap">
                        <span class="font-semibold text-stone-800 dark:text-stone-100">
                            {{ entry.name }}
                        </span>
                        <!-- An extension installs files but never gets a row in
                             the plugin table, so say so before the install. -->
                        <span v-if="entry.kind === 'extension'" class="badge-stone !me-0 text-[10px]">
                            {{ trans('catalog_extension') }}
                        </span>
                        <span v-if="isInstalled(entry)" class="badge-green !me-0 text-[10px]">
                            {{ trans('catalog_installed') }}
                        </span>
                        <a
                            class="link !text-xs cursor-pointer inline-flex items-center gap-1"
                            :href="entry.homepage"
                            target="_blank"
                            rel="noopener"
                        >
                            GitHub <GIcon name="external-link" size="sm" />
                        </a>
                    </div>
                    <div class="text-sm text-stone-500 dark:text-stone-400 mt-0.5">
                        {{ entry.description }}
                    </div>
                </div>
                <GButton
                    :color="isInstalled(entry) ? 'white' : 'green'"
                    size="small"
                    :disabled="installing !== null"
                    @click="install(entry)"
                >
                    <GIcon v-if="installing === entry.key" name="refresh" />
                    <i v-else class="fa-solid fa-download"></i>
                    <span class="ml-1">
                        {{
                            installing === entry.key
                                ? trans('catalog_installing')
                                : isInstalled(entry)
                                  ? trans('catalog_reinstall')
                                  : trans('install')
                        }}
                    </span>
                </GButton>
            </div>
            <div class="text-xs text-stone-400 dark:text-stone-500">
                {{ trans('catalog_hint') }}
            </div>
        </div>
    </GModal>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { usePluginTrans } from '@gameap/plugin-sdk';

import { apiErrorMessage, getCatalog, installCatalogEntry } from '../api/plugin';
import type { CatalogEntryInfo } from '../types';

const props = defineProps<{
    show: boolean;
    serverId: number;
    pluginId: string;
    installedPlugins: string[];
}>();

const emit = defineEmits<{
    'update:show': [value: boolean];
    installed: [plugins: string[]];
}>();

const { trans } = usePluginTrans();

const entries = ref<CatalogEntryInfo[]>([]);
const loading = ref(false);
const installing = ref<string | null>(null);

/** Only plugin entries can be spotted on disk; an extension has no row. */
function isInstalled(entry: CatalogEntryInfo): boolean {
    return entry.plugin !== '' && props.installedPlugins.includes(entry.plugin);
}

watch(
    () => props.show,
    async (shown) => {
        if (!shown || entries.value.length > 0) {
            return;
        }
        loading.value = true;
        try {
            entries.value = await getCatalog(props.pluginId, props.serverId);
        } catch (error) {
            window.$message?.error(apiErrorMessage(error, trans('load_failed')));
            emit('update:show', false);
        } finally {
            loading.value = false;
        }
    },
);

async function install(entry: CatalogEntryInfo): Promise<void> {
    installing.value = entry.key;
    try {
        const result = await installCatalogEntry(props.pluginId, props.serverId, entry.key);
        window.$message?.success(
            trans('catalog_installed_toast', { name: entry.name, version: result.version }),
        );
        emit('installed', result.plugins);
    } catch (error) {
        window.$message?.error(apiErrorMessage(error, trans('op_failed')));
    } finally {
        installing.value = null;
    }
}
</script>
