<script>
  import { invoke } from '../api/index.js';

  let { onclose } = $props();

  let query = $state('');
  let results = $state([]);
  let collections = $state([]);
  let selectedCollection = $state('tauri_code');
  let searching = $state(false);
  let indexing = $state(false);
  let connected = $state(false);
  let status = $state('');
  let debounceTimer = $state(null);

  async function connectQdrant() {
    try {
      await invoke('connect_qdrant');
      connected = true;
      status = 'connected';
      await loadCollections();
      setTimeout(() => status = '', 2000);
    } catch (e) {
      status = 'error: ' + e;
      connected = false;
    }
  }

  async function loadCollections() {
    try {
      collections = await invoke('list_knowledge_collections');
    } catch (e) {
      console.error('Failed to load collections:', e);
    }
  }

  function onQueryInput() {
    if (debounceTimer) clearTimeout(debounceTimer);
    if (query.trim().length < 3) {
      results = [];
      return;
    }
    debounceTimer = setTimeout(() => search(), 300);
  }

  async function search() {
    if (!query.trim()) return;
    searching = true;
    try {
      results = await invoke('search_knowledge', {
        query: query.trim(),
        collection: selectedCollection,
        limit: 8,
        pathFilter: null,
      });
    } catch (e) {
      status = 'search error: ' + e;
    } finally {
      searching = false;
    }
  }

  async function indexProject() {
    indexing = true;
    status = 'indexing...';
    try {
      const count = await invoke('index_project_knowledge', {
        collection: selectedCollection,
      });
      status = `indexed ${count} chunks`;
      await loadCollections();
      setTimeout(() => status = '', 3000);
    } catch (e) {
      status = 'index error: ' + e;
    } finally {
      indexing = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') onclose();
    if (e.key === 'Enter' && query.trim()) search();
  }

  function scoreColor(score) {
    if (score > 0.8) return '#6ee7b7';
    if (score > 0.6) return '#60a5fa';
    if (score > 0.4) return '#fbbf24';
    return '#64748b';
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="panel">
  <div class="panel-header">
    <h3>Knowledge Base</h3>
    <div class="header-actions">
      {#if !connected}
        <button class="connect-btn" onclick={connectQdrant}>Connect Qdrant</button>
      {:else}
        <span class="connected-badge">connected</span>
      {/if}
      <button class="close-btn" onclick={onclose}>&times;</button>
    </div>
  </div>

  {#if connected}
    <div class="search-bar">
      <input
        type="text"
        bind:value={query}
        oninput={onQueryInput}
        placeholder="Semantic search across code..."
        autofocus
      />
      {#if searching}
        <span class="spinner"></span>
      {/if}
    </div>

    <div class="controls">
      <select bind:value={selectedCollection}>
        {#each collections as coll}
          <option value={coll.name}>{coll.name} ({coll.points_count} vectors)</option>
        {/each}
        <option value="tauri_code">tauri_code (default)</option>
      </select>

      <button class="index-btn" onclick={indexProject} disabled={indexing}>
        {indexing ? 'Indexing...' : 'Index Project'}
      </button>
    </div>

    <div class="results">
      {#each results as result}
        <div class="result-card">
          <div class="result-header">
            <span class="file-path">{result.file_path}</span>
            <span class="line-range">L{result.start_line}-{result.end_line}</span>
            <span class="score" style="color: {scoreColor(result.score)}">
              {result.score.toFixed(3)}
            </span>
          </div>
          <pre class="code-preview">{result.code_chunk}</pre>
        </div>
      {:else}
        {#if query.trim().length >= 3 && !searching}
          <p class="empty">No results found</p>
        {:else if !query.trim()}
          <p class="empty">Type to search semantically across your codebase</p>
        {/if}
      {/each}
    </div>
  {:else}
    <div class="not-connected">
      <p>Connect to Qdrant to enable semantic code search.</p>
      <p class="hint">Ensure Qdrant is running at the URL configured in Settings.</p>
    </div>
  {/if}

  {#if status}
    <div class="status-bar" class:error={status.startsWith('error') || status.includes('error')}>
      {status}
    </div>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #0f172a;
    border-left: 1px solid #1e293b;
    width: 380px;
    min-width: 300px;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #1e293b;
  }

  .panel-header h3 {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 400;
    color: #94a3b8;
    letter-spacing: 0.05em;
  }

  .header-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .connect-btn {
    padding: 0.25rem 0.6rem;
    background: #1e293b;
    border: 1px solid #334155;
    color: #60a5fa;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .connect-btn:hover {
    border-color: #60a5fa;
  }

  .connected-badge {
    font-size: 0.7rem;
    color: #6ee7b7;
    background: #064e3b;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
  }

  .close-btn {
    background: none;
    border: none;
    color: #475569;
    font-size: 1.2rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  .close-btn:hover { color: #e2e8f0; }

  .search-bar {
    padding: 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border-bottom: 1px solid #1e293b;
  }

  .search-bar input {
    flex: 1;
    background: #1e293b;
    border: 1px solid #334155;
    color: #e2e8f0;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.8rem;
    outline: none;
  }

  .search-bar input:focus {
    border-color: #60a5fa;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid #334155;
    border-top-color: #60a5fa;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .controls {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid #1e293b;
  }

  select {
    flex: 1;
    background: #1e293b;
    border: 1px solid #334155;
    color: #94a3b8;
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
  }

  .index-btn {
    padding: 0.3rem 0.75rem;
    background: #064e3b;
    border: 1px solid #065f46;
    color: #6ee7b7;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
    white-space: nowrap;
  }

  .index-btn:hover:not(:disabled) { background: #065f46; }
  .index-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .results {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .result-card {
    background: #1e293b;
    border: 1px solid #334155;
    border-radius: 6px;
    margin-bottom: 0.5rem;
    overflow: hidden;
  }

  .result-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid #334155;
    font-size: 0.7rem;
  }

  .file-path {
    color: #60a5fa;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line-range {
    color: #475569;
  }

  .score {
    font-weight: 500;
  }

  .code-preview {
    padding: 0.5rem 0.6rem;
    margin: 0;
    font-size: 0.7rem;
    line-height: 1.4;
    color: #94a3b8;
    overflow-x: auto;
    max-height: 150px;
    white-space: pre-wrap;
  }

  .empty {
    color: #475569;
    font-size: 0.8rem;
    text-align: center;
    padding: 2rem 1rem;
    font-style: italic;
  }

  .not-connected {
    padding: 2rem 1rem;
    text-align: center;
  }

  .not-connected p {
    color: #64748b;
    font-size: 0.85rem;
  }

  .hint {
    color: #475569;
    font-size: 0.75rem;
  }

  .status-bar {
    padding: 0.4rem 0.75rem;
    font-size: 0.7rem;
    color: #6ee7b7;
    border-top: 1px solid #1e293b;
    background: #0f172a;
  }

  .status-bar.error {
    color: #fca5a5;
  }
</style>
