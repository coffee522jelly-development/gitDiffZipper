<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { LazyStore } from '@tauri-apps/plugin-store';

  let gitPath = $state('C:\\Program Files\\Git\\cmd\\git.exe');
  let repoPath = $state('');
  let fromOffset = $state(0);
  let toOffset = $state(0);
  let excludeFilter = $state('.pdf, .zip, .docx');
  let outputPath = $state('');

  let gitVersion = $state('');
  let commitInfo = $state<{hash: string, message: string} | null>(null);
  let changedFiles = $state<string[]>([]);
  let isLoading = $state(false);
  let message = $state('');
  let isError = $state(false);

  const store = new LazyStore('.settings.dat');

  onMount(async () => {
    const savedGitPath = await store.get<string>('gitPath');
    if (savedGitPath) {
      gitPath = savedGitPath;
      validateGit();
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
        await store.set('gitPath', gitPath);
        await store.save();
      } else {
        gitVersion = 'Gitではありません';
      }
    } catch (e) {
      gitVersion = '検証失敗';
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
      validateGit();
    }
  }

  async function validateRepo(path: string) {
    if (!path || !gitPath) return;
    try {
      const isValid = await invoke<boolean>('validate_repo', { gitPath, repoPath: path });
      if (isValid) {
        repoPath = path;
        message = '';
        updateCommitInfo();
      } else {
        isError = true;
        message = 'Gitリポジトリではありません';
      }
    } catch (e) {
      isError = true;
      message = '検証失敗: ' + e;
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

  async function updateCommitInfo() {
    if (!repoPath || !gitPath) return;
    isLoading = true;
    try {
      commitInfo = await invoke('get_commit_info', { gitPath, repoPath, from_offset: fromOffset, to_offset: toOffset });
      const filesResult = await invoke<{files: string[]}>('get_changed_files', { gitPath, repoPath, from_offset: fromOffset, to_offset: toOffset });
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
        from_offset: fromOffset,
        to_offset: toOffset,
        outputPath,
        excludePatterns
      });
      await store.set('excludeFilter', excludeFilter);
      await store.set('fromOffset', fromOffset);
      await store.set('toOffset', toOffset);
      await store.save();
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
    if (fromOffset >= 0 && toOffset >= 0) {
      updateCommitInfo();
    }
  });

</script>

<div class="h-screen bg-slate-50 flex flex-col overflow-hidden text-slate-800 text-xs select-none">
  <header class="bg-white border-b px-3 py-2 flex justify-between items-center shrink-0">
    <h1 class="font-bold tracking-tight">gitDiffZipper</h1>
    {#if gitVersion}
      <span class="text-[10px] text-slate-400 truncate max-w-[150px]">{gitVersion}</span>
    {/if}
  </header>

  <main class="flex-1 overflow-y-auto p-3 space-y-3">
    <!-- Row: Git & Repo -->
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <label class="font-semibold text-slate-500" for="git-path">Git.exe</label>
        <div class="flex gap-1">
          <input id="git-path" type="text" bind:value={gitPath} class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors truncate" />
          <button onclick={selectGitPath} class="bg-slate-100 hover:bg-slate-200 rounded px-2 py-1 border">...</button>
        </div>
      </div>
      <div class="space-y-1">
        <label class="font-semibold text-slate-500" for="repo-path">リポジトリ</label>
        <div class="flex gap-1">
          <input
            id="repo-path"
            type="text"
            bind:value={repoPath}
            onchange={() => validateRepo(repoPath)}
            placeholder="C:\Project"
            class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors truncate"
          />
          <button onclick={selectRepoPath} class="bg-slate-100 hover:bg-slate-200 rounded px-2 py-1 border">...</button>
          <button onclick={fetchRemote} disabled={!repoPath || isLoading} class="bg-slate-100 hover:bg-slate-200 rounded px-2 py-1 border disabled:opacity-30">↻</button>
        </div>
      </div>
    </div>

    <!-- Row: Range & Exclude -->
    <div class="grid grid-cols-2 gap-3">
      <div class="space-y-1">
        <label class="font-semibold text-slate-500">対象範囲 (HEAD~N)</label>
        <div class="flex items-center gap-1">
          <input type="number" min="0" bind:value={fromOffset} title="古い方" class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors" />
          <span>～</span>
          <input type="number" min="0" bind:value={toOffset} title="新しい方" class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors" />
        </div>
      </div>
      <div class="space-y-1">
        <label class="font-semibold text-slate-500" for="exclude-filter">除外フィルタ (カンマ区切り)</label>
        <input id="exclude-filter" type="text" bind:value={excludeFilter} placeholder=".pdf, .zip" class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors truncate" />
      </div>
    </div>

    <!-- Row: Output -->
    <div class="space-y-1">
      <label class="font-semibold text-slate-500" for="output-path">出力先 (ZIP)</label>
      <div class="flex gap-1">
          <input id="output-path" type="text" bind:value={outputPath} placeholder="C:\temp\patch.zip" class="w-full bg-white border rounded px-2 py-1 outline-none focus:border-slate-400 transition-colors truncate" />
        <button onclick={selectOutputPath} class="bg-slate-100 hover:bg-slate-200 rounded px-2 py-1 border">...</button>
      </div>
    </div>

    <!-- Commit Preview -->
    <div class="space-y-1">
      <div class="flex justify-between items-baseline">
        <label class="font-semibold text-slate-500">コミット情報</label>
        {#if commitInfo}
          <span class="text-[10px] font-mono text-slate-400">{commitInfo.hash.slice(0,7)}</span>
        {/if}
      </div>
      <div class="bg-white border rounded p-2 h-12 overflow-y-auto">
        {#if commitInfo}
          <p class="leading-tight">{commitInfo.message}</p>
        {:else}
          <p class="text-slate-300 italic">情報なし</p>
        {/if}
      </div>
    </div>

    <!-- Files Preview -->
    <div class="space-y-1 flex flex-col flex-1 min-h-0">
      <label class="font-semibold text-slate-500">変更ファイル ({changedFiles.length})</label>
      <div class="bg-white border rounded p-2 flex-1 overflow-y-auto font-mono text-[10px] leading-tight min-h-[80px]">
        {#if changedFiles.length > 0}
          <ul class="divide-y divide-slate-50">
            {#each changedFiles as file}
              <li class="py-0.5 truncate">{file}</li>
            {/each}
          </ul>
        {:else}
          <p class="text-slate-300 italic">変更なし</p>
        {/if}
      </div>
    </div>
  </main>

  <footer class="bg-white border-t p-2 space-y-2 shrink-0">
    {#if isLoading}
      <div class="w-full bg-slate-100 h-1 rounded-full overflow-hidden">
        <div class="bg-slate-800 h-full animate-progress w-1/3"></div>
      </div>
    {/if}

    <div class="flex items-center gap-3">
      <div class="flex-1 min-w-0 max-h-24 overflow-y-auto">
        {#if message}
          <p class={`text-[10px] whitespace-pre-wrap leading-tight font-bold ${isError ? 'text-red-600' : 'text-green-600'}`}>
            {isError ? '× ' : '✓ '}{message}
          </p>
        {/if}
      </div>
      <button
        onclick={createZip}
        disabled={isLoading}
        class="bg-slate-800 hover:bg-slate-900 text-white font-bold rounded px-6 py-2 transition-all disabled:opacity-50 shadow-sm whitespace-nowrap"
      >
        {isLoading ? '作成中...' : 'ZIP作成'}
      </button>
    </div>
  </footer>
</div>

<style>
  :global(html, body) {
    height: 100%;
    margin: 0;
    overflow: hidden;
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
    width: 4px;
    height: 4px;
  }
  ::-webkit-scrollbar-thumb {
    background: #e2e8f0;
    border-radius: 2px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background: #cbd5e1;
  }
</style>
