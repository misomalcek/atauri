<script>
  import { invoke } from '../api/index.js';
  import { projectStore } from '../stores/project';

  let { ontoggle, onfileselect } = $props();
  let files = $state([]);
  let gitFiles = $state({ modified: [], staged: [], untracked: [] });
  let project = $derived($projectStore);

  $effect(() => {
    if (project) {
      loadFiles();
      loadGitStatus();
    }
  });

  async function loadFiles() {
    try {
      files = await invoke('get_file_tree');
    } catch (e) {
      console.error('Failed to load files:', e);
    }
  }

  async function loadGitStatus() {
    try {
      const s = await invoke('git_status');
      gitFiles = { modified: s.modified, staged: s.staged, untracked: s.untracked };
    } catch {
      gitFiles = { modified: [], staged: [], untracked: [] };
    }
  }

  function gitBadge(file) {
    if (gitFiles.staged.includes(file)) return 'S';
    if (gitFiles.modified.includes(file)) return 'M';
    if (gitFiles.untracked.includes(file)) return 'U';
    return null;
  }

  function badgeClass(badge) {
    if (badge === 'S') return 'git-staged';
    if (badge === 'M') return 'git-modified';
    if (badge === 'U') return 'git-untracked';
    return '';
  }

  function getIcon(path) {
    if (path.endsWith('.rs')) return '\u{1F980}';
    if (path.endsWith('.ts') || path.endsWith('.js')) return '\u{1F4DC}';
    if (path.endsWith('.svelte')) return '\u{1F536}';
    if (path.endsWith('.json') || path.endsWith('.toml')) return '\u{2699}\uFE0F';
    if (path.endsWith('.md')) return '\u{1F4DD}';
    return '\u{1F4C4}';
  }
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <span class="project-name">{project?.name || 'No project'}</span>
    <button class="toggle" onclick={ontoggle}>x</button>
  </div>

  {#if project}
    <div class="meta">
      <span class="tech">{project.tech_stack?.join(' + ')}</span>
      <span class="count">{project.total_files} files</span>
    </div>
    <div class="file-list">
      {#each files as file}
        <div class="file-item" title={file} role="button" tabindex="0"
          onclick={() => onfileselect?.(file)}
          onkeydown={(e) => e.key === 'Enter' && onfileselect?.(file)}>
          <span class="icon">{getIcon(file)}</span>
          <span class="path">{file}</span>
          {#if gitBadge(file)}
            <span class="git-badge {badgeClass(gitBadge(file))}">{gitBadge(file)}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</aside>

<style>
  .sidebar { width: 260px; background: #0f172a; border-right: 1px solid #1e293b; display: flex; flex-direction: column; overflow: hidden; }
  .sidebar-header { display: flex; justify-content: space-between; align-items: center; padding: 0.75rem 1rem; border-bottom: 1px solid #1e293b; }
  .project-name { font-size: 0.85rem; font-weight: 600; color: #60a5fa; }
  .toggle { background: none; border: none; color: #475569; cursor: pointer; font-size: 0.8rem; }
  .meta { padding: 0.5rem 1rem; display: flex; flex-direction: column; gap: 0.2rem; border-bottom: 1px solid #1e293b; }
  .tech { font-size: 0.7rem; color: #6ee7b7; }
  .count { font-size: 0.7rem; color: #475569; }
  .file-list { flex: 1; overflow-y: auto; padding: 0.5rem 0; }
  .file-item { display: flex; gap: 0.5rem; padding: 0.25rem 1rem; font-size: 0.8rem; cursor: pointer; align-items: center; }
  .file-item:hover { background: #1e293b; }
  .icon { font-size: 0.7rem; flex-shrink: 0; }
  .path { color: #94a3b8; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .git-badge { font-size: 0.6rem; font-weight: 700; padding: 0 0.25rem; border-radius: 2px; flex-shrink: 0; }
  .git-modified { color: #fbbf24; }
  .git-untracked { color: #6ee7b7; }
  .git-staged { color: #60a5fa; }
</style>
