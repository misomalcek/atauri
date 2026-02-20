import { writable } from 'svelte/store';

const initial = {
  lm_studio_port: 1234,
  claude_api_key: null,
  qdrant_url: 'http://localhost:6333',
  complexity_threshold: 0.4,
  default_model: null,
};

export const settingsStore = writable(initial);

export function updateSettings(settings) {
  settingsStore.set(settings);
}
