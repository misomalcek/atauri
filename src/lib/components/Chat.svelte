<script>
  import { invoke, isDesktop, streamChat, listen } from '../api/index.js';
  import { chatStore, addMessage, setLoading, appendToStream, finalizeStream, clearChat } from '../stores/chat';
  import { modelStore, setOverride } from '../stores/models';
  import { Marked } from 'marked';
  import hljs from 'highlight.js/lib/core';
  import javascript from 'highlight.js/lib/languages/javascript';
  import typescript from 'highlight.js/lib/languages/typescript';
  import rust from 'highlight.js/lib/languages/rust';
  import python from 'highlight.js/lib/languages/python';
  import json from 'highlight.js/lib/languages/json';
  import bash from 'highlight.js/lib/languages/bash';
  import css from 'highlight.js/lib/languages/css';
  import xml from 'highlight.js/lib/languages/xml';

  hljs.registerLanguage('javascript', javascript);
  hljs.registerLanguage('js', javascript);
  hljs.registerLanguage('typescript', typescript);
  hljs.registerLanguage('ts', typescript);
  hljs.registerLanguage('rust', rust);
  hljs.registerLanguage('rs', rust);
  hljs.registerLanguage('python', python);
  hljs.registerLanguage('py', python);
  hljs.registerLanguage('json', json);
  hljs.registerLanguage('bash', bash);
  hljs.registerLanguage('sh', bash);
  hljs.registerLanguage('css', css);
  hljs.registerLanguage('html', xml);
  hljs.registerLanguage('svelte', xml);

  const marked = new Marked({
    renderer: {
      code({ text, lang }) {
        const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
        let highlighted;
        try {
          highlighted = lang ? hljs.highlight(text, { language }).value : text;
        } catch {
          highlighted = text;
        }
        return `<div class="code-block"><div class="code-header"><span class="code-lang">${language}</span><button class="copy-btn" onclick="navigator.clipboard.writeText(decodeURIComponent('${encodeURIComponent(text)}'))">copy</button></div><pre><code class="hljs language-${language}">${highlighted}</code></pre></div>`;
      }
    }
  });

  let input = $state('');
  let chatContainer = $state(null);
  let chat = $derived($chatStore);
  let models = $derived($modelStore);
  let showSlashMenu = $state(false);
  let slashFilter = $state('');
  let pendingActions = $state([]);

  const slashCommands = [
    { cmd: '/search', desc: 'Semantic code search' },
    { cmd: '/browse', desc: 'Browse collection data' },
    { cmd: '/collections', desc: 'List Qdrant collections' },
    { cmd: '/deliberate', desc: 'Snow Globe multi-model' },
    { cmd: '/write', desc: 'Save last response to file' },
    { cmd: '/create', desc: 'Create a new file' },
    { cmd: '/files', desc: 'List all project files' },
    { cmd: '/index', desc: 'Index dir into collection' },
    { cmd: '/read', desc: 'Show file content in chat' },
    { cmd: '/git', desc: 'Git status overview' },
    { cmd: '/diff', desc: 'Show git changes' },
    { cmd: '/log', desc: 'Recent git commits' },
    { cmd: '/commit', desc: 'Stage all + commit' },
    { cmd: '/push', desc: 'Push to remote' },
    { cmd: '/pull', desc: 'Pull from remote' },
    { cmd: '/model', desc: 'Switch model' },
    { cmd: '/ghost', desc: 'Remember something' },
    { cmd: '/clear', desc: 'Clear chat' },
  ];

  let filteredCommands = $derived(
    slashFilter
      ? slashCommands.filter(c => c.cmd.startsWith('/' + slashFilter))
      : slashCommands
  );

  function renderMarkdown(content) {
    if (!content) return '';
    return marked.parse(content);
  }

  async function send() {
    if (!input.trim()) return;
    const userMsg = input.trim();

    // Handle slash commands
    if (userMsg.startsWith('/')) {
      const parts = userMsg.split(' ');
      const arg = parts.slice(1).join(' ');
      input = '';
      showSlashMenu = false;

      switch (parts[0]) {
        case '/search': await handleSearch(arg); return;
        case '/browse': await handleBrowse(arg); return;
        case '/collections': await handleCollections(); return;
        case '/deliberate': await handleDeliberate(arg); return;
        case '/write': await handleWrite(arg); return;
        case '/create': await handleCreate(arg); return;
        case '/files': await handleListFiles(); return;
        case '/index': await handleIndex(arg); return;
        case '/read': await handleReadFile(arg); return;
        case '/git': await handleGitStatus(); return;
        case '/diff': await handleGitDiff(arg); return;
        case '/log': await handleGitLog(arg); return;
        case '/commit': await handleGitCommit(arg); return;
        case '/push': await handleGitPush(); return;
        case '/pull': await handleGitPull(); return;
        case '/model': handleModel(arg); return;
        case '/ghost': await handleGhost(arg); return;
        case '/clear': clearChat(); invoke('clear_conversation').catch(() => {}); pendingActions = []; return;
      }
    }

    input = '';
    showSlashMenu = false;
    addMessage('user', userMsg);
    setLoading(true);

    if (!isDesktop()) {
      // Web mode: streaming via WebSocket
      let accumulated = '';
      let finalDecision = null;
      let finalGuardian = null;

      const unlisten = await listen('chat-token', ({ payload }) => {
        if (!payload.done) {
          accumulated += payload.token;
          appendToStream(payload.token);
        } else {
          finalDecision = payload.decision ?? null;
          finalGuardian = payload.guardian ?? null;
        }
      });

      try {
        await streamChat(userMsg, models.override || null, null);
        const actions = parseActions(accumulated);
        if (actions.length > 0) {
          pendingActions = actions;
          finalizeStream(finalDecision, finalGuardian);
          // Replace with clean content
          const clean = stripActionBlocks(accumulated) || 'I have proposed actions for you:';
          chatStore.update(s => {
            const msgs = [...s.messages];
            msgs[msgs.length - 1] = { ...msgs[msgs.length - 1], content: clean };
            return { ...s, messages: msgs };
          });
        } else {
          finalizeStream(finalDecision, finalGuardian);
        }
      } catch (e) {
        finalizeStream(null, null);
        addMessage('system', `Error: ${e}`);
      } finally {
        unlisten();
        setLoading(false);
      }
    } else {
      // Desktop mode: regular invoke (non-streaming)
      try {
        const response = await invoke('chat', {
          request: {
            message: userMsg,
            model_override: models.override || null,
            temperature: null,
          }
        });

        const actions = parseActions(response.content);
        if (actions.length > 0) {
          pendingActions = actions;
          const cleanContent = stripActionBlocks(response.content);
          addMessage('assistant', cleanContent || 'I have proposed actions for you:', response.decision, response.guardian ?? null);
        } else {
          addMessage('assistant', response.content, response.decision, response.guardian ?? null);
        }
      } catch (e) {
        addMessage('system', `Error: ${e}`);
      } finally {
        setLoading(false);
      }
    }

    scrollToBottom();
  }

  function scrollToBottom() {
    setTimeout(() => {
      if (chatContainer) chatContainer.scrollTop = chatContainer.scrollHeight;
    }, 50);
  }

  function handleKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  function handleInput() {
    if (input.startsWith('/') && !input.includes(' ')) {
      showSlashMenu = true;
      slashFilter = input.slice(1);
    } else {
      showSlashMenu = false;
    }
  }

  function selectSlashCommand(cmd) {
    input = cmd.cmd + ' ';
    showSlashMenu = false;
  }

  async function ensureQdrant() {
    try {
      await invoke('connect_qdrant');
      return true;
    } catch (e) {
      addMessage('system', `Qdrant connection failed: ${e}`);
      return false;
    }
  }

  async function handleSearch(query) {
    if (!query) {
      addMessage('system', 'Usage: /search <query>');
      return;
    }
    addMessage('user', `/search ${query}`);
    setLoading(true);
    try {
      const results = await invoke('search_knowledge', {
        query, collection: null, limit: 5, pathFilter: null,
      });
      const formatted = results.map(r =>
        `**${r.file_path}** (L${r.start_line}-${r.end_line}, score: ${r.score.toFixed(3)})\n\`\`\`\n${r.code_chunk.slice(0, 300)}\n\`\`\``
      ).join('\n\n');
      addMessage('assistant', formatted || 'No results found.');
    } catch (e) {
      // Auto-connect and retry once
      if (String(e).includes('not connected') && await ensureQdrant()) {
        try {
          const results = await invoke('search_knowledge', {
            query, collection: null, limit: 5, pathFilter: null,
          });
          const formatted = results.map(r =>
            `**${r.file_path}** (L${r.start_line}-${r.end_line}, score: ${r.score.toFixed(3)})\n\`\`\`\n${r.code_chunk.slice(0, 300)}\n\`\`\``
          ).join('\n\n');
          addMessage('assistant', formatted || 'No results found.');
        } catch (e2) {
          addMessage('system', `Search error: ${e2}`);
        }
      } else {
        addMessage('system', `Search error: ${e}`);
      }
    }
    setLoading(false);
  }

  async function handleCollections() {
    addMessage('user', '/collections');
    setLoading(true);
    try {
      let collections;
      try {
        collections = await invoke('list_knowledge_collections');
      } catch {
        // Auto-connect and retry
        if (!await ensureQdrant()) { setLoading(false); return; }
        collections = await invoke('list_knowledge_collections');
      }
      if (collections.length === 0) {
        addMessage('assistant', 'No Qdrant collections found. Use the Knowledge Panel (Cmd+K) to index a project.');
      } else {
        const formatted = `**Qdrant Collections** (${collections.length})\n\n` +
          collections.map(c => `- **${c.name}** — ${c.points_count.toLocaleString()} vectors`).join('\n');
        addMessage('assistant', formatted);
      }
    } catch (e) {
      addMessage('system', `Collections error: ${e}`);
    }
    setLoading(false);
  }

  async function handleBrowse(collectionName) {
    if (!isDesktop()) {
      addMessage('system', '/browse is not yet available in web mode. Use /search instead.');
      return;
    }
    if (!collectionName) {
      addMessage('system', 'Usage: /browse <collection-name>  (use /collections to see names)');
      return;
    }
    addMessage('user', `/browse ${collectionName}`);
    setLoading(true);
    try {
      let results;
      try {
        results = await invoke('browse_knowledge', { collection: collectionName, limit: 10 });
      } catch {
        if (!await ensureQdrant()) { setLoading(false); return; }
        results = await invoke('browse_knowledge', { collection: collectionName, limit: 10 });
      }
      if (results.length === 0) {
        addMessage('assistant', `Collection **${collectionName}** is empty.`);
      } else {
        const formatted = `**Browsing ${collectionName}** (${results.length} samples)\n\n` +
          results.map(r =>
            `**${r.file_path || '(no path)'}** (L${r.start_line}-${r.end_line})\n\`\`\`\n${r.code_chunk.slice(0, 200)}\n\`\`\``
          ).join('\n\n');
        addMessage('assistant', formatted);
      }
    } catch (e) {
      addMessage('system', `Browse error: ${e}`);
    }
    setLoading(false);
  }

  async function handleDeliberate(query) {
    if (!query) {
      addMessage('system', 'Usage: /deliberate <question>');
      return;
    }
    addMessage('user', `/deliberate ${query}`);
    setLoading(true);
    try {
      const result = await invoke('deliberate', { message: query });
      let md = '## Snow Globe Deliberation\n\n';
      for (const resp of result.responses) {
        md += `### ${resp.model_name} ${resp.success ? '' : '(failed)'}\n\n${resp.content}\n\n---\n\n`;
      }
      if (result.consensus) {
        md += `### Consensus\n\n${result.consensus}\n\n`;
      }
      if (result.divergence_points.length > 0) {
        md += `### Divergence Points\n\n${result.divergence_points.map(d => `- ${d}`).join('\n')}\n`;
      }
      addMessage('assistant', md);
    } catch (e) {
      addMessage('system', `Deliberation error: ${e}`);
    }
    setLoading(false);
  }

  function handleModel(name) {
    if (!name) {
      addMessage('system', 'Usage: /model <name> or /model auto');
      return;
    }
    if (name === 'auto') {
      setOverride(null);
      addMessage('system', 'Switched to Auto (Weaver)');
    } else {
      setOverride(name);
      addMessage('system', `Switched to model: ${name}`);
    }
  }

  async function handleGhost(content) {
    if (!content) {
      addMessage('system', 'Usage: /ghost <something to remember>');
      return;
    }
    try {
      await invoke('ghost_remember', { kind: 'preference', content });
      addMessage('system', `Ghost remembered: ${content}`);
    } catch (e) {
      addMessage('system', `Ghost error: ${e}`);
    }
  }

  async function handleWrite(path) {
    if (!path) {
      addMessage('system', 'Usage: /write <filepath> — saves last AI response to file');
      return;
    }
    // Find last assistant message
    const lastAssistant = [...chat.messages].reverse().find(m => m.role === 'assistant');
    if (!lastAssistant) {
      addMessage('system', 'No AI response to save.');
      return;
    }
    addMessage('user', `/write ${path}`);
    try {
      await invoke('write_file_content', { path, content: lastAssistant.content });
      addMessage('system', `Saved to **${path}**`);
    } catch (e) {
      addMessage('system', `Write error: ${e}`);
    }
  }

  async function handleCreate(path) {
    if (!path) {
      addMessage('system', 'Usage: /create <filepath> [content] — creates a new file');
      return;
    }
    // Check if there's content after the path (first word = path, rest = content)
    const firstSpace = path.indexOf(' ');
    let filePath, content;
    if (firstSpace > 0) {
      filePath = path.substring(0, firstSpace);
      content = path.substring(firstSpace + 1);
    } else {
      filePath = path;
      content = '';
    }
    addMessage('user', `/create ${filePath}`);
    try {
      await invoke('write_file_content', { path: filePath, content });
      addMessage('system', `Created **${filePath}**${content ? ` (${content.length} chars)` : ' (empty)'}`);
    } catch (e) {
      addMessage('system', `Create error: ${e}`);
    }
  }

  async function handleListFiles() {
    addMessage('user', '/files');
    setLoading(true);
    try {
      const files = await invoke('list_all_files');
      if (files.length === 0) {
        addMessage('assistant', 'No files found in project.');
      } else {
        const formatted = `**Project Files** (${files.length})\n\n` +
          files.map(f => `- \`${f}\``).join('\n');
        addMessage('assistant', formatted);
      }
    } catch (e) {
      addMessage('system', `Files error: ${e}`);
    }
    setLoading(false);
  }

  async function handleIndex(arg) {
    if (!arg) {
      addMessage('system', 'Usage: /index <collection> [subdirectory]\nExample: /index aisvet-instructions new-content/instructions');
      return;
    }
    const parts = arg.split(' ');
    const collection = parts[0];
    const subdirectory = parts.slice(1).join(' ') || null;
    addMessage('user', `/index ${collection}${subdirectory ? ' ' + subdirectory : ''}`);
    setLoading(true);
    try {
      if (!await ensureQdrant()) { setLoading(false); return; }
      const count = await invoke('index_project_knowledge', {
        collection,
        subdirectory,
      });
      addMessage('assistant', `Indexed **${count}** chunks into collection **${collection}**${subdirectory ? ` from \`${subdirectory}\`` : ''}`);
    } catch (e) {
      addMessage('system', `Index error: ${e}`);
    }
    setLoading(false);
  }

  async function handleReadFile(path) {
    if (!path) {
      addMessage('system', 'Usage: /read <filepath>');
      return;
    }
    addMessage('user', `/read ${path}`);
    setLoading(true);
    try {
      const content = await invoke('read_file', { path });
      const ext = path.split('.').pop() || 'txt';
      const lang = { rs: 'rust', js: 'javascript', ts: 'typescript', py: 'python', svelte: 'svelte', md: 'markdown' }[ext] || ext;
      addMessage('assistant', `**${path}**\n\n\`\`\`${lang}\n${content}\n\`\`\``);
    } catch (e) {
      addMessage('system', `Read error: ${e}`);
    }
    setLoading(false);
  }

  // ──── Action parsing ────

  function parseActions(content) {
    const actions = [];
    const regex = /```action:(\w+)\n([\s\S]*?)```/g;
    let match;
    let i = 0;
    while ((match = regex.exec(content)) !== null) {
      const type = match[1];
      const body = match[2].trim();
      const action = parseActionBlock(type, body, i);
      if (action) {
        actions.push({ ...action, status: 'pending' });
        i++;
      }
    }
    return actions;
  }

  function parseActionBlock(type, body, index) {
    if (type === 'write_file' || type === 'create_file') {
      const lines = body.split('\n');
      const pathLine = lines[0];
      if (!pathLine.startsWith('path:')) return null;
      const path = pathLine.replace('path:', '').trim();
      const sepIdx = lines.findIndex(l => l.trim() === '---');
      const contentLines = sepIdx >= 0 ? lines.slice(sepIdx + 1) : lines.slice(1);
      const content = contentLines.join('\n');
      return {
        id: `action-${index}`,
        kind: type === 'create_file' ? 'CreateFile' : 'WriteFile',
        description: `${type === 'create_file' ? 'Create' : 'Write'} ${path}`,
        details: { FileWrite: { path, content } },
      };
    }
    if (type === 'delete_file') {
      const pathLine = body.split('\n')[0];
      if (!pathLine.startsWith('path:')) return null;
      const path = pathLine.replace('path:', '').trim();
      return {
        id: `action-${index}`,
        kind: 'DeleteFile',
        description: `Delete ${path}`,
        details: { FileDelete: { path } },
      };
    }
    if (type === 'git_commit') {
      const msgLine = body.split('\n')[0];
      if (!msgLine.startsWith('message:')) return null;
      const message = msgLine.replace('message:', '').trim();
      return {
        id: `action-${index}`,
        kind: 'GitCommit',
        description: `Commit: ${message}`,
        details: { GitCommit: { message } },
      };
    }
    if (type === 'git_push') {
      return {
        id: `action-${index}`,
        kind: 'GitPush',
        description: 'Push to remote',
        details: 'GitPush',
      };
    }
    return null;
  }

  function stripActionBlocks(content) {
    return content.replace(/```action:\w+\n[\s\S]*?```/g, '').trim();
  }

  async function applyAction(action) {
    const idx = pendingActions.findIndex(a => a.id === action.id);
    if (idx < 0) return;
    pendingActions[idx] = { ...pendingActions[idx], status: 'applying' };
    try {
      const result = await invoke('execute_action', { action: {
        id: action.id,
        kind: action.kind,
        description: action.description,
        details: action.details,
      }});
      pendingActions[idx] = { ...pendingActions[idx], status: 'applied', result: result.message };
      addMessage('system', `Applied: ${result.message}`);
    } catch (e) {
      pendingActions[idx] = { ...pendingActions[idx], status: 'failed', result: String(e) };
      addMessage('system', `Action failed: ${e}`);
    }
  }

  function skipAction(action) {
    const idx = pendingActions.findIndex(a => a.id === action.id);
    if (idx >= 0) {
      pendingActions[idx] = { ...pendingActions[idx], status: 'skipped' };
    }
  }

  async function applyAll() {
    for (const action of pendingActions) {
      if (action.status === 'pending') {
        await applyAction(action);
      }
    }
  }

  // ──── Git commands ────

  async function handleGitStatus() {
    addMessage('user', '/git');
    setLoading(true);
    try {
      const s = await invoke('git_status');
      const statusIcon = s.is_clean ? '✓' : '●';
      let md = `## Git Status ${statusIcon}\n\n`;
      md += `**Branch:** \`${s.branch}\`\n`;
      if (s.has_remote) md += `**Remote:** ${s.remote_url}\n`;
      if (s.ahead > 0 || s.behind > 0) md += `**Ahead:** ${s.ahead} | **Behind:** ${s.behind}\n`;
      md += '\n';
      if (s.is_clean) {
        md += 'Working tree clean.\n';
      } else {
        if (s.staged.length > 0) md += `**Staged** (${s.staged.length})\n` + s.staged.map(f => `- \`${f}\``).join('\n') + '\n\n';
        if (s.modified.length > 0) md += `**Modified** (${s.modified.length})\n` + s.modified.map(f => `- \`${f}\``).join('\n') + '\n\n';
        if (s.untracked.length > 0) md += `**Untracked** (${s.untracked.length})\n` + s.untracked.map(f => `- \`${f}\``).join('\n') + '\n';
      }
      addMessage('assistant', md);
    } catch (e) {
      addMessage('system', `Git error: ${e}`);
    }
    setLoading(false);
  }

  async function handleGitDiff(arg) {
    const staged = arg === 'staged';
    addMessage('user', `/diff${staged ? ' staged' : ''}`);
    setLoading(true);
    try {
      const d = await invoke('git_diff', { staged });
      if (!d.diff_text) {
        addMessage('assistant', `No ${staged ? 'staged ' : ''}changes.`);
      } else {
        let md = `## Git Diff${staged ? ' (staged)' : ''}\n\n`;
        md += `**${d.files_changed}** files, **+${d.insertions}** / **-${d.deletions}**\n\n`;
        md += '```diff\n' + d.diff_text.slice(0, 5000) + '\n```';
        if (d.diff_text.length > 5000) md += '\n\n*(truncated)*';
        addMessage('assistant', md);
      }
    } catch (e) {
      addMessage('system', `Diff error: ${e}`);
    }
    setLoading(false);
  }

  async function handleGitLog(arg) {
    const count = parseInt(arg) || 10;
    addMessage('user', `/log ${count}`);
    setLoading(true);
    try {
      const entries = await invoke('git_log', { count });
      if (entries.length === 0) {
        addMessage('assistant', 'No commits yet.');
      } else {
        let md = `## Git Log (${entries.length})\n\n`;
        md += entries.map(e =>
          `\`${e.short_hash}\` ${e.message} — *${e.author}* (${e.date.slice(0, 10)})`
        ).join('\n');
        addMessage('assistant', md);
      }
    } catch (e) {
      addMessage('system', `Log error: ${e}`);
    }
    setLoading(false);
  }

  async function handleGitCommit(message) {
    if (!message) {
      addMessage('system', 'Usage: /commit <message>');
      return;
    }
    addMessage('user', `/commit ${message}`);
    setLoading(true);
    try {
      const result = await invoke('git_commit', { message });
      addMessage('assistant', `**Committed:** ${result}`);
    } catch (e) {
      addMessage('system', `Commit error: ${e}`);
    }
    setLoading(false);
  }

  async function handleGitPush() {
    addMessage('user', '/push');
    setLoading(true);
    try {
      const result = await invoke('git_push');
      addMessage('assistant', `**Pushed:** ${result || 'Success'}`);
    } catch (e) {
      addMessage('system', `Push error: ${e}`);
    }
    setLoading(false);
  }

  async function handleGitPull() {
    addMessage('user', '/pull');
    setLoading(true);
    try {
      const result = await invoke('git_pull');
      addMessage('assistant', `**Pulled:** ${result || 'Already up to date'}`);
    } catch (e) {
      addMessage('system', `Pull error: ${e}`);
    }
    setLoading(false);
  }

  $effect(() => {
    if (chat.streamingContent) {
      scrollToBottom();
    }
  });
</script>

<div class="chat">
  <div class="messages" bind:this={chatContainer}>
    {#each chat.messages as msg (msg.id)}
      <div class="message {msg.role}">
        <div class="message-header">
          <span class="role">{msg.role}</span>
          {#if msg.decision}
            <span class="model-badge {msg.decision.tier.toLowerCase()}">{msg.decision.model_name}</span>
            <span class="complexity">complexity: {msg.decision.complexity.toFixed(2)}</span>
          {/if}
          {#if msg.guardian}
            <span class="quinn-chip {msg.guardian.trust >= 0.7 ? 'solid' : msg.guardian.trust >= 0.5 ? 'warn' : 'alert'}">
              Quinn {Math.round(msg.guardian.trust * 100)}%
            </span>
          {/if}
          <span class="timestamp">{new Date(msg.timestamp).toLocaleTimeString()}</span>
        </div>
        {#if msg.role === 'assistant'}
          <div class="message-content markdown">{@html renderMarkdown(msg.content)}</div>
        {:else}
          <div class="message-content">{msg.content}</div>
        {/if}
      </div>
    {/each}

    {#if chat.streamingContent}
      <div class="message assistant streaming">
        <div class="message-header">
          <span class="role">assistant</span>
          <span class="stream-indicator">streaming...</span>
        </div>
        <div class="message-content markdown">{@html renderMarkdown(chat.streamingContent)}</div>
      </div>
    {/if}

    {#if chat.loading && !chat.streamingContent}
      <div class="message assistant loading">
        <span class="dot"></span><span class="dot"></span><span class="dot"></span>
      </div>
    {/if}
  </div>

  {#if pendingActions.length > 0}
    <div class="action-panel">
      <div class="action-header">
        <span class="action-title">Proposed Actions ({pendingActions.length})</span>
        {#if pendingActions.some(a => a.status === 'pending')}
          <button class="action-apply-all" onclick={applyAll}>Apply All</button>
        {/if}
      </div>
      {#each pendingActions as action (action.id)}
        <div class="action-card {action.status}">
          <div class="action-info">
            <span class="action-kind">{action.kind}</span>
            <span class="action-desc">{action.description}</span>
            {#if action.details?.FileWrite}
              <span class="action-preview">{action.details.FileWrite.content.slice(0, 80)}...</span>
            {/if}
          </div>
          <div class="action-buttons">
            {#if action.status === 'pending'}
              <button class="btn-apply" onclick={() => applyAction(action)}>Apply</button>
              <button class="btn-skip" onclick={() => skipAction(action)}>Skip</button>
            {:else if action.status === 'applying'}
              <span class="action-status">...</span>
            {:else if action.status === 'applied'}
              <span class="action-status applied">Applied</span>
            {:else if action.status === 'skipped'}
              <span class="action-status skipped">Skipped</span>
            {:else if action.status === 'failed'}
              <span class="action-status failed">Failed</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <div class="input-area">
    {#if showSlashMenu && filteredCommands.length > 0}
      <div class="slash-menu">
        {#each filteredCommands as cmd}
          <button class="slash-item" onclick={() => selectSlashCommand(cmd)}>
            <span class="slash-cmd">{cmd.cmd}</span>
            <span class="slash-desc">{cmd.desc}</span>
          </button>
        {/each}
      </div>
    {/if}

    <textarea
      bind:value={input}
      onkeydown={handleKeydown}
      oninput={handleInput}
      placeholder="Ask anything... (/ for commands, Enter to send)"
      rows="3"
    ></textarea>
    <button onclick={send} disabled={chat.loading || !input.trim()}>Send</button>
  </div>
</div>

<style>
  .chat { display: flex; flex-direction: column; flex: 1; overflow: hidden; }
  .messages { flex: 1; overflow-y: auto; padding: 1rem; display: flex; flex-direction: column; gap: 1rem; }

  .message { padding: 0.75rem 1rem; border-radius: 8px; max-width: 85%; }
  .message.user { background: #1e293b; align-self: flex-end; border: 1px solid #334155; }
  .message.assistant { background: #0f172a; align-self: flex-start; border: 1px solid #1e293b; max-width: 90%; }
  .message.system { background: #1a0a0a; align-self: center; border: 1px solid #7f1d1d; color: #fca5a5; font-size: 0.85rem; }
  .message.streaming { border-color: #1e40af; }

  .message-header { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.5rem; font-size: 0.75rem; }
  .role { color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; }
  .timestamp { color: #334155; margin-left: auto; font-size: 0.65rem; }
  .stream-indicator { color: #60a5fa; animation: pulse 1s ease-in-out infinite; }

  .model-badge { padding: 0.1rem 0.4rem; border-radius: 4px; font-size: 0.7rem; }
  .model-badge.local { background: #064e3b; color: #6ee7b7; }
  .model-badge.cloud { background: #1e1b4b; color: #a5b4fc; }
  .complexity { color: #475569; }

  .quinn-chip { padding: 0.1rem 0.4rem; border-radius: 4px; font-size: 0.65rem; font-weight: 600; letter-spacing: 0.02em; }
  .quinn-chip.solid { background: #052e16; color: #4ade80; border: 1px solid #166534; }
  .quinn-chip.warn  { background: #422006; color: #fb923c; border: 1px solid #92400e; }
  .quinn-chip.alert { background: #450a0a; color: #f87171; border: 1px solid #7f1d1d; }

  .message-content { line-height: 1.6; }
  .message-content:not(.markdown) { white-space: pre-wrap; }

  /* Markdown styles */
  .message-content.markdown :global(p) { margin: 0.3rem 0; }
  .message-content.markdown :global(h1),
  .message-content.markdown :global(h2),
  .message-content.markdown :global(h3) {
    margin: 0.6rem 0 0.3rem 0;
    color: #e2e8f0;
  }
  .message-content.markdown :global(ul),
  .message-content.markdown :global(ol) {
    margin: 0.3rem 0;
    padding-left: 1.5rem;
  }
  .message-content.markdown :global(a) { color: #60a5fa; }
  .message-content.markdown :global(strong) { color: #f1f5f9; }
  .message-content.markdown :global(code:not(.hljs)) {
    background: #1e293b;
    padding: 0.15rem 0.35rem;
    border-radius: 3px;
    font-size: 0.85em;
    color: #6ee7b7;
  }
  .message-content.markdown :global(blockquote) {
    border-left: 3px solid #334155;
    margin: 0.5rem 0;
    padding: 0.3rem 0.8rem;
    color: #94a3b8;
  }

  /* Code blocks */
  .message-content.markdown :global(.code-block) {
    margin: 0.5rem 0;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid #1e293b;
  }
  .message-content.markdown :global(.code-header) {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.3rem 0.6rem;
    background: #1e293b;
    font-size: 0.7rem;
  }
  .message-content.markdown :global(.code-lang) {
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .message-content.markdown :global(.copy-btn) {
    background: none;
    border: 1px solid #334155;
    color: #64748b;
    padding: 0.15rem 0.5rem;
    border-radius: 3px;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.65rem;
  }
  .message-content.markdown :global(.copy-btn:hover) {
    color: #60a5fa;
    border-color: #60a5fa;
  }
  .message-content.markdown :global(pre) {
    margin: 0;
    padding: 0.75rem;
    background: #0a0e17;
    overflow-x: auto;
    font-size: 0.8rem;
    line-height: 1.5;
  }
  .message-content.markdown :global(code.hljs) {
    background: transparent;
    padding: 0;
  }

  /* highlight.js overrides */
  .message-content.markdown :global(.hljs-keyword) { color: #c678dd; }
  .message-content.markdown :global(.hljs-string) { color: #98c379; }
  .message-content.markdown :global(.hljs-number) { color: #d19a66; }
  .message-content.markdown :global(.hljs-function) { color: #61afef; }
  .message-content.markdown :global(.hljs-title) { color: #61afef; }
  .message-content.markdown :global(.hljs-params) { color: #e2e8f0; }
  .message-content.markdown :global(.hljs-comment) { color: #5c6370; font-style: italic; }
  .message-content.markdown :global(.hljs-built_in) { color: #e6c07b; }
  .message-content.markdown :global(.hljs-type) { color: #e6c07b; }
  .message-content.markdown :global(.hljs-attr) { color: #d19a66; }
  .message-content.markdown :global(.hljs-variable) { color: #e06c75; }
  .message-content.markdown :global(.hljs-literal) { color: #56b6c2; }

  .loading { display: flex; gap: 0.3rem; padding: 1rem; }
  .dot { width: 6px; height: 6px; background: #60a5fa; border-radius: 50%; animation: pulse 1.4s ease-in-out infinite; }
  .dot:nth-child(2) { animation-delay: 0.2s; }
  .dot:nth-child(3) { animation-delay: 0.4s; }
  @keyframes pulse {
    0%, 80%, 100% { opacity: 0.3; transform: scale(0.8); }
    40% { opacity: 1; transform: scale(1); }
  }

  .input-area {
    position: relative;
    padding: 1rem;
    border-top: 1px solid #1e293b;
    display: flex;
    gap: 0.5rem;
  }

  .slash-menu {
    position: absolute;
    bottom: 100%;
    left: 1rem;
    right: 5rem;
    background: #1e293b;
    border: 1px solid #334155;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 -4px 12px rgba(0, 0, 0, 0.3);
  }

  .slash-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.5rem 0.75rem;
    background: none;
    border: none;
    border-bottom: 1px solid #334155;
    color: #e2e8f0;
    font-family: inherit;
    font-size: 0.8rem;
    cursor: pointer;
    text-align: left;
  }

  .slash-item:last-child { border-bottom: none; }
  .slash-item:hover { background: #334155; }

  .slash-cmd { color: #60a5fa; font-weight: 500; min-width: 5rem; }
  .slash-desc { color: #64748b; font-size: 0.75rem; }

  textarea {
    flex: 1;
    background: #0f172a;
    border: 1px solid #334155;
    color: #e2e8f0;
    border-radius: 8px;
    padding: 0.75rem;
    font-family: inherit;
    font-size: 0.9rem;
    resize: none;
    outline: none;
  }
  textarea:focus { border-color: #60a5fa; }

  button:not(.slash-item):not(.copy-btn) {
    padding: 0.75rem 1.5rem;
    background: #1e40af;
    border: none;
    color: white;
    border-radius: 8px;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.2s;
  }
  button:not(.slash-item):not(.copy-btn):hover:not(:disabled) { background: #2563eb; }
  button:not(.slash-item):not(.copy-btn):not(.btn-apply):not(.btn-skip):not(.action-apply-all):disabled { opacity: 0.4; cursor: not-allowed; }

  /* Action cards */
  .action-panel {
    border-top: 1px solid #1e3a5f;
    background: #0c1220;
    padding: 0.5rem 1rem;
  }
  .action-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }
  .action-title {
    color: #60a5fa;
    font-size: 0.8rem;
    font-weight: 600;
  }
  .action-apply-all {
    padding: 0.25rem 0.75rem;
    background: #065f46;
    border: 1px solid #059669;
    color: #6ee7b7;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .action-apply-all:hover { background: #047857; }
  .action-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    margin-bottom: 0.25rem;
    background: #111827;
    border: 1px solid #1e293b;
    border-radius: 6px;
  }
  .action-card.applied { border-color: #065f46; opacity: 0.7; }
  .action-card.skipped { border-color: #374151; opacity: 0.5; }
  .action-card.failed { border-color: #7f1d1d; }
  .action-info { display: flex; flex-direction: column; gap: 0.15rem; flex: 1; }
  .action-kind {
    font-size: 0.6rem;
    color: #a78bfa;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .action-desc { font-size: 0.8rem; color: #e2e8f0; }
  .action-preview {
    font-size: 0.7rem;
    color: #475569;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 400px;
  }
  .action-buttons { display: flex; gap: 0.3rem; flex-shrink: 0; }
  .btn-apply {
    padding: 0.25rem 0.6rem;
    background: #065f46;
    border: 1px solid #059669;
    color: #6ee7b7;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .btn-apply:hover { background: #047857; }
  .btn-skip {
    padding: 0.25rem 0.6rem;
    background: #1f2937;
    border: 1px solid #374151;
    color: #9ca3af;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }
  .btn-skip:hover { background: #374151; }
  .action-status { font-size: 0.7rem; }
  .action-status.applied { color: #6ee7b7; }
  .action-status.skipped { color: #6b7280; }
  .action-status.failed { color: #f87171; }
</style>
