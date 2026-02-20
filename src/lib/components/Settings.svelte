<script>
  import { invoke } from '../api/index.js';
  import { settingsStore, updateSettings } from '../stores/settings';
  import { onMount } from 'svelte';

  let { onclose } = $props();

  let settings = $state({
    lm_studio_port: 1234,
    claude_api_key: null,
    qdrant_url: 'http://localhost:6333',
    complexity_threshold: 0.4,
    default_model: null,
  });

  let promptState = $state({
    system_prompt: '',
    core_entries_count: 0,
    core_preview: '',
    ratio: 0.5,
  });

  let systemPromptDraft = $state('');
  let ratioDraft = $state(0.5);
  let saving = $state(false);
  let status = $state('');
  let activeTab = $state('connections');

  onMount(async () => {
    try {
      const loaded = await invoke('load_settings');
      settings = loaded;
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
    try {
      const ps = await invoke('get_prompt_state');
      promptState = ps;
      systemPromptDraft = ps.system_prompt;
      ratioDraft = ps.ratio;
    } catch (_) {
      // Prompt engine not available until project is opened
    }
  });

  async function saveSettings() {
    saving = true;
    status = '';
    try {
      await invoke('save_settings', { settings });
      updateSettings(settings);
      status = 'saved';
      setTimeout(() => status = '', 2000);
    } catch (e) {
      status = 'error: ' + e;
    } finally {
      saving = false;
    }
  }

  async function saveSystemPrompt() {
    try {
      await invoke('update_system_prompt', { prompt: systemPromptDraft });
      status = 'prompt saved';
      setTimeout(() => status = '', 2000);
    } catch (e) {
      status = 'error: ' + e;
    }
  }

  async function saveRatio() {
    try {
      await invoke('set_prompt_ratio', { ratio: ratioDraft });
      status = 'ratio saved';
      setTimeout(() => status = '', 2000);
    } catch (e) {
      status = 'error: ' + e;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay" onclick={onclose}>
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2>Settings</h2>
      <button class="close-btn" onclick={onclose}>&times;</button>
    </div>

    <div class="tabs">
      <button class:active={activeTab === 'connections'} onclick={() => activeTab = 'connections'}>
        Connections
      </button>
      <button class:active={activeTab === 'prompt'} onclick={() => activeTab = 'prompt'}>
        Prompt Engine
      </button>
      <button class:active={activeTab === 'routing'} onclick={() => activeTab = 'routing'}>
        Routing
      </button>
    </div>

    <div class="content">
      {#if activeTab === 'connections'}
        <div class="section">
          <h3>LM Studio</h3>
          <label>
            <span>Port</span>
            <input type="number" bind:value={settings.lm_studio_port} min="1" max="65535" />
          </label>
        </div>

        <div class="section">
          <h3>Claude API</h3>
          <label>
            <span>API Key</span>
            <input
              type="password"
              bind:value={settings.claude_api_key}
              placeholder="sk-ant-..."
            />
          </label>
        </div>

        <div class="section">
          <h3>Qdrant</h3>
          <label>
            <span>URL</span>
            <input type="text" bind:value={settings.qdrant_url} placeholder="http://localhost:6333" />
          </label>
        </div>

      {:else if activeTab === 'prompt'}
        <div class="section">
          <h3>System Prompt <span class="hint">(user-editable)</span></h3>
          <textarea
            class="prompt-editor"
            bind:value={systemPromptDraft}
            rows="6"
          ></textarea>
          <button class="action-btn" onclick={saveSystemPrompt}>Save System Prompt</button>
        </div>

        <div class="section">
          <h3>Core Prompt <span class="hint">(AI-evolving, read-only)</span></h3>
          <div class="core-info">
            <span class="badge">{promptState.core_entries_count} entries</span>
          </div>
          {#if promptState.core_preview}
            <pre class="core-preview">{promptState.core_preview}</pre>
          {:else}
            <p class="empty">No core entries yet. The AI will learn as you interact.</p>
          {/if}
        </div>

        <div class="section">
          <h3>Ratio</h3>
          <div class="ratio-control">
            <span class="ratio-label">System {Math.round((1 - ratioDraft) * 100)}%</span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              bind:value={ratioDraft}
              oninput={() => {}}
            />
            <span class="ratio-label">Core {Math.round(ratioDraft * 100)}%</span>
          </div>
          <button class="action-btn" onclick={saveRatio}>Apply Ratio</button>
        </div>

      {:else if activeTab === 'routing'}
        <div class="section">
          <h3>Routing Spectrum</h3>
          <div class="spectrum-visual">
            <div class="spectrum-bar">
              <div class="zone local" style="width: {(settings.local_ceiling ?? 0.25) * 100}%">LOCAL</div>
              <div class="zone weaver" style="width: {((settings.cloud_floor ?? 0.65) - (settings.local_ceiling ?? 0.25)) * 100}%">WEAVER</div>
              <div class="zone cloud" style="width: {(1 - (settings.cloud_floor ?? 0.65)) * 100}%">CLOUD</div>
            </div>
          </div>

          <div class="threshold-control">
            <span class="ratio-label" style="color: #6ee7b7">Local ceiling</span>
            <input
              type="range"
              min="0.05" max="0.5" step="0.05"
              bind:value={settings.local_ceiling}
            />
            <span class="ratio-label">{(settings.local_ceiling ?? 0.25).toFixed(2)}</span>
          </div>

          <div class="threshold-control">
            <span class="ratio-label" style="color: #a78bfa">Cloud floor</span>
            <input
              type="range"
              min="0.3" max="0.95" step="0.05"
              bind:value={settings.cloud_floor}
            />
            <span class="ratio-label">{(settings.cloud_floor ?? 0.65).toFixed(2)}</span>
          </div>

          <p class="hint">
            Below {(settings.local_ceiling ?? 0.25).toFixed(2)} = always local.
            Above {(settings.cloud_floor ?? 0.65).toFixed(2)} = always cloud.
            Between = The Weaver decides based on task type.
          </p>
        </div>

        <div class="section">
          <h3>Weaver Zone Factors</h3>
          <label class="toggle-row">
            <input type="checkbox" bind:checked={settings.prefer_local_for_code} />
            <span>Prefer local for code tasks</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" bind:checked={settings.prefer_cloud_for_reasoning} />
            <span>Prefer cloud for reasoning/analysis</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" bind:checked={settings.conversation_depth_factor} />
            <span>Escalate to cloud in deep conversations</span>
          </label>
        </div>

        <div class="section">
          <h3>Default Model</h3>
          <input
            type="text"
            bind:value={settings.default_model}
            placeholder="auto (Weaver decides)"
          />
        </div>
      {/if}
    </div>

    <div class="modal-footer">
      {#if status}
        <span class="status" class:error={status.startsWith('error')}>{status}</span>
      {/if}
      <button class="save-btn" onclick={saveSettings} disabled={saving}>
        {saving ? 'Saving...' : 'Save Settings'}
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .modal {
    background: #0f172a;
    border: 1px solid #1e293b;
    border-radius: 12px;
    width: min(560px, 90vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 25px 60px rgba(0, 0, 0, 0.5);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #1e293b;
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 400;
    letter-spacing: 0.05em;
    color: #e2e8f0;
  }

  .close-btn {
    background: none;
    border: none;
    color: #475569;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }
  .close-btn:hover { color: #e2e8f0; }

  .tabs {
    display: flex;
    padding: 0 1.5rem;
    border-bottom: 1px solid #1e293b;
    gap: 0;
  }

  .tabs button {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: #475569;
    padding: 0.75rem 1rem;
    font-family: inherit;
    font-size: 0.8rem;
    cursor: pointer;
    letter-spacing: 0.03em;
    transition: all 0.2s;
  }

  .tabs button:hover { color: #94a3b8; }
  .tabs button.active {
    color: #60a5fa;
    border-bottom-color: #60a5fa;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
  }

  .section {
    margin-bottom: 1.5rem;
  }

  .section h3 {
    margin: 0 0 0.6rem 0;
    font-size: 0.85rem;
    font-weight: 500;
    color: #94a3b8;
    letter-spacing: 0.03em;
  }

  .hint {
    color: #475569;
    font-weight: 300;
    font-size: 0.75rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  label span {
    font-size: 0.75rem;
    color: #64748b;
  }

  input[type="text"],
  input[type="password"],
  input[type="number"] {
    background: #1e293b;
    border: 1px solid #334155;
    color: #e2e8f0;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.2s;
  }

  input:focus {
    border-color: #60a5fa;
  }

  .prompt-editor {
    width: 100%;
    background: #1e293b;
    border: 1px solid #334155;
    color: #e2e8f0;
    padding: 0.8rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.8rem;
    resize: vertical;
    outline: none;
    line-height: 1.5;
    box-sizing: border-box;
  }

  .prompt-editor:focus {
    border-color: #60a5fa;
  }

  .core-info {
    margin-bottom: 0.5rem;
  }

  .badge {
    background: #1e293b;
    color: #6ee7b7;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
    letter-spacing: 0.03em;
  }

  .core-preview {
    background: #1e293b;
    border: 1px solid #334155;
    border-radius: 6px;
    padding: 0.8rem;
    font-size: 0.75rem;
    color: #94a3b8;
    overflow-x: auto;
    white-space: pre-wrap;
    margin: 0;
  }

  .empty {
    color: #475569;
    font-style: italic;
    font-size: 0.8rem;
  }

  .ratio-control, .threshold-control {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.5rem;
  }

  .ratio-label {
    font-size: 0.75rem;
    color: #64748b;
    min-width: 5rem;
    text-align: center;
  }

  input[type="range"] {
    flex: 1;
    accent-color: #60a5fa;
    height: 4px;
  }

  .action-btn {
    margin-top: 0.5rem;
    padding: 0.4rem 1rem;
    background: #1e293b;
    border: 1px solid #334155;
    color: #94a3b8;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .action-btn:hover {
    border-color: #60a5fa;
    color: #e2e8f0;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 1rem;
    padding: 1rem 1.5rem;
    border-top: 1px solid #1e293b;
  }

  .status {
    font-size: 0.75rem;
    color: #6ee7b7;
  }

  .status.error {
    color: #fca5a5;
  }

  .save-btn {
    padding: 0.5rem 1.5rem;
    background: #1e40af;
    border: none;
    color: white;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .save-btn:hover:not(:disabled) { background: #2563eb; }
  .save-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .spectrum-visual { margin-bottom: 1rem; }
  .spectrum-bar {
    display: flex;
    height: 28px;
    border-radius: 6px;
    overflow: hidden;
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.05em;
  }
  .zone {
    display: flex;
    align-items: center;
    justify-content: center;
    transition: width 0.3s ease;
  }
  .zone.local { background: #065f4620; color: #6ee7b7; border: 1px solid #6ee7b740; }
  .zone.weaver { background: #3b82f620; color: #60a5fa; border: 1px solid #60a5fa40; }
  .zone.cloud { background: #7c3aed20; color: #a78bfa; border: 1px solid #a78bfa40; }

  .toggle-row {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 0;
    cursor: pointer;
  }
  .toggle-row input[type="checkbox"] {
    accent-color: #60a5fa;
    width: 16px;
    height: 16px;
  }
  .toggle-row span {
    font-size: 0.8rem;
    color: #94a3b8;
  }
</style>
