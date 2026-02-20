<script>
  import { invoke } from '../api/index.js';
  import { onMount } from 'svelte';

  let lmStudio = $state({ status: 'unknown', models: 0 });
  let qdrant = $state({ status: 'unknown', collections: 0 });
  let claude = $state({ status: 'unknown' });
  let ghostStats = $state(null);
  let git = $state({ branch: '', modified: 0, ahead: 0, behind: 0, is_clean: true, active: false });

  onMount(() => {
    checkAll();
    const interval = setInterval(checkAll, 30000); // Check every 30s
    return () => clearInterval(interval);
  });

  async function checkAll() {
    await Promise.all([checkLmStudio(), checkQdrant(), checkClaude(), checkGhost(), checkGit()]);
  }

  async function checkLmStudio() {
    try {
      const models = await invoke('list_models');
      const localModels = models.filter(m => m.tier === 'Local');
      lmStudio = { status: localModels.length > 0 ? 'connected' : 'empty', models: localModels.length };
    } catch {
      lmStudio = { status: 'disconnected', models: 0 };
    }
  }

  async function checkQdrant() {
    try {
      const collections = await invoke('list_knowledge_collections');
      qdrant = { status: 'connected', collections: collections.length };
    } catch {
      qdrant = { status: 'disconnected', collections: 0 };
    }
  }

  async function checkClaude() {
    try {
      const settings = await invoke('load_settings');
      claude = { status: settings.claude_api_key ? 'configured' : 'no key' };
    } catch {
      claude = { status: 'unknown' };
    }
  }

  async function checkGhost() {
    try {
      ghostStats = await invoke('get_ghost_stats');
    } catch {
      ghostStats = null;
    }
  }

  async function checkGit() {
    try {
      const s = await invoke('git_status');
      const modified = s.modified.length + s.untracked.length + s.staged.length;
      git = { branch: s.branch, modified, ahead: s.ahead, behind: s.behind, is_clean: s.is_clean, active: true };
    } catch {
      git = { branch: '', modified: 0, ahead: 0, behind: 0, is_clean: true, active: false };
    }
  }

  function statusDot(status) {
    switch (status) {
      case 'connected': case 'configured': return 'dot-green';
      case 'empty': case 'no key': return 'dot-yellow';
      default: return 'dot-red';
    }
  }
</script>

<div class="status-bar">
  <div class="status-item">
    <span class="dot {statusDot(lmStudio.status)}"></span>
    <span class="label">LM Studio</span>
    {#if lmStudio.models > 0}
      <span class="count">{lmStudio.models}</span>
    {/if}
  </div>

  <div class="status-item">
    <span class="dot {statusDot(qdrant.status)}"></span>
    <span class="label">Qdrant</span>
    {#if qdrant.collections > 0}
      <span class="count">{qdrant.collections}</span>
    {/if}
  </div>

  <div class="status-item">
    <span class="dot {statusDot(claude.status)}"></span>
    <span class="label">Claude</span>
  </div>

  {#if ghostStats}
    <div class="status-item ghost">
      <span class="label">Ghost</span>
      <span class="count">{ghostStats.total}</span>
    </div>
  {/if}

  {#if git.active}
    <div class="status-item git">
      <span class="dot {git.is_clean ? 'dot-green' : 'dot-yellow'}"></span>
      <span class="label git-branch">{git.branch}</span>
      {#if git.modified > 0}
        <span class="git-mod">+{git.modified}</span>
      {/if}
      {#if git.ahead > 0}
        <span class="git-ahead">{git.ahead}↑</span>
      {/if}
      {#if git.behind > 0}
        <span class="git-behind">{git.behind}↓</span>
      {/if}
    </div>
  {/if}

  <div class="shortcuts">
    <span class="shortcut">^K search</span>
    <span class="shortcut">^B sidebar</span>
    <span class="shortcut">^, settings</span>
  </div>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    padding: 0.2rem 1rem;
    background: #0a0e17;
    border-top: 1px solid #1e293b;
    gap: 1rem;
    font-size: 0.65rem;
    color: #475569;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .dot-green { background: #6ee7b7; }
  .dot-yellow { background: #fbbf24; }
  .dot-red { background: #64748b; }

  .label { color: #64748b; }
  .count {
    color: #94a3b8;
    font-weight: 500;
  }

  .ghost .label { color: #fbbf24; }

  .git-branch { color: #a78bfa; }
  .git-mod { color: #fbbf24; font-weight: 500; }
  .git-ahead { color: #6ee7b7; }
  .git-behind { color: #f87171; }

  .shortcuts {
    margin-left: auto;
    display: flex;
    gap: 0.75rem;
  }

  .shortcut {
    color: #334155;
    letter-spacing: 0.03em;
  }
</style>
