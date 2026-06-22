<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { Store } from '@tauri-apps/plugin-store';

  let gitPath = $state('C:\\Program Files\\Git\\cmd\\git.exe');
  let repoPath = $state('');
  let fromOffset = $state(0);
  let toOffset = $state(0);
  let excludeFilter = $state('.pdf, .zip, .docx');
  let outputPath = $state('');

  let gitVersion = $state('');
  let commitInfo = $state<{hash: string, message: string} | null>(null);
  let changedFiles = $state<string[]>([]);
  let commitHistory = $state<{index: number, hash: string, message: string, date: string}[]>([]);
  let isLoading = $state(false);
  let message = $state('');
  let isError = $state(false);
  let historyError = $state('');

  let store: Store | null = null;

  onMount(async () => {
    store = await Store.load('.settings.dat');

    const savedGitPath = await store.get<string>('gitPath');
    if (savedGitPath) {
      gitPath = savedGitPath;
    }
    const savedExcludeFilter = await store.get<string>('excludeFilter');
    if (savedExcludeFilter !== null) {
      excludeFilter = savedExcludeFilter;
    }
    const savedFrom = await store.get<number>('fromOffset');
    if (savedFrom !== null) fromOffset = savedFrom;
    const savedTo = await store.get<number>('toOffset');
    if (savedTo !== null) toOffset = savedTo;
  });

  async function validateGit() {
    try {
      const result = await invoke<{valid: boolean, version: string}>('validate_git', { gitPath });
      if (result.valid) {
        gitVersion = result.version;
        if (store) {
          await store.set('gitPath', gitPath);
          await store.save();
        }
        return true;
      } else {
        gitVersion = 'Gitではありません';
        return false;
      }
    } catch (e) {
      gitVersion = '検証失敗';
      return false;
    }
  }

  async function selectGitPath() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Executable', extensions: ['exe'] }]
    });
    if (selected && typeof selected === 'string') {
      gitPath = selected;
      // Do not validate immediately as per user request
    }
  }

  async function validateRepo(path: string) {
    if (!path || !gitPath) return;

    isLoading = true;
    message = 'リポジトリを確認中...';
    isError = false;

    try {
      // Perform Git connectivity check only when repository is specified
      const isGitValid = await validateGit();
      if (!isGitValid) {
        isError = true;
        message = 'Git実行ファイルが正しく設定されていません';
        isLoading = false;
        return;
      }

      const isValid = await invoke<boolean>('validate_repo', { gitPath, repoPath: path });
      if (isValid) {
        repoPath = path;
        message = '';
        updateCommitInfo();
        updateCommitHistory();
      } else {
        isError = true;
        message = 'Gitリポジトリではありません';
      }
    } catch (e) {
      isError = true;
      message = '検証失敗: ' + e;
    } finally {
      isLoading = false;
    }
  }

  async function selectRepoPath() {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      validateRepo(selected);
    }
  }

  async function fetchRemote() {
    if (!repoPath || !gitPath) return;
    isLoading = true;
    message = '更新中...';
    isError = false;
    try {
      await invoke('fetch_remote', { gitPath, repoPath });
      isError = false;
      message = '更新完了';
      updateCommitInfo();
      updateCommitHistory();
    } catch (e) {
      isError = true;
      message = '失敗: ' + e;
    } finally {
      isLoading = false;
    }
  }

  async function selectOutputPath() {
    const selected = await save({
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
      defaultPath: 'patch.zip'
    });
    if (selected) {
      outputPath = selected;
    }
  }

  async function updateCommitHistory() {
    if (!repoPath || !gitPath) return;
    try {
      commitHistory = await invoke('get_commit_history', { gitPath, repoPath, count: 20 });
      historyError = '';
    } catch (e) {
      commitHistory = [];
      historyError = '履歴の取得に失敗しました';
    }
  }

  async function updateCommitInfo() {
    if (!repoPath || !gitPath) return;
    isLoading = true;
    try {
      commitInfo = await invoke('get_commit_info', { gitPath, repoPath, fromOffset, toOffset });
      const filesResult = await invoke<{files: string[]}>('get_changed_files', { gitPath, repoPath, fromOffset, toOffset });
      changedFiles = filesResult.files;
      isError = false;
      message = '';
    } catch (e) {
      commitInfo = null;
      changedFiles = [];
      isError = true;
      message = String(e);
    } finally {
      isLoading = false;
    }
  }

  async function createZip() {
    if (!gitPath || !repoPath || !outputPath) {
      isError = true;
      message = '未入力項目があります';
      return;
    }
    isLoading = true;
    const excludePatterns = excludeFilter.split(',').map(p => p.trim()).filter(p => p.length > 0);
    try {
      await invoke('create_zip', {
        gitPath,
        repoPath,
        fromOffset,
        toOffset,
        outputPath,
        excludePatterns
      });
      if (store) {
        await store.set('excludeFilter', excludeFilter);
        await store.set('fromOffset', fromOffset);
        await store.set('toOffset', toOffset);
        await store.save();
      }
      isError = false;
      message = '作成成功';
    } catch (e) {
      isError = true;
      message = '作成失敗: ' + e;
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    // Only update if repoPath is set
    if (repoPath && fromOffset >= 0 && toOffset >= 0) {
      updateCommitInfo();
    }
  });

</script>

<div class="h-screen bg-slate-50 flex flex-col overflow-hidden text-slate-800 text-sm select-none">
  <header class="bg-white border-b px-3 py-2 flex justify-between items-center shrink-0">
    <div class="flex items-center gap-2">

        {#if gitVersion}
            <span class="text-[10px] text-slate-400 truncate max-w-[200px]">({gitVersion})</span>
        {/if}
    </div>
    <div class="flex gap-2">
        <button onclick={fetchRemote} disabled={!repoPath || isLoading} class="bg-slate-100 hover:bg-slate-200 rounded px-2 py-0.5 border disabled:opacity-30 flex items-center gap-1 transition-colors">
            <span>↻</span> リモート更新
        </button>
    </div>
  </header>

  <main class="flex-1 flex overflow-hidden">
    <!-- Left Column: Commit History -->
    <section class="w-1/3 border-r bg-white flex flex-col min-w-0">
        <div class="px-3 py-2 border-b bg-slate-50/50 flex justify-between items-center">
            <h2 class="font-bold text-slate-600 uppercase tracking-wider text-[10px]">コミット履歴 (最新20件)</h2>
        </div>
        <div class="flex-1 overflow-y-auto">
            <table class="w-full text-[10px] text-left border-collapse table-fixed">
                <thead class="bg-white sticky top-0 shadow-sm z-10">
                  <tr>
                    <th class="px-3 py-2 border-b w-16 text-slate-400">ID</th>
                    <th class="px-3 py-2 border-b w-20 text-slate-400">Hash</th>
                    <th class="px-3 py-2 border-b text-slate-400">Message</th>
                    <th class="px-3 py-2 border-b w-24 text-right text-slate-400">Action</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-slate-50">
                  {#each commitHistory as item}
                    <tr class="hover:bg-blue-50/50 transition-colors group">
                      <td class="px-3 py-2 font-mono text-slate-400">HEAD~{item.index}</td>
                      <td class="px-3 py-2 font-mono text-slate-400">{item.hash.slice(0,7)}</td>
                      <td class="px-3 py-2 truncate text-slate-700" title={item.message}>{item.message}</td>
                      <td class="px-3 py-2 text-right space-x-1 whitespace-nowrap">
                        <button onclick={() => fromOffset = item.index} class="px-1.5 py-0.5 bg-white group-hover:bg-white hover:bg-blue-100 text-blue-600 rounded border border-blue-100 transition-colors">始</button>
                        <button onclick={() => toOffset = item.index} class="px-1.5 py-0.5 bg-white group-hover:bg-white hover:bg-emerald-100 text-emerald-600 rounded border border-emerald-100 transition-colors">終</button>
                      </td>
                    </tr>
                  {/each}
                  {#if commitHistory.length === 0}
                    <tr>
                      <td colspan="4" class="px-3 py-8 text-center italic">
                        {#if historyError}
                          <span class="text-red-400 font-bold">{historyError}</span>
                        {:else}
                          <span class="text-slate-300">リポジトリを選択してください</span>
                        {/if}
                      </td>
                    </tr>
                  {/if}
                </tbody>
            </table>
        </div>
    </section>

    <!-- Right Column: Panel -->
    <section class="flex-1 flex flex-col min-w-0 bg-slate-50/30 overflow-y-auto p-4 space-y-4">
        <!-- Group: Configuration -->
        <div class="space-y-3 bg-white p-3 rounded-lg border shadow-sm">
            <div class="grid grid-cols-1 gap-3">
                <div class="space-y-1">
                  <label class="font-bold text-slate-500 uppercase tracking-tight text-[11px]" for="git-path">Git 実行ファイル (git.exe)</label>
                  <div class="flex gap-1">
                    <input id="git-path" type="text" bind:value={gitPath} class="w-full bg-slate-50 border rounded px-2 py-1.5 outline-none focus:border-slate-400 transition-colors truncate" />
                    <button onclick={selectGitPath} class="bg-white hover:bg-slate-50 rounded px-3 py-1 border shadow-sm transition-colors">参照</button>
                  </div>
                </div>
                <div class="space-y-1">
                  <label class="font-bold text-slate-500 uppercase tracking-tight text-[11px]" for="repo-path">Git リポジトリ フォルダ</label>
                  <div class="flex gap-1">
                    <input
                      id="repo-path"
                      type="text"
                      bind:value={repoPath}
                      onchange={() => validateRepo(repoPath)}
                      placeholder="C:\Project"
                      class="w-full bg-slate-50 border rounded px-2 py-1.5 outline-none focus:border-slate-400 transition-colors truncate"
                    />
                    <button onclick={selectRepoPath} class="bg-white hover:bg-slate-50 rounded px-3 py-1 border shadow-sm transition-colors">参照</button>
                  </div>
                </div>
            </div>

            <div class="grid grid-cols-2 gap-4 pt-2">
                <div class="space-y-1">
                  <span class="block font-bold text-slate-500 uppercase tracking-tight text-[11px]">抽出範囲 (HEAD~N)</span>
                  <div class="flex items-center gap-2">
                    <div class="flex-1 relative">
                        <span class="absolute left-2 top-1/2 -translate-y-1/2 text-[10px] text-slate-400 font-bold">FROM</span>
                        <input type="number" min="0" bind:value={fromOffset} class="w-full bg-slate-50 border rounded pl-8 pr-2 py-1.5 outline-none focus:border-blue-400 transition-colors text-blue-600 font-bold" />
                    </div>
                    <span class="text-slate-300">→</span>
                    <div class="flex-1 relative">
                        <span class="absolute left-2 top-1/2 -translate-y-1/2 text-[10px] text-slate-400 font-bold">TO</span>
                        <input type="number" min="0" bind:value={toOffset} class="w-full bg-slate-50 border rounded pl-8 pr-2 py-1.5 outline-none focus:border-emerald-400 transition-colors text-emerald-600 font-bold" />
                    </div>
                  </div>
                </div>
                <div class="space-y-1">
                  <label class="font-bold text-slate-500 uppercase tracking-tight text-[11px]" for="exclude-filter">除外フィルタ (カンマ区切り)</label>
                  <input id="exclude-filter" type="text" bind:value={excludeFilter} placeholder=".pdf, .zip" class="w-full bg-slate-50 border rounded px-2 py-1.5 outline-none focus:border-slate-400 transition-colors truncate" />
                </div>
            </div>
        </div>

        <!-- Group: Previews -->
        <div class="grid grid-cols-1 gap-4">
            <!-- Commit Info -->
            <div class="space-y-1">
                <span class="block font-bold text-slate-400 uppercase tracking-tight text-[11px]">選択中のコミット内容</span>
                <div class="bg-white border rounded-lg p-3 min-h-[60px] shadow-sm flex flex-col justify-center">
                    {#if commitInfo}
                        <div class="flex items-center gap-2 mb-1">
                            <span class="bg-slate-100 text-slate-600 px-1.5 py-0.5 rounded font-mono text-[11px]">{commitInfo.hash.slice(0,10)}</span>
                        </div>
                        <p class="text-slate-700 leading-snug">{commitInfo.message}</p>
                    {:else}
                        <p class="text-slate-300 italic text-center">情報を取得できません</p>
                    {/if}
                </div>
            </div>

            <!-- Files -->
            <div class="flex flex-col min-h-0">
                <div class="flex justify-between items-baseline mb-1">
                    <span class="block font-bold text-slate-400 uppercase tracking-tight text-[11px]">変更ファイル一覧 ({changedFiles.length})</span>
                </div>
                <div class="bg-white border rounded-lg overflow-hidden shadow-sm flex-1 flex flex-col min-h-[150px]">
                    <div class="flex-1 overflow-y-auto font-mono text-[10px] p-2 leading-relaxed">
                        {#if changedFiles.length > 0}
                          <ul class="space-y-0.5">
                            {#each changedFiles as file}
                              <li class="px-2 py-0.5 hover:bg-slate-50 rounded transition-colors truncate text-slate-600 border-l-2 border-transparent hover:border-blue-400">{file}</li>
                            {/each}
                          </ul>
                        {:else}
                          <div class="h-full flex items-center justify-center">
                              <p class="text-slate-300 italic">変更ファイルはありません</p>
                          </div>
                        {/if}
                    </div>
                </div>
            </div>
        </div>

        <!-- Group: Output -->
        <div class="space-y-1 bg-blue-50/50 p-3 rounded-lg border border-blue-100">
            <label class="font-bold text-blue-400 uppercase tracking-tight text-[11px]" for="output-path">保存先 ZIP パス</label>
            <div class="flex gap-2">
                <input id="output-path" type="text" bind:value={outputPath} placeholder="C:\temp\patch.zip" class="w-full bg-white border-blue-100 border rounded px-3 py-2 outline-none focus:border-blue-400 transition-colors truncate" />
              <button onclick={selectOutputPath} class="bg-white hover:bg-blue-50 text-blue-600 rounded px-4 py-2 border border-blue-200 shadow-sm transition-colors font-bold">保存先を選択</button>
            </div>
        </div>
    </section>
  </main>

  <!-- Footer -->
  <footer class="bg-white border-t p-3 shrink-0 flex items-center gap-4 shadow-[0_-1px_3px_rgba(0,0,0,0.05)] relative z-20">
    {#if isLoading}
      <div class="absolute top-0 left-0 w-full h-0.5 bg-slate-50 overflow-hidden">
        <div class="bg-blue-600 h-full animate-progress w-1/3"></div>
      </div>
    {/if}

    <div class="flex-1 min-w-0">
        {#if message}
            <div class={`flex items-center gap-2 p-2 rounded ${isError ? 'bg-red-50 text-red-700' : 'bg-emerald-50 text-emerald-700'}`}>
                <span class="text-lg leading-none">{isError ? '×' : '✓'}</span>
                <p class="text-[10px] font-bold truncate">{message}</p>
            </div>
        {/if}
    </div>

    <button
      onclick={createZip}
      disabled={isLoading || !outputPath || !repoPath}
      class="bg-slate-800 hover:bg-slate-900 text-white font-bold rounded-lg px-10 py-2.5 transition-all disabled:opacity-50 disabled:bg-slate-300 shadow-lg hover:shadow-xl active:scale-[0.98] whitespace-nowrap text-sm"
    >
      {isLoading ? '処理中...' : 'ZIPファイルを作成'}
    </button>
  </footer>
</div>

<style>
  :global(html, body) {
    height: 100%;
    margin: 0;
    overflow: hidden;
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
  }

  @keyframes progress {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(300%); }
  }
  .animate-progress {
    animation: progress 1.5s infinite linear;
  }

  /* Compact scrollbar */
  ::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  ::-webkit-scrollbar-track {
      background: transparent;
  }
  ::-webkit-scrollbar-thumb {
    background: #e2e8f0;
    border-radius: 10px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background: #cbd5e1;
  }

  input[type=number]::-webkit-inner-spin-button,
  input[type=number]::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
</style>
