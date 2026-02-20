<script>
  import { invoke } from '../api/index.js';
  import hljs from 'highlight.js';

  let { filepath, onclose } = $props();
  let content = $state('');
  let originalContent = $state('');
  let saving = $state(false);
  let modified = $derived(content !== originalContent);
  let language = $derived(detectLanguage(filepath));

  $effect(() => {
    if (filepath) loadFile();
  });

  async function loadFile() {
    try {
      const data = await invoke('read_file', { path: filepath });
      content = data;
      originalContent = data;
    } catch (e) {
      content = `// Error loading file: ${e}`;
      originalContent = content;
    }
  }

  async function saveFile() {
    saving = true;
    try {
      await invoke('write_file_content', { path: filepath, content });
      originalContent = content;
    } catch (e) {
      alert(`Save failed: ${e}`);
    }
    saving = false;
  }

  function handleKeydown(e) {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      if (modified) saveFile();
    }
  }

  function detectLanguage(path) {
    if (!path) return 'plaintext';
    const ext = path.split('.').pop();
    const map = {
      rs: 'rust', js: 'javascript', ts: 'typescript', svelte: 'xml',
      json: 'json', toml: 'ini', md: 'markdown', py: 'python',
      css: 'css', html: 'xml', yaml: 'yaml', yml: 'yaml',
      tsx: 'typescript', jsx: 'javascript', go: 'go',
    };
    return map[ext] || 'plaintext';
  }

  function getFilename(path) {
    return path?.split('/').pop() || 'untitled';
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="file-viewer">
  <div class="viewer-header">
    <div class="file-info">
      <span class="filename">{getFilename(filepath)}</span>
      <span class="filepath">{filepath}</span>
      {#if modified}<span class="modified-badge">modified</span>{/if}
    </div>
    <div class="actions">
      <button class="save-btn" onclick={saveFile} disabled={!modified || saving}>
        {saving ? 'Saving...' : 'Save'}
      </button>
      <button class="close-btn" onclick={onclose}>x</button>
    </div>
  </div>
  <textarea
    class="editor"
    bind:value={content}
    spellcheck="false"
  ></textarea>
</div>

<style>
  .file-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #0a0e17;
    border-left: 1px solid #1e293b;
    min-width: 400px;
  }
  .viewer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid #1e293b;
    background: #0f172a;
    gap: 1rem;
  }
  .file-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    overflow: hidden;
    min-width: 0;
  }
  .filename {
    font-weight: 600;
    color: #60a5fa;
    font-size: 0.85rem;
    flex-shrink: 0;
  }
  .filepath {
    font-size: 0.7rem;
    color: #475569;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .modified-badge {
    font-size: 0.65rem;
    color: #fbbf24;
    background: #fbbf2420;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .save-btn {
    background: #16a34a;
    color: white;
    border: none;
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    font-size: 0.75rem;
    cursor: pointer;
    font-family: inherit;
  }
  .save-btn:disabled { opacity: 0.3; cursor: not-allowed; }
  .save-btn:not(:disabled):hover { background: #15803d; }
  .close-btn {
    background: none;
    border: 1px solid #334155;
    color: #475569;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    cursor: pointer;
    font-family: inherit;
  }
  .close-btn:hover { color: #e2e8f0; border-color: #475569; }
  .editor {
    flex: 1;
    background: #0a0e17;
    color: #e2e8f0;
    border: none;
    padding: 1rem;
    font-family: 'SF Mono', 'Fira Code', 'JetBrains Mono', monospace;
    font-size: 0.8rem;
    line-height: 1.6;
    resize: none;
    outline: none;
    tab-size: 2;
  }
</style>
