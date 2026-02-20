<script>
  import { invoke } from '../api/index.js';
  import { modelStore, setModels, setOverride } from '../stores/models';
  import { onMount } from 'svelte';

  let { onsettings } = $props();

  let discovering = $state(false);
  let models = $derived($modelStore);

  onMount(async () => {
    await discoverModels();
  });

  async function discoverModels() {
    discovering = true;
    try {
      const result = await invoke('list_models');
      setModels(result);
    } catch (e) {
      console.error('Model discovery failed:', e);
    } finally {
      discovering = false;
    }
  }

  function selectModel(id) {
    if (models.override === id) {
      setOverride(null);
    } else {
      setOverride(id);
    }
  }
</script>

<div class="model-bar">
  <div class="bar-left">
    <span class="label">Models</span>
    {#if discovering}
      <span class="discovering">scanning...</span>
    {/if}
  </div>

  <div class="models">
    <button
      class="model-chip auto"
      class:active={!models.override}
      onclick={() => setOverride(null)}
    >
      Auto (Weaver)
    </button>

    {#each models.available as model}
      <button
        class="model-chip {model.tier.toLowerCase()}"
        class:active={models.override === model.name}
        onclick={() => selectModel(model.name)}
        title="{model.id} ({model.tier})"
      >
        {model.name}
      </button>
    {/each}
  </div>

  <button class="refresh" onclick={discoverModels} title="Rescan models">
    &#x21bb;
  </button>

  <button class="settings-btn" onclick={onsettings} title="Settings">
    &#x2699;
  </button>
</div>

<style>
  .model-bar {
    display: flex;
    align-items: center;
    padding: 0.4rem 1rem;
    background: #0f172a;
    border-bottom: 1px solid #1e293b;
    gap: 1rem;
    overflow-x: auto;
  }
  .bar-left {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .label {
    color: #475569;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .discovering {
    color: #60a5fa;
    font-size: 0.7rem;
    animation: pulse 1s ease-in-out infinite;
  }
  .models {
    display: flex;
    gap: 0.4rem;
    flex-wrap: nowrap;
  }
  .model-chip {
    padding: 0.25rem 0.6rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-family: inherit;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.15s;
    white-space: nowrap;
  }
  .model-chip.auto { background: #1e293b; color: #94a3b8; }
  .model-chip.local { background: #052e16; color: #6ee7b7; }
  .model-chip.cloud { background: #1e1b4b; color: #a5b4fc; }
  .model-chip.active {
    border-color: #60a5fa;
    box-shadow: 0 0 8px rgba(96, 165, 250, 0.2);
  }
  .model-chip:hover { border-color: #475569; }
  .refresh {
    background: none;
    border: none;
    color: #475569;
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0.2rem;
    flex-shrink: 0;
  }
  .refresh:hover { color: #60a5fa; }
  .settings-btn {
    background: none;
    border: none;
    color: #475569;
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0.2rem;
    flex-shrink: 0;
    margin-left: auto;
  }
  .settings-btn:hover { color: #60a5fa; }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
