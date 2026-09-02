<template>
    <GModal
        :show="show"
        :title="trans('install_title')"
        :style="{ width: '600px' }"
        transform-origin="center"
        @update:show="onUpdateShow"
    >
        <div class="space-y-3">
            <n-upload
                v-if="!file"
                :default-upload="false"
                :show-file-list="false"
                accept=".smx,.zip,.gz,.tgz"
                @change="onUploadChange"
            >
                <n-upload-dragger>
                    <div class="flex flex-col items-center gap-2 py-6">
                        <GIcon name="upload" class="text-4xl text-stone-400" />
                        <p class="text-stone-700 dark:text-stone-300 font-medium">
                            {{ trans('drop_hint') }}
                        </p>
                        <p class="text-sm text-stone-500 dark:text-stone-500">
                            {{ trans('file_hint') }}
                        </p>
                    </div>
                </n-upload-dragger>
            </n-upload>

            <template v-else>
                <div
                    class="flex items-center gap-3 p-3 rounded border border-stone-200 dark:border-stone-700 bg-stone-50 dark:bg-stone-900"
                >
                    <GIcon name="file-code" size="lg" class="text-stone-400" />
                    <div class="min-w-0 flex-1">
                        <div class="font-mono text-sm text-stone-800 dark:text-stone-100 truncate">
                            {{ file.name }}
                        </div>
                        <div class="text-xs text-stone-400">{{ prettySize }}</div>
                    </div>
                    <button
                        v-if="!uploading"
                        class="text-stone-400 hover:text-stone-600 dark:hover:text-stone-200"
                        @click="file = null"
                    >
                        <GIcon name="xmark" />
                    </button>
                </div>

                <n-progress
                    v-if="uploading"
                    type="line"
                    :percentage="progress"
                    :show-indicator="false"
                    :height="8"
                    :border-radius="4"
                    processing
                />
            </template>

            <n-alert v-if="validationError" type="warning" :show-icon="true">
                {{ validationError }}
            </n-alert>

            <n-alert v-if="isSourceFile" type="warning" :show-icon="true">
                {{ trans('wrong_type_sp') }}
            </n-alert>

            <div
                v-if="isOverwrite && !validationError"
                class="border border-orange-300 dark:border-orange-800 rounded p-3 bg-orange-50 dark:bg-orange-950/40"
            >
                <div class="flex items-center gap-2">
                    <GIcon name="warning" class="text-orange-500" />
                    <strong class="text-orange-700 dark:text-orange-300">
                        {{ trans('overwrite_title') }}
                    </strong>
                </div>
                <p class="mt-1 text-sm text-orange-700 dark:text-orange-300">
                    {{ trans('overwrite_text') }}
                </p>
            </div>

            <div class="flex items-center justify-end flex-wrap gap-2">
                <div class="text-xs text-stone-400 dark:text-stone-500 font-mono">→ {{ targetPath }}</div>
            </div>
        </div>

        <template #footer>
            <GButton
                :color="isOverwrite ? 'orange' : 'green'"
                :disabled="!file || Boolean(validationError) || uploading"
                @click="install"
            >
                <GIcon :name="uploading ? 'spinner' : isOverwrite ? 'refresh' : 'download'" class="mr-1" />
                {{
                    uploading
                        ? trans('uploading')
                        : isOverwrite
                          ? trans('overwrite')
                          : trans('install')
                }}
            </GButton>
        </template>
    </GModal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NAlert, NProgress, NUpload, NUploadDragger } from 'naive-ui';
import type { UploadFileInfo } from 'naive-ui';
import { usePluginTrans } from '@gameap/plugin-sdk';

import { fmUploadFile } from '../api/gameap';
import { apiErrorMessage, installArchive, registerPlugin } from '../api/plugin';
import { httpStatus } from '../lib/http-error';
import { fileStem, prettyName } from '../lib/naming';
import type { StatePaths } from '../types';

const props = defineProps<{
    show: boolean;
    serverId: number;
    pluginId: string;
    paths: StatePaths;
    existingNames: string[];
}>();

const emit = defineEmits<{
    'update:show': [value: boolean];
    installed: [replaced: boolean];
}>();

const { trans } = usePluginTrans();

const file = ref<File | null>(null);
const uploading = ref(false);
const progress = ref(0);

watch(
    () => props.show,
    (shown) => {
        if (shown) {
            file.value = null;
            uploading.value = false;
            progress.value = 0;
        }
    },
);

/** Archive extensions the backend's extractor understands. */
function isArchiveName(name: string): boolean {
    const lower = name.toLowerCase();
    return lower.endsWith('.zip') || lower.endsWith('.tar.gz') || lower.endsWith('.tgz');
}

const isSmx = computed(() => Boolean(file.value) && /\.smx$/i.test(file.value!.name));
const isArchive = computed(() => Boolean(file.value) && isArchiveName(file.value!.name));

/** A .sp is SourcePawn source, not a plugin — a common upload mistake, and one
 * worth naming rather than rejecting as an unspecified wrong type. */
const isSourceFile = computed(() => Boolean(file.value) && /\.sp$/i.test(file.value!.name));

const validationError = computed(() => {
    if (!file.value) {
        return null;
    }
    if (isSourceFile.value) {
        return trans('wrong_type');
    }
    return isSmx.value || isArchive.value ? null : trans('wrong_type');
});

/** SourceMod loads plugins/<name>.smx — the plugin name is the file stem. */
const pluginName = computed(() => (file.value && isSmx.value ? fileStem(file.value.name) : null));

/** The picked .smx matches an already known plugin. Archives learn this
 * server-side (the backend answers 409 and we ask before retrying). */
const isOverwrite = computed(() => {
    const name = pluginName.value?.toLowerCase();
    if (!name) {
        return false;
    }
    return props.existingNames.some((existing) => existing.toLowerCase() === name);
});

const targetPath = computed(() =>
    isSmx.value
        ? `${props.paths.sm_plugins_dir}/${file.value?.name ?? '…'}`
        : `${props.paths.sm_plugins_dir}/`,
);

const prettySize = computed(() => {
    if (!file.value) {
        return '';
    }
    const size = file.value.size;
    if (size < 1024) {
        return `${size} B`;
    }
    if (size < 1024 * 1024) {
        return `${Math.round(size / 1024)} KB`;
    }
    return `${(size / 1024 / 1024).toFixed(1)} MB`;
});

function onUploadChange(payload: { file: UploadFileInfo }): void {
    file.value = payload.file.file ?? null;
}

function onUpdateShow(value: boolean): void {
    if (!uploading.value) {
        emit('update:show', value);
    }
}

async function install(): Promise<void> {
    const picked = file.value;
    if (!picked || validationError.value) {
        return;
    }
    if (isArchive.value) {
        await installArchiveFile(picked);
        return;
    }
    const name = pluginName.value;
    if (!name) {
        return;
    }
    const replaced = isOverwrite.value;
    uploading.value = true;
    progress.value = 0;
    try {
        await fmUploadFile(props.serverId, props.paths.sm_plugins_dir, picked, (percent) => {
            progress.value = percent;
        });
        await registerPlugin(props.pluginId, props.serverId, { name, force: replaced });
        window.$message?.success(
            trans(replaced ? 'updated_toast' : 'installed_toast', { name: prettyName(name) }),
        );
        emit('installed', replaced);
        emit('update:show', false);
    } catch (error) {
        window.$message?.error(apiErrorMessage(error, trans('op_failed')));
    } finally {
        uploading.value = false;
    }
}

/** Archive flow: file-manager upload (no size squeeze), then the backend
 * unpacks, detects the layout and registers whatever plugins it installed. */
async function installArchiveFile(picked: File): Promise<void> {
    uploading.value = true;
    progress.value = 0;
    const archivePath = `${props.paths.sm_dir}/${picked.name}`;
    try {
        await fmUploadFile(props.serverId, props.paths.sm_dir, picked, (percent) => {
            progress.value = percent;
        });
        await finishArchiveInstall(archivePath, false);
    } catch (error) {
        if (httpStatus(error) === 409) {
            confirmArchiveOverwrite(archivePath, apiErrorMessage(error, ''));
            return; // uploading is reset by the dialog path
        }
        window.$message?.error(apiErrorMessage(error, trans('op_failed')));
        uploading.value = false;
    }
}

function confirmArchiveOverwrite(archivePath: string, detail: string): void {
    window.$dialog?.warning({
        title: trans('overwrite_title'),
        content: detail || trans('overwrite_text'),
        positiveText: trans('overwrite'),
        negativeText: trans('no'),
        onPositiveClick: async () => {
            try {
                await finishArchiveInstall(archivePath, true);
            } catch (error) {
                window.$message?.error(apiErrorMessage(error, trans('op_failed')));
                uploading.value = false;
            }
        },
        onNegativeClick: () => {
            uploading.value = false;
        },
        onClose: () => {
            uploading.value = false;
        },
    });
}

async function finishArchiveInstall(archivePath: string, force: boolean): Promise<void> {
    const result = await installArchive(props.pluginId, props.serverId, archivePath, force);
    window.$message?.success(
        result.plugins.length > 0
            ? trans('archive_installed_toast', {
                  plugins: result.plugins.join(', '),
                  count: result.files_written,
              })
            : trans('archive_installed_no_plugins', { count: result.files_written }),
    );
    uploading.value = false;
    emit('installed', force);
    emit('update:show', false);
}
</script>
