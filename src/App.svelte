<script>
  import Chat from './lib/components/Chat.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import ModelBar from './lib/components/ModelBar.svelte';
  import Welcome from './lib/components/Welcome.svelte';
  import Settings from './lib/components/Settings.svelte';
  import KnowledgePanel from './lib/components/KnowledgePanel.svelte';
  import FileViewer from './lib/components/FileViewer.svelte';
  import StatusBar from './lib/components/StatusBar.svelte';
  import { projectStore } from './lib/stores/project';
  import { invoke, openFolderDialog, isDesktop } from './lib/api/index.js';

  let showSidebar = $state(true);
  let showSettings = $state(false);
  let showKnowledge = $state(false);
  let selectedFile = $state(null);
  let project = $derived($projectStore);

  // Web mode: path input state
  let showWebPathInput = $state(false);
  let webPathInput = $state('');

  function handleFileSelect(filePath) {
    const root = $projectStore?.root;
    if (root) {
      selectedFile = root + '/' + filePath;
    }
  }

  async function openProject() {
    if (isDesktop()) {
      const selected = await openFolderDialog();
      if (selected) {
        try {
          const ctx = await invoke('open_project', { path: selected });
          projectStore.set(ctx);
        } catch (e) {
          console.error('Failed to open project:', e);
        }
      }
    } else {
      // Web: show path input modal
      showWebPathInput = true;
    }
  }

  async function submitWebPath() {
    const path = webPathInput.trim();
    if (!path) return;
    showWebPathInput = false;
    try {
      const ctx = await invoke('open_project', { path });
      projectStore.set(ctx);
    } catch (e) {
      console.error('Failed to open project:', e);
      // Re-show with error
      showWebPathInput = true;
    }
    webPathInput = '';
  }

  function handleGlobalKeydown(e) {
    // Cmd+K = toggle knowledge panel
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      showKnowledge = !showKnowledge;
    }
    // Cmd+B = toggle sidebar
    if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
      e.preventDefault();
      showSidebar = !showSidebar;
    }
    // Cmd+O = open project
    if ((e.metaKey || e.ctrlKey) && e.key === 'o') {
      e.preventDefault();
      openProject();
    }
    // Cmd+, = settings
    if ((e.metaKey || e.ctrlKey) && e.key === ',') {
      e.preventDefault();
      showSettings = !showSettings;
    }
    // Escape = close path input
    if (e.key === 'Escape' && showWebPathInput) {
      showWebPathInput = false;
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<main class="app">
  <ModelBar onsettings={() => showSettings = true} />

  <div class="workspace">
    {#if project && showSidebar}
      <Sidebar ontoggle={() => showSidebar = !showSidebar} onfileselect={handleFileSelect} />
    {/if}

    <div class="main-area">
      {#if project}
        <Chat />
      {:else}
        <Welcome onopen={openProject} />
      {/if}
    </div>

    {#if selectedFile}
      <FileViewer filepath={selectedFile} onclose={() => selectedFile = null} />
    {/if}

    {#if showKnowledge}
      <KnowledgePanel onclose={() => showKnowledge = false} />
    {/if}
  </div>

  <StatusBar />
</main>

{#if showSettings}
  <Settings onclose={() => showSettings = false} />
{/if}

{#if showWebPathInput}
  <div class="path-overlay" role="dialog" aria-modal="true">
    <div class="path-modal">
      <h3>Open Project</h3>
      <p class="path-hint">Enter the absolute path on this server</p>
      <input
        type="text"
        bind:value={webPathInput}
        placeholder="/Users/m/my-project"
        onkeydown={(e) => e.key === 'Enter' && submitWebPath()}
        autofocus
        class="path-input"
      />
      <div class="path-shortcuts">
        <span class="path-shortcut-label">Quick open:</span>
        {#each ['/Users/m/tauri-code', '/Users/m/aisvet', '/Users/m/claween'] as p}
          <button class="path-quick" onclick={() => { webPathInput = p; submitWebPath(); }}>{p.split('/').pop()}</button>
        {/each}
      </div>
      <div class="path-actions">
        <button class="path-cancel" onclick={() => showWebPathInput = false}>Cancel</button>
        <button class="path-submit" onclick={submitWebPath} disabled={!webPathInput.trim()}>Open</button>
      </div>
    </div>
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    font-family: 'SF Mono', 'Fira Code', 'JetBrains Mono', monospace;
    background: #0a0e17;
    color: #e2e8f0;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  /* Web path input modal */
  .path-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .path-modal {
    background: #0f1623;
    border: 1px solid rgba(96, 165, 250, 0.3);
    border-radius: 12px;
    padding: 1.5rem 2rem;
    width: 480px;
    max-width: 90vw;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(96, 165, 250, 0.1);
  }

  .path-modal h3 {
    margin: 0 0 0.25rem;
    color: #e2e8f0;
    font-size: 1rem;
  }

  .path-hint {
    color: #475569;
    font-size: 0.75rem;
    margin: 0 0 0.75rem;
  }

  .path-input {
    width: 100%;
    box-sizing: border-box;
    background: #0a0e17;
    border: 1px solid rgba(96, 165, 250, 0.3);
    border-radius: 8px;
    color: #e2e8f0;
    font-family: inherit;
    font-size: 0.85rem;
    padding: 0.6rem 0.75rem;
    outline: none;
    transition: border-color 0.2s;
  }

  .path-input:focus {
    border-color: #60a5fa;
    box-shadow: 0 0 0 2px rgba(96, 165, 250, 0.15);
  }

  .path-shortcuts {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-top: 0.5rem;
    flex-wrap: wrap;
  }

  .path-shortcut-label {
    color: #334155;
    font-size: 0.7rem;
  }

  .path-quick {
    background: transparent;
    border: 1px solid rgba(96, 165, 250, 0.2);
    color: #60a5fa;
    border-radius: 4px;
    padding: 0.15rem 0.5rem;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .path-quick:hover {
    background: rgba(96, 165, 250, 0.1);
    border-color: rgba(96, 165, 250, 0.5);
  }

  .path-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .path-cancel {
    background: transparent;
    border: 1px solid #1e293b;
    color: #475569;
    border-radius: 6px;
    padding: 0.4rem 1rem;
    font-family: inherit;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .path-cancel:hover {
    border-color: #334155;
    color: #64748b;
  }

  .path-submit {
    background: linear-gradient(135deg, #3b82f6, #6366f1);
    border: none;
    color: white;
    border-radius: 6px;
    padding: 0.4rem 1rem;
    font-family: inherit;
    font-size: 0.8rem;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .path-submit:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .path-submit:not(:disabled):hover {
    opacity: 0.85;
  }
</style>
