/**
 * Backend abstraction layer
 * Tauri desktop: routes to @tauri-apps/api/core invoke()
 * Web browser:   routes to fetch('/api/') HTTP calls
 *
 * Drop-in replacement: just change the import path in components.
 * invoke() signature is identical to @tauri-apps/api/core.
 */

// Tauri v2 uses __TAURI_INTERNALS__, v1 used __TAURI__
const isTauri = typeof window !== 'undefined' &&
  !!(window.__TAURI_INTERNALS__ ?? window.__TAURI__);

// ──────────────────────────────────────
// Arg normalizers
//
// Some components pass nested arg wrappers that Tauri IPC uses as
// named parameters, but Axum expects a flat JSON body. These
// normalizers transform args before fetch() in web mode only.
// ──────────────────────────────────────

const ARG_NORMALIZERS = {
  // Chat.svelte: invoke('chat', { request: { message, model_override, temperature } })
  // Axum ChatReq expects flat body: { message, model_override, temperature }
  chat: (args) => args.request ?? args,

  // Settings.svelte: invoke('save_settings', { settings: { lm_studio_port, ... } })
  // Axum expects AppConfig directly
  save_settings: (args) => args.settings ?? args,

  // Chat.svelte: invoke('search_knowledge', { query, collection, limit, pathFilter })
  // Axum SearchReq uses snake_case: path_filter
  search_knowledge: (args) => ({
    query: args.query,
    collection: args.collection ?? null,
    limit: args.limit ?? 5,
    path_filter: args.pathFilter ?? args.path_filter ?? null,
  }),

  // Chat.svelte: invoke('execute_action', { action: { id, kind, description, details } })
  // Axum expects ProposedAction directly
  execute_action: (args) => args.action ?? args,
};

// Commands with no web implementation (all commands now have Axum routes)
const MISSING_WEB_COMMANDS = new Set([]);

// ──────────────────────────────────────
// Core invoke resolver
// ──────────────────────────────────────

let _nativeInvoke = null;

async function getInvoke() {
  if (_nativeInvoke) return _nativeInvoke;

  if (isTauri) {
    const mod = await import('@tauri-apps/api/core');
    _nativeInvoke = mod.invoke;
  } else {
    _nativeInvoke = async (command, args = {}) => {
      if (MISSING_WEB_COMMANDS.has(command)) {
        console.warn(`[api] '${command}' not available in web mode`);
        return [];
      }

      const normalizer = ARG_NORMALIZERS[command];
      const body = normalizer ? normalizer(args) : args;

      const resp = await fetch(`/api/${command}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });

      if (!resp.ok) {
        const text = await resp.text();
        throw new Error(text || `HTTP ${resp.status} ${resp.statusText}`);
      }

      return resp.json();
    };
  }

  return _nativeInvoke;
}

/** Drop-in replacement for @tauri-apps/api/core invoke — identical signature */
export async function invoke(command, args = {}) {
  const fn = await getInvoke();
  return fn(command, args);
}

/** Same as invoke — use this name for clarity in new code */
export async function call(command, args = {}) {
  return invoke(command, args);
}

/** Check if running inside Tauri desktop app */
export function isDesktop() {
  return isTauri;
}

/** Open native folder dialog (Tauri) or return null (web → triggers path input UI) */
export async function openFolderDialog() {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ directory: true, multiple: false });
  }
  return null;
}

// ──────────────────────────────────────
// Event / streaming
// ──────────────────────────────────────

const _listeners = new Map();

/** Listen for events. Tauri: native events. Web: WebSocket-backed. */
export async function listen(event, callback) {
  if (isTauri) {
    const { listen: tauriListen } = await import('@tauri-apps/api/event');
    return tauriListen(event, callback);
  }

  if (!_listeners.has(event)) {
    _listeners.set(event, new Set());
  }
  _listeners.get(event).add(callback);
  return () => _listeners.get(event)?.delete(callback);
}

function getWsUrl() {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${window.location.host}/ws/stream`;
}

/**
 * Streaming chat.
 * Web:   opens WebSocket to /ws/stream, dispatches 'chat-token' events per token
 * Tauri: uses regular invoke('chat') and dispatches a single done event
 *
 * Each 'chat-token' payload: { token: string, done: boolean, decision?, guardian? }
 * Returns a Promise that resolves with the RouteDecision when streaming is complete.
 */
export function streamChat(message, modelOverride = null, temperature = null) {
  if (isTauri) {
    // Tauri has no streaming — invoke full response and simulate done event
    return invoke('chat', {
      request: { message, model_override: modelOverride, temperature }
    }).then(response => {
      const listeners = _listeners.get('chat-token');
      if (listeners) {
        for (const cb of listeners) {
          cb({ payload: {
            token: response.content,
            done: true,
            decision: response.decision,
            guardian: response.guardian,
          }});
        }
      }
      return response.decision ?? null;
    });
  }

  // Web: true streaming via WebSocket
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(getWsUrl());

    const timeout = setTimeout(() => {
      ws.close();
      reject(new Error('WebSocket connection timeout'));
    }, 10000);

    ws.onopen = () => {
      clearTimeout(timeout);
      ws.send(JSON.stringify({
        message,
        model_override: modelOverride,
        temperature,
      }));
    };

    ws.onmessage = (event) => {
      let token;
      try { token = JSON.parse(event.data); } catch { return; }

      const listeners = _listeners.get('chat-token');
      if (listeners) {
        for (const cb of listeners) cb({ payload: token });
      }

      if (token.done) {
        ws.close();
        resolve(token.decision ?? null);
      }
    };

    ws.onerror = () => reject(new Error('WebSocket connection failed'));
    ws.onclose = () => {};
  });
}
