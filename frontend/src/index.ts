import type { PluginDefinition } from '@gameap/plugin-sdk';

import ModsTab from './components/ModsTab.vue';

/**
 * Stock GameAP game codes running Source 1.
 *
 * Listed explicitly rather than gated on `engines: ['Source']`, because the
 * panel's tab gate is engines OR codes with no version component, and the
 * engine string alone is too coarse in both directions: `cs2` is also
 * engine "source" (Source 2, handled by the CS2 Addons plugin), and `ark` is
 * mislabeled "Source" in the stock game list while being Unreal Engine.
 *
 * Kept in step with SOURCE_GAME_CODES in src/maintenance.rs. To use this
 * plugin with a custom Source 1 game entry, add its code here and rebuild —
 * the backend gate is version-based and already accepts it.
 */
const SOURCE_GAME_CODES = [
    'cssource',
    'cssv34',
    'csgo',
    'dods',
    'garrysmod',
    'hl2mp',
    'l4d',
    'l4d2',
    'tf2',
    'bms',
    'synergy',
];

export const sourceAddonsPlugin: PluginDefinition = {
    // Must match src/lib.rs PLUGIN_ID and survive the panel's CompactPluginID
    // normalization (valid base32, a-z2-7): base32("srcaddon").
    id: 'onzggylemrxw4',
    name: 'Source Addons',
    version: '0.1.2',
    apiVersion: '1.0',
    description: 'Manage Metamod:Source and SourceMod plugins on Source engine servers',
    author: 'SilverSasquatchGameAPDev',

    translations: {
        en: {
            tab_label: 'Plugins',
            'abilities.manage': 'Manage Source addons (Metamod:Source / SourceMod)',

            metamod_desc:
                'Modification layer for the Source engine. Loads binary plugins and is required by SourceMod.',
            sm_desc:
                'SourcePawn scripting platform on top of Metamod:Source: admin tools, map votes, game modes and thousands of community plugins.',
            status_not_installed: 'Not installed',
            status_not_active: 'Not active',
            not_active_hint:
                'The addons/metamod directory exists, but nothing loads it: there is no addons/metamod.vdf and no gameinfo.txt search path.',
            sm_not_registered_hint:
                'SourceMod is unpacked but not registered with Metamod, so it never starts. Its addons/metamod/sourcemod.vdf entry is missing or switched off.',
            loader_vdf: 'Loaded by the engine through addons/metamod.vdf.',
            loader_gameinfo: 'Loaded through the gameinfo.txt search path.',
            loader_both:
                'Loaded through both addons/metamod.vdf and the gameinfo.txt search path — one is enough; remove either if Metamod reports loading twice.',
            version_unknown: 'version unknown',
            stats_total: 'Plugins',
            stats_enabled: 'Enabled',
            stats_errors: 'Errors',
            install_hint_metamod:
                'Install Metamod:Source into addons/metamod; the loader file is written for you.',
            install_hint_sm: 'Install SourceMod into addons/sourcemod.',

            rcon_unavailable_offline:
                'Server is offline — console versions and statuses are unavailable.',
            rcon_unavailable_norcon:
                'RCON is not configured — console versions and statuses are unavailable.',
            rcon_unavailable_error: 'Failed to query the server console.',
            rcon_unavailable_badpass:
                'Wrong RCON password — console versions and statuses are unavailable. Check the password in the server settings.',
            rcon_unavailable_empty:
                'The server console returned an empty response — versions and statuses are unavailable.',
            rcon_password_missing:
                'RCON cannot work: the launch parameters set no rcon_password. Add +rcon_password in Launch Settings and restart.',
            rcon_metamod_not_loaded:
                'The console works, but Metamod is not loaded in the running server — restart the server to load it.',
            rcon_sm_not_loaded:
                'Metamod is loaded but SourceMod is not, so the console does not know the sm command. Live statuses and hot load/unload are unavailable. Check that addons/metamod/sourcemod.vdf is enabled and restart the server.',

            upload_file: 'Upload file',
            search_placeholder: 'Search by name, file, author…',
            filter_all: 'All statuses',
            filter_on: 'Enabled',
            filter_off: 'Disabled',
            filter_err: 'With errors',
            selected: 'Selected: :count',
            bulk_enable: 'Enable',
            bulk_disable: 'Disable',
            bulk_delete: 'Delete',

            col_plugin: 'Plugin',
            col_version: 'Version',
            col_enabled: 'On',
            col_actions: 'Actions',

            status_running: 'Running',
            status_paused: 'Paused',
            status_enabled: 'Enabled',
            status_stopped: 'Stopped',
            status_pending: 'Awaiting load',
            status_error: 'Error',
            status_missing: 'File missing',

            action_config: 'Config',
            action_delete: 'Delete',
            action_unload: 'Unload',
            action_load: 'Load',
            action_reload: 'Reload',
            action_unpause: 'Unpause',
            group_other: 'Other',
            comment_add: 'add comment',
            comment_edit: 'Edit comment',
            comment_placeholder: 'Comment…',
            comment_saved: 'Comment for ":name" saved',

            empty_no_plugins: 'No plugins installed',
            empty_no_results: 'Nothing found — adjust the search',
            install_first: 'Install the first plugin',
            open_in_filemanager: 'open in file manager',

            sm_missing: 'SourceMod is not installed',
            platform_missing_hint:
                'Install the platform on the server — the plugin list and file upload will appear here.',
            nothing_installed_title: 'Metamod:Source and SourceMod are not installed',
            nothing_installed_text:
                'Install Metamod:Source and SourceMod on the server to manage plugins from the panel: enable, disable and upload your own files.',
            not_source1:
                'This tab is available only for Source 1 servers (Counter-Strike: Source, Garry’s Mod, Team Fortress 2, Left 4 Dead 2, …). Counter-Strike 2 is handled by the CS2 Addons plugin.',

            loading: 'Loading…',
            load_failed: 'Failed to load the plugins state',
            retry: 'Retry',

            delete_title: 'Delete plugin ":name"?',
            delete_text:
                'The .smx file is removed. Its configs, translations and gamedata are kept.',
            bulk_delete_title: 'Delete selected plugins (:count)?',
            bulk_delete_text: 'The .smx files are removed; configs are kept.',
            yes: 'Yes',
            no: 'No',

            toggled_on: 'Plugin ":name" enabled — load it or restart the server',
            toggled_off: 'Plugin ":name" disabled — applies after restart or unload',
            unloaded_ok: 'Plugin ":name" unloaded',
            loaded_ok: 'Plugin ":name" loaded',
            reloaded_ok: 'Plugin ":name" reloaded',
            unpaused_ok: 'Plugin ":name" resumed',
            unload_failed: 'Failed to unload ":name"',
            load_failed_named: 'Failed to load ":name"',
            deleted: 'Plugin ":name" deleted',
            bulk_enabled: 'Plugins enabled: :count',
            bulk_disabled: 'Plugins disabled: :count',
            bulk_deleted: 'Plugins deleted: :count',
            installed_toast: 'Plugin ":name" installed from file',
            op_failed: 'Operation failed',

            install_title: 'Install plugin — SourceMod',
            drop_hint: 'Drop a file here or click to choose',
            file_hint:
                'A compiled .smx plugin, or a whole release .zip / .tar.gz — the archive layout is detected and unpacked to the right place automatically.',
            wrong_type: 'A .smx, .zip or .tar.gz file is required',
            wrong_type_sp:
                'This is SourcePawn source, not a plugin. Compile it to .smx first (addons/sourcemod/scripting/compile.sh, or the online compiler) and upload the result.',
            archive_installed_toast: 'Installed from archive: :plugins (:count files)',
            archive_installed_no_plugins:
                'Installed from archive: :count files, no new plugin (an extension or a data-only package)',
            install: 'Install',
            uploading: 'Uploading…',
            overwrite: 'Overwrite',
            overwrite_title: 'Plugin already installed',
            overwrite_text:
                'The existing .smx will be overwritten with the new version; comments and configs stay in place.',
            updated_toast: 'Plugin ":name" updated from file',

            config_title: 'Configuration — :name',
            config_hint:
                'SourceMod generates these from the plugin’s cvars. Changes apply on map change, or immediately with Reload.',
            save: 'Save',
            config_saved: 'Configuration saved',
            config_load_failed: 'Failed to load the config',

            toolbar_catalog: 'Catalog',
            toolbar_snapshots: 'Snapshots',
            toolbar_admins: 'Admins',
            toolbar_logs: 'Logs',
            toolbar_history: 'History',

            update_available: 'v:version available',
            update_available_hint: 'A newer release is available upstream',

            repair_loader: 'Fix loader',
            loader_repaired: 'addons/metamod.vdf written — restart the server to load Metamod',
            loader_already_ok: 'Metamod is already wired in — nothing to fix',

            mm_plugins: 'Metamod plugins',
            mm_hint:
                'Toggling renames the .vdf or comments the metaplugins.ini line; applies on the next restart.',
            mm_badge_vdf: 'vdf',
            mm_badge_ini: 'ini',
            mm_platform_badge: 'platform',
            mm_platform_title: 'Disable SourceMod itself?',
            mm_platform_text:
                'This entry registers SourceMod with Metamod. Switching it off unloads the whole platform at the next restart: every plugin stops and this tab loses live statuses and hot load/unload. Disable it anyway?',
            mm_enabled: 'Metamod plugin ":name" enabled — applies after restart',
            mm_disabled: 'Metamod plugin ":name" disabled — applies after restart',

            platform_install: 'Install latest',
            platform_installing: 'Installing…',
            platform_update: 'Update to v:version',
            platform_install_title: 'Install :name?',
            platform_install_text:
                'The latest build is downloaded and unpacked on the server (existing files are overwritten). This can take a minute.',
            platform_installed: ':name :version installed — restart the server to load it',

            restart_pending: 'Changes are waiting for a server restart to take effect.',
            restart_now: 'Restart server',
            restart_title: 'Restart the server?',
            restart_text: 'Players online will be disconnected.',
            restart_sent: 'Restart requested',

            catalog_title: 'Plugin catalog',
            catalog_installed: 'installed',
            catalog_extension: 'extension',
            catalog_installing: 'Installing…',
            catalog_reinstall: 'Reinstall',
            catalog_hint:
                'Installs the latest GitHub release. Most of the SourceMod ecosystem lives on the AlliedModders forums with no version feed — install those through Upload, which takes the same archives.',
            catalog_installed_toast: ':name :version installed — load it or restart the server',

            snapshots_title: 'Plugin setup snapshots',
            snapshot_create: 'Create snapshot',
            snapshots_retention:
                'plugins, configs, data, gamedata, translations, extensions and cfg/sourcemod are archived; the 5 newest are kept.',
            snapshots_empty: 'No snapshots yet',
            snapshot_download: 'download',
            snapshot_restore: 'Restore',
            snapshot_created: 'Snapshot created',
            snapshot_restore_title: 'Restore this snapshot?',
            snapshot_restore_text:
                'Plugins, configs and data are replaced with the state from :date. Files added since then are removed.',
            snapshot_restored: 'Snapshot restored — restart the server to apply',
            snapshots_transfer_hint:
                'To copy a setup between servers: download a snapshot here, upload the .tar into the other server’s backups folder via its file manager, then restore it there.',

            admins_title: 'SourceMod admins',
            admins_tab_admins: 'Admins (admins_simple.ini)',
            admins_tab_groups: 'Groups & overrides (admins.cfg)',
            admins_col_identity: 'Identity',
            admins_col_flags: 'Flags',
            admins_col_immunity: 'Immunity',
            admins_col_password: 'Password',
            admins_password_optional: 'optional',
            admins_add: 'Add admin',
            admins_empty: 'No admins yet',
            admins_hint:
                'Identity is a Steam ID (STEAM_0:1:… or 76561198…), an IP, or a name prefixed with !. Flags are the letters from admin_levels.cfg — z is root. Comments in the file are preserved. Changes apply on map change or sm_reloadadmins.',
            admins_groups_hint:
                'Raw KeyValues: admin groups and command overrides. Left untouched when empty.',
            admins_saved: 'Admin configuration saved',

            logs_title: 'SourceMod log',
            logs_kind_errors: 'Errors',
            logs_kind_general: 'General',
            logs_filter_placeholder: 'Filter lines (plugin name, "native"…)',
            logs_empty: 'No log lines — the server has not written this log yet',
            logs_empty_errors: 'No errors logged — nothing has gone wrong yet',
            logs_follow: 'Follow',
            logs_download: 'download',

            audit_title: 'Recent panel actions',
            audit_empty: 'Nothing recorded yet',

            update_all: 'Update all (:count)',
            update_all_title: 'Update :count plugins?',
            update_all_text:
                'Each plugin is reinstalled from its latest GitHub release. A snapshot is taken automatically first.',
            update_all_done: 'Plugins updated: :count — restart or reload them',
            update_all_partial: 'Updated :count, failed: :failed',

            toolbar_doctor: 'Doctor',
            doctor_title: 'Setup health check',
            doctor_recheck: 'Re-check',
            doctor_all_ok: 'Everything looks healthy',
            doctor_summary: ':fails failed, :warns warnings',
            doctor_rconpass_ok: 'rcon_password is set in the launch parameters',
            doctor_rconpass_missing:
                'The launch parameters set no rcon_password; RCON cannot work without one',
            doctor_rcon_ok: 'Console reachable, live data flowing',
            doctor_sm_loaded_ok: 'Loaded in the running server and answering sm commands',
            doctor_sm_not_loaded:
                'Installed on disk but not loaded in the running server — every row below shows its file state only. Check addons/metamod/sourcemod.vdf and restart.',
            doctor_check_rconpass: 'Launch parameters',
            doctor_check_rcon: 'RCON console',
            doctor_check_smloaded: 'SourceMod loaded',
            doctor_check_metamod: 'Metamod:Source',
            doctor_check_loader: 'Metamod loader',
            doctor_check_loaderbin: 'Loader binary',
            doctor_check_sourcemod: 'SourceMod',
            doctor_check_smbin: 'SourceMod binaries',
            doctor_check_smregistered: 'SourceMod registration',
            doctor_check_duplicates: 'Enabled/disabled conflicts',
            doctor_check_orphans: 'Manifest consistency',
            doctor_check_cfgdir: 'Generated configs',
            doctor_check_scripting: 'Uncompiled sources',
            doctor_check_loadfail: 'Plugin load failures',
            doctor_check_vdf: 'Metamod plugin aliases',
            doctor_check_metaplugins: 'metaplugins.ini entries',
            doctor_check_scratch: 'Download leftovers',
        },
        ru: {
            tab_label: 'Плагины',
            'abilities.manage': 'Управление Source-аддонами (Metamod:Source / SourceMod)',

            metamod_desc:
                'Слой модификаций для движка Source. Загружает бинарные плагины и требуется для работы SourceMod.',
            sm_desc:
                'Платформа скриптов SourcePawn поверх Metamod:Source: администрирование, голосования за карты, игровые режимы и тысячи плагинов сообщества.',
            status_not_installed: 'Не установлен',
            status_not_active: 'Не активен',
            not_active_hint:
                'Каталог addons/metamod найден, но его никто не загружает: нет ни addons/metamod.vdf, ни пути в gameinfo.txt.',
            sm_not_registered_hint:
                'SourceMod распакован, но не зарегистрирован в Metamod и потому не запускается. Файл addons/metamod/sourcemod.vdf отсутствует или выключен.',
            loader_vdf: 'Загружается движком через addons/metamod.vdf.',
            loader_gameinfo: 'Загружается через путь поиска в gameinfo.txt.',
            loader_both:
                'Загружается и через addons/metamod.vdf, и через gameinfo.txt — достаточно одного; удалите любой, если Metamod загружается дважды.',
            version_unknown: 'версия неизвестна',
            stats_total: 'Плагинов',
            stats_enabled: 'Включено',
            stats_errors: 'Ошибок',
            install_hint_metamod:
                'Установите Metamod:Source в addons/metamod — файл загрузчика будет создан автоматически.',
            install_hint_sm: 'Установите SourceMod в addons/sourcemod.',

            rcon_unavailable_offline: 'Сервер офлайн — версии и статусы из консоли недоступны.',
            rcon_unavailable_norcon: 'RCON не настроен — версии и статусы из консоли недоступны.',
            rcon_unavailable_error: 'Не удалось опросить консоль сервера.',
            rcon_unavailable_badpass:
                'Неверный RCON-пароль — версии и статусы из консоли недоступны. Проверьте пароль в настройках сервера.',
            rcon_unavailable_empty:
                'Консоль сервера вернула пустой ответ — версии и статусы недоступны.',
            rcon_password_missing:
                'RCON не может работать: в параметрах запуска нет rcon_password. Добавьте +rcon_password в настройках запуска и перезапустите сервер.',
            rcon_metamod_not_loaded:
                'Консоль работает, но Metamod не загружен на запущенном сервере — перезапустите сервер.',
            rcon_sm_not_loaded:
                'Metamod загружен, а SourceMod — нет, поэтому консоль не знает команду sm. Живые статусы и горячая загрузка недоступны. Проверьте, что addons/metamod/sourcemod.vdf включён, и перезапустите сервер.',

            upload_file: 'Загрузить файл',
            search_placeholder: 'Поиск по названию, файлу, автору…',
            filter_all: 'Все статусы',
            filter_on: 'Включённые',
            filter_off: 'Выключенные',
            filter_err: 'С ошибками',
            selected: 'Выбрано: :count',
            bulk_enable: 'Включить',
            bulk_disable: 'Выключить',
            bulk_delete: 'Удалить',

            col_plugin: 'Плагин',
            col_version: 'Версия',
            col_enabled: 'Вкл.',
            col_actions: 'Действия',

            status_running: 'Работает',
            status_paused: 'Приостановлен',
            status_enabled: 'Включен',
            status_stopped: 'Остановлен',
            status_pending: 'Ждёт загрузки',
            status_error: 'Ошибка',
            status_missing: 'Файл отсутствует',

            action_config: 'Конфиг',
            action_delete: 'Удалить',
            action_unload: 'Выгрузить',
            action_load: 'Загрузить',
            action_reload: 'Перезагрузить',
            action_unpause: 'Возобновить',
            group_other: 'Прочее',
            comment_add: 'добавить комментарий',
            comment_edit: 'Изменить комментарий',
            comment_placeholder: 'Комментарий…',
            comment_saved: 'Комментарий для «:name» сохранён',

            empty_no_plugins: 'Плагины не установлены',
            empty_no_results: 'Ничего не найдено — измените условия поиска',
            install_first: 'Установить первый плагин',
            open_in_filemanager: 'открыть в файловом менеджере',

            sm_missing: 'SourceMod не установлен',
            platform_missing_hint:
                'Установите платформу на сервере — после этого здесь появится список плагинов и загрузка файлов.',
            nothing_installed_title: 'Metamod:Source и SourceMod не установлены',
            nothing_installed_text:
                'Установите Metamod:Source и SourceMod на сервер — и управляйте плагинами прямо из панели: включение, выключение и загрузка своих файлов.',
            not_source1:
                'Вкладка доступна только для серверов на движке Source 1 (Counter-Strike: Source, Garry’s Mod, Team Fortress 2, Left 4 Dead 2 и др.). Counter-Strike 2 обслуживает плагин CS2 Addons.',

            loading: 'Загрузка…',
            load_failed: 'Не удалось загрузить состояние плагинов',
            retry: 'Повторить',

            delete_title: 'Удалить плагин «:name»?',
            delete_text:
                'Файл .smx будет удалён. Конфиги, переводы и gamedata сохранятся.',
            bulk_delete_title: 'Удалить выбранные плагины (:count)?',
            bulk_delete_text: 'Файлы .smx удаляются; конфигурация сохраняется.',
            yes: 'Да',
            no: 'Нет',

            toggled_on: 'Плагин «:name» включён — загрузите его или перезапустите сервер',
            toggled_off: 'Плагин «:name» выключен — применится после перезапуска или выгрузки',
            unloaded_ok: 'Плагин «:name» выгружен',
            loaded_ok: 'Плагин «:name» загружен',
            reloaded_ok: 'Плагин «:name» перезагружен',
            unpaused_ok: 'Плагин «:name» возобновлён',
            unload_failed: 'Не удалось выгрузить «:name»',
            load_failed_named: 'Не удалось загрузить «:name»',
            deleted: 'Плагин «:name» удалён',
            bulk_enabled: 'Включено плагинов: :count',
            bulk_disabled: 'Выключено плагинов: :count',
            bulk_deleted: 'Удалено плагинов: :count',
            installed_toast: 'Плагин «:name» установлен из файла',
            op_failed: 'Операция не выполнена',

            install_title: 'Установка плагина — SourceMod',
            drop_hint: 'Перетащите файл сюда или нажмите для выбора',
            file_hint:
                'Скомпилированный .smx или целый релизный архив .zip / .tar.gz — раскладка архива определяется и распаковывается куда нужно автоматически.',
            wrong_type: 'Нужен файл .smx, .zip или .tar.gz',
            wrong_type_sp:
                'Это исходник SourcePawn, а не плагин. Сначала скомпилируйте его в .smx (addons/sourcemod/scripting/compile.sh или онлайн-компилятор) и загрузите результат.',
            archive_installed_toast: 'Установлено из архива: :plugins (файлов: :count)',
            archive_installed_no_plugins:
                'Установлено из архива: файлов :count, новых плагинов нет (расширение или пакет данных)',
            install: 'Установить',
            uploading: 'Загрузка…',
            overwrite: 'Перезаписать',
            overwrite_title: 'Плагин уже установлен',
            overwrite_text:
                'Существующий .smx будет перезаписан новой версией; комментарии и конфиги сохранятся.',
            updated_toast: 'Плагин «:name» обновлён из файла',

            config_title: 'Конфигурация — :name',
            config_hint:
                'SourceMod создаёт эти файлы из cvar-переменных плагина. Изменения применяются на смене карты или сразу по кнопке «Перезагрузить».',
            save: 'Сохранить',
            config_saved: 'Конфигурация сохранена',
            config_load_failed: 'Не удалось загрузить конфиг',

            toolbar_catalog: 'Каталог',
            toolbar_snapshots: 'Снапшоты',
            toolbar_admins: 'Админы',
            toolbar_logs: 'Логи',
            toolbar_history: 'История',

            update_available: 'доступна v:version',
            update_available_hint: 'Вышла более новая версия',

            repair_loader: 'Починить загрузчик',
            loader_repaired: 'addons/metamod.vdf создан — перезапустите сервер',
            loader_already_ok: 'Metamod уже подключён — чинить нечего',

            mm_plugins: 'Плагины Metamod',
            mm_hint:
                'Переключение переименовывает .vdf или комментирует строку в metaplugins.ini; применится после перезапуска.',
            mm_badge_vdf: 'vdf',
            mm_badge_ini: 'ini',
            mm_platform_badge: 'платформа',
            mm_platform_title: 'Отключить сам SourceMod?',
            mm_platform_text:
                'Эта запись регистрирует SourceMod в Metamod. Её отключение выгрузит всю платформу при следующем перезапуске: все плагины остановятся, а вкладка потеряет живые статусы и горячую загрузку. Всё равно отключить?',
            mm_enabled: 'Metamod-плагин «:name» включён — применится после перезапуска',
            mm_disabled: 'Metamod-плагин «:name» выключен — применится после перезапуска',

            platform_install: 'Установить последнюю',
            platform_installing: 'Установка…',
            platform_update: 'Обновить до v:version',
            platform_install_title: 'Установить :name?',
            platform_install_text:
                'Последняя сборка будет скачана и распакована на сервере (существующие файлы перезаписываются). Это может занять минуту.',
            platform_installed: ':name :version установлен — перезапустите сервер',

            restart_pending: 'Изменения ждут перезапуска сервера.',
            restart_now: 'Перезапустить',
            restart_title: 'Перезапустить сервер?',
            restart_text: 'Игроки на сервере будут отключены.',
            restart_sent: 'Перезапуск запрошен',

            catalog_title: 'Каталог плагинов',
            catalog_installed: 'установлен',
            catalog_extension: 'расширение',
            catalog_installing: 'Установка…',
            catalog_reinstall: 'Переустановить',
            catalog_hint:
                'Устанавливается последний релиз с GitHub. Большая часть экосистемы SourceMod живёт на форуме AlliedModders без ленты версий — такие плагины ставьте через «Загрузить файл», он принимает те же архивы.',
            catalog_installed_toast: ':name :version установлен — загрузите его или перезапустите сервер',

            snapshots_title: 'Снапшоты набора плагинов',
            snapshot_create: 'Создать снапшот',
            snapshots_retention:
                'Архивируются plugins, configs, data, gamedata, translations, extensions и cfg/sourcemod; хранятся 5 последних.',
            snapshots_empty: 'Снапшотов пока нет',
            snapshot_download: 'скачать',
            snapshot_restore: 'Восстановить',
            snapshot_created: 'Снапшот создан',
            snapshot_restore_title: 'Восстановить этот снапшот?',
            snapshot_restore_text:
                'Плагины, конфиги и данные будут заменены состоянием от :date. Файлы, добавленные позже, будут удалены.',
            snapshot_restored: 'Снапшот восстановлен — перезапустите сервер',
            snapshots_transfer_hint:
                'Чтобы перенести набор на другой сервер: скачайте снапшот, загрузите .tar в папку backups другого сервера через его файловый менеджер и восстановите там.',

            admins_title: 'Админы SourceMod',
            admins_tab_admins: 'Админы (admins_simple.ini)',
            admins_tab_groups: 'Группы и переопределения (admins.cfg)',
            admins_col_identity: 'Идентификатор',
            admins_col_flags: 'Флаги',
            admins_col_immunity: 'Иммунитет',
            admins_col_password: 'Пароль',
            admins_password_optional: 'необязательно',
            admins_add: 'Добавить админа',
            admins_empty: 'Администраторов пока нет',
            admins_hint:
                'Идентификатор — это Steam ID (STEAM_0:1:… или 76561198…), IP или имя с префиксом !. Флаги — буквы из admin_levels.cfg, z означает root. Комментарии в файле сохраняются. Применяется на смене карты или по sm_reloadadmins.',
            admins_groups_hint:
                'Формат KeyValues: группы админов и переопределения команд. Пустое поле ничего не перезаписывает.',
            admins_saved: 'Настройки админов сохранены',

            logs_title: 'Лог SourceMod',
            logs_kind_errors: 'Ошибки',
            logs_kind_general: 'Общий',
            logs_filter_placeholder: 'Фильтр строк (имя плагина, «native»…)',
            logs_empty: 'Строк нет — сервер ещё не писал этот лог',
            logs_empty_errors: 'Ошибок нет — пока всё в порядке',
            logs_follow: 'Следить',
            logs_download: 'скачать',

            audit_title: 'Последние действия в панели',
            audit_empty: 'Пока ничего не записано',

            update_all: 'Обновить все (:count)',
            update_all_title: 'Обновить плагины (:count)?',
            update_all_text:
                'Каждый плагин переустанавливается из последнего релиза на GitHub. Перед этим автоматически создаётся снапшот.',
            update_all_done: 'Обновлено плагинов: :count — перезапустите или перезагрузите их',
            update_all_partial: 'Обновлено: :count, не удалось: :failed',

            toolbar_doctor: 'Диагностика',
            doctor_title: 'Проверка состояния',
            doctor_recheck: 'Проверить снова',
            doctor_all_ok: 'Всё в порядке',
            doctor_summary: 'ошибок: :fails, предупреждений: :warns',
            doctor_rconpass_ok: 'rcon_password задан в параметрах запуска',
            doctor_rconpass_missing:
                'В параметрах запуска нет rcon_password; без него RCON не работает',
            doctor_rcon_ok: 'Консоль доступна, данные идут',
            doctor_sm_loaded_ok: 'Загружен на запущенном сервере, отвечает на команды sm',
            doctor_sm_not_loaded:
                'Установлен на диске, но не загружен на запущенном сервере — строки ниже показывают только состояние файлов. Проверьте addons/metamod/sourcemod.vdf и перезапустите сервер.',
            doctor_check_rconpass: 'Параметры запуска',
            doctor_check_rcon: 'Консоль RCON',
            doctor_check_smloaded: 'SourceMod загружен',
            doctor_check_metamod: 'Metamod:Source',
            doctor_check_loader: 'Загрузчик Metamod',
            doctor_check_loaderbin: 'Бинарник загрузчика',
            doctor_check_sourcemod: 'SourceMod',
            doctor_check_smbin: 'Бинарники SourceMod',
            doctor_check_smregistered: 'Регистрация SourceMod',
            doctor_check_duplicates: 'Конфликты вкл/выкл',
            doctor_check_orphans: 'Согласованность манифеста',
            doctor_check_cfgdir: 'Генерируемые конфиги',
            doctor_check_scripting: 'Нескомпилированные исходники',
            doctor_check_loadfail: 'Ошибки загрузки плагинов',
            doctor_check_vdf: 'Алиасы плагинов Metamod',
            doctor_check_metaplugins: 'Записи metaplugins.ini',
            doctor_check_scratch: 'Остатки загрузок',
        },
    },

    slots: {
        'server-tabs': [
            {
                component: ModsTab,
                order: 100,
                label: '@:tab_label',
                icon: 'plug',
                name: 'plugins',
                checkPermission: {
                    type: 'hasServerPermissions',
                    permissions: ['plugin:onzggylemrxw4:manage'],
                },
                checkGame: {
                    codes: SOURCE_GAME_CODES,
                },
            },
        ],
    },
};
