<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { LazyStore } from '@tauri-apps/plugin-store';

  let gitPath = $state('C:\\Program Files\\Git\\cmd\\git.exe');
  let repoPath = $state('');
  let offset = $state(0);
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
  });

  async function validateGit() {
    try {
      const result = await invoke<{valid: boolean, version: string}>('validate_git', { gitPath });
      if (result.valid) {
        gitVersion = result.version;
        await store.set('gitPath', gitPath);
        await store.save();
      } else {
        gitVersion = '指定されたファイルはGitではありません';
      }
    } catch (e) {
      gitVersion = 'Gitの検証に失敗しました';
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

  async function selectRepoPath() {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (selected && typeof selected === 'string') {
      try {
        const isValid = await invoke<boolean>('validate_repo', { gitPath, repoPath: selected });
        if (isValid) {
          repoPath = selected;
          updateCommitInfo();
        } else {
          isError = true;
          message = '選択されたフォルダはGitリポジトリではありません';
        }
      } catch (e) {
        isError = true;
        message = 'リポジトリの検証に失敗しました: ' + e;
      }
    }
  }

  async function selectOutputPath() {
    const selected = await save({
      filters: [{ name: 'ZIP Archive', extensions: ['zip'] }],
      defaultPath: 'patch.zip'
    });
    if (selected) {
      outputPath = selected;
    }
  }

  async function updateCommitInfo() {
    if (!repoPath || !gitPath) return;
    try {
      commitInfo = await invoke('get_commit_info', { gitPath, repoPath, offset });
      const filesResult = await invoke<{files: string[]}>('get_changed_files', { gitPath, repoPath, offset });
      changedFiles = filesResult.files;
      isError = false;
      message = '';
    } catch (e) {
      commitInfo = null;
      changedFiles = [];
      isError = true;
      message = String(e);
    }
  }

  async function createZip() {
    if (!gitPath || !repoPath || !outputPath) {
      isError = true;
      message = 'すべての項目を入力してください';
      return;
    }
    isLoading = true;
    try {
      await invoke('create_zip', { gitPath, repoPath, offset, outputPath });
      isError = false;
      message = 'ZIP作成に成功しました';
    } catch (e) {
      isError = true;
      message = 'ZIP作成に失敗しました: ' + e;
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    if (offset >= 0) {
      updateCommitInfo();
    }
  });

</script>

<div class="min-h-screen bg-slate-50 p-4 font-sans text-slate-900">
  <div class="mx-auto max-w-2xl space-y-6">
    <header>
      <h1 class="text-2xl font-bold">Git差分ZIP作成ツール</h1>
    </header>

    <div class="rounded-lg border bg-white p-6 shadow-sm space-y-4">
      <!-- Git Path -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none" for="git-path">Git実行ファイル</label>
        <div class="flex gap-2">
          <input
            id="git-path"
            type="text"
            bind:value={gitPath}
            class="flex h-10 w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm"
          />
          <button
            onclick={selectGitPath}
            class="inline-flex items-center justify-center rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-900/90"
          >
            参照
          </button>
        </div>
        {#if gitVersion}
          <p class="text-xs text-slate-500">{gitVersion}</p>
        {/if}
      </div>

      <hr class="border-slate-100" />

      <!-- Repo Path -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none" for="repo-path">リポジトリ</label>
        <div class="flex gap-2">
          <input
            id="repo-path"
            type="text"
            bind:value={repoPath}
            class="flex h-10 w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm"
          />
          <button
            onclick={selectRepoPath}
            class="inline-flex items-center justify-center rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-900/90"
          >
            参照
          </button>
        </div>
      </div>

      <hr class="border-slate-100" />

      <!-- Offset -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none" for="offset">対象コミット</label>
        <div class="flex items-center gap-4">
          <input
            id="offset"
            type="number"
            min="0"
            bind:value={offset}
            class="flex h-10 w-20 rounded-md border border-slate-200 bg-white px-3 py-2 text-sm"
          />
          <span class="text-xs text-slate-500">
            0 = 最新, 1 = 1個前, 2 = 2個前
          </span>
        </div>
      </div>

      <hr class="border-slate-100" />

      <!-- Commit Info -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none">コミット情報</label>
        <div class="rounded-md border border-slate-100 bg-slate-50 p-3 text-sm min-h-[4rem]">
          {#if commitInfo}
            <p class="font-mono text-xs">{commitInfo.hash}</p>
            <p class="mt-1">{commitInfo.message}</p>
          {:else}
            <p class="text-slate-400">情報なし</p>
          {/if}
        </div>
      </div>

      <hr class="border-slate-100" />

      <!-- Changed Files -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none">
          変更ファイル（{changedFiles.length}件）
        </label>
        <div class="rounded-md border border-slate-100 bg-slate-50 p-3 text-sm h-32 overflow-y-auto font-mono text-xs">
          {#if changedFiles.length > 0}
            <ul class="space-y-1">
              {#each changedFiles as file}
                <li>{file}</li>
              {/each}
            </ul>
          {:else}
            <p class="text-slate-400 italic">変更なし</p>
          {/if}
        </div>
      </div>

      <hr class="border-slate-100" />

      <!-- Output Path -->
      <div class="space-y-2">
        <label class="text-sm font-medium leading-none" for="output-path">出力先</label>
        <div class="flex gap-2">
          <input
            id="output-path"
            type="text"
            bind:value={outputPath}
            class="flex h-10 w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm"
          />
          <button
            onclick={selectOutputPath}
            class="inline-flex items-center justify-center rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-900/90"
          >
            参照
          </button>
        </div>
      </div>

      {#if isLoading}
        <div class="w-full bg-slate-100 h-2 rounded-full overflow-hidden">
          <div class="bg-slate-900 h-full animate-progress-indefinite w-1/3"></div>
        </div>
      {/if}

      <div class="pt-4 text-center">
        <button
          onclick={createZip}
          disabled={isLoading}
          class="inline-flex h-12 w-full max-w-xs items-center justify-center rounded-md bg-slate-900 px-8 py-2 text-base font-bold text-white shadow transition-colors hover:bg-slate-900/90 disabled:opacity-50"
        >
          {#if isLoading}
            作成中...
          {:else}
            ZIP作成
          {/if}
        </button>
      </div>

      {#if message}
        <div class={`mt-4 rounded-md p-3 text-sm flex items-start gap-2 ${isError ? 'bg-red-50 text-red-900 border border-red-100' : 'bg-green-50 text-green-900 border border-green-100'}`}>
          <span class="mt-0.5">
            {#if isError}
              ⚠️
            {:else}
              ✅
            {/if}
          </span>
          <div>
            <p class="font-bold">{isError ? 'エラー' : '成功'}</p>
            <p>{message}</p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
  }

  @keyframes progress-indefinite {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(300%); }
  }
  .animate-progress-indefinite {
    animation: progress-indefinite 2s infinite linear;
  }
</style>
