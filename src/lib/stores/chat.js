import { writable } from 'svelte/store';

const initial = {
  messages: [],
  loading: false,
  streamingContent: '',
};

export const chatStore = writable(initial);

export function addMessage(role, content, decision = null, guardian = null) {
  chatStore.update(state => ({
    ...state,
    messages: [
      ...state.messages,
      {
        id: crypto.randomUUID(),
        role,
        content,
        decision,
        guardian,
        timestamp: new Date().toISOString(),
      }
    ]
  }));
}

export function appendToStream(token) {
  chatStore.update(state => ({
    ...state,
    streamingContent: state.streamingContent + token,
  }));
}

export function finalizeStream(decision = null, guardian = null) {
  chatStore.update(state => {
    const content = state.streamingContent;
    return {
      ...state,
      streamingContent: '',
      loading: false,
      messages: [
        ...state.messages,
        {
          id: crypto.randomUUID(),
          role: 'assistant',
          content,
          decision,
          guardian,
          timestamp: new Date().toISOString(),
        }
      ]
    };
  });
}

export function setLoading(loading) {
  chatStore.update(state => ({ ...state, loading }));
}

export function clearChat() {
  chatStore.set({ ...initial, messages: [] });
}
