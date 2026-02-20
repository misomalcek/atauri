import { writable } from 'svelte/store';

const initial = {
  available: [],
  override: null, // null = auto-route (The Weaver decides)
};

export const modelStore = writable(initial);

export function setModels(models) {
  modelStore.update(state => ({ ...state, available: models }));
}

export function setOverride(modelName) {
  modelStore.update(state => ({ ...state, override: modelName }));
}
