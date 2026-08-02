<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  /**
   * @typedef {{
   *   id: number,
   *   traditional: string,
   *   simplified: string,
   *   pinyin: string,
   *   definition: string
   * }} Word
   */

  /** @type {HTMLElement | null} */
  let header = null;
  let sticky = 0;
  let headerSticky = false;
  let darkMode = false;
  let search = "";
  /** @type {Word[]} */
  let result = [];
  let error = "";
  let loading = false;
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timeout;
  let listenToClipboard = false;
  let isTauri = false;
  let requestVersion = 0;
  /** @type {(() => void) | null} */
  let unlistenSearch = null;

  $: {
    const normalizedSearch = search.trim();
    if (normalizedSearch.length > 0) {
      scheduleSearch(normalizedSearch);
    } else {
      clearTimeout(timeout);
      result = [];
      error = "";
      loading = false;
    }
  }

  /** @param {string} query */
  function scheduleSearch(query) {
    clearTimeout(timeout);
    timeout = setTimeout(() => void fetchResults(query), 200);
  }

  /** @param {string} query */
  async function fetchResults(query) {
    if (!isTauri) {
      result = [];
      error = "Dictionary search is available in the desktop app.";
      return;
    }

    const version = ++requestVersion;
    loading = true;
    error = "";

    try {
      const data = await invoke("dictionary_search", { query });
      if (version !== requestVersion || query !== search.trim()) return;
      result = Array.isArray(data) ? data : [];
    } catch (e) {
      if (version !== requestVersion) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (version === requestVersion) loading = false;
    }
  }

  /** @param {boolean} enabled */
  async function setClipboardMonitoring(enabled) {
    if (!isTauri) {
      listenToClipboard = false;
      error = "Clipboard monitoring is available in the desktop app.";
      return;
    }

    try {
      await invoke(enabled ? "start_clipboard" : "stop_clipboard");
      listenToClipboard = enabled;
      error = "";
    } catch (e) {
      listenToClipboard = false;
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function toggleTheme() {
    darkMode = !darkMode;
    document.body.classList.toggle("dark-mode", darkMode);
    try {
      localStorage.setItem("ch-en-dict-theme", darkMode ? "dark" : "light");
    } catch (_) {}
  }

  function toggleListenToClipboard() {
    void setClipboardMonitoring(!listenToClipboard);
  }

  onMount(() => {
    let disposed = false;
    isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

    try {
      darkMode = localStorage.getItem("ch-en-dict-theme") === "dark";
      document.body.classList.toggle("dark-mode", darkMode);
    } catch (_) {}

    if (header) sticky = header.offsetTop;
    window.addEventListener("scroll", stickyFunction);

    if (isTauri) {
      void listen("search", (event) => {
        if (typeof event.payload === "string") search = event.payload;
      })
        .then((unlisten) => {
          if (disposed) {
            unlisten();
          } else {
            unlistenSearch = unlisten;
          }
        })
        .catch((e) => {
          if (!disposed) error = e instanceof Error ? e.message : String(e);
        });
    }

    return () => {
      disposed = true;
      clearTimeout(timeout);
      window.removeEventListener("scroll", stickyFunction);
      unlistenSearch?.();
      if (isTauri && listenToClipboard) {
        void invoke("stop_clipboard").catch(() => {});
      }
    };
  });

  function stickyFunction() {
    if (!header) return;
    headerSticky = window.pageYOffset > sticky;
  }
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link
    href="https://fonts.googleapis.com/css2?family=Figtree:wght@400;500;600&display=swap"
    rel="stylesheet"
  />
</svelte:head>

<header class="header" class:sticky={headerSticky} bind:this={header}>
  <div class="header-inner">
    <h1 class="logo">词典</h1>
    <div class="search-wrap">
      <input
        type="text"
        bind:value={search}
        placeholder="Search Chinese or English…"
        class="search-input"
        aria-label="Search dictionary"
      />
    </div>
    <button
      type="button"
      class="theme-btn"
      onclick={toggleTheme}
      aria-label={darkMode ? "Switch to light mode" : "Switch to dark mode"}
      title={darkMode ? "Light mode" : "Dark mode"}
    >
      {#if darkMode}
        ☀
      {:else}
        ☽
      {/if}
    </button>
  </div>
</header>

<main class="main">
  {#if error}
    <div class="message message-error" role="alert">
      <span class="message-icon" aria-hidden="true">!</span>
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="loading" aria-live="polite">
      <span class="loading-dots">
        <span></span><span></span><span></span>
      </span>
      <span class="loading-text">Searching…</span>
    </div>
  {/if}

  {#if !loading && search.trim().length > 0 && result.length === 0 && !error}
    <p class="message message-empty">No results for “{search}”</p>
  {/if}

  {#if !loading && result.length > 0}
    <ul class="result-list">
      {#each result as word (word.traditional + (word.pinyin || '') + (word.definition || ''))}
        <li class="result-card">
          <div class="result-head">
            <span class="result-char">{word.traditional}</span>
            {#if word.simplified && word.simplified !== word.traditional}
              <span class="result-char-alt">({word.simplified})</span>
            {/if}
          </div>
          {#if word.pinyin}
            <p class="result-pinyin">{word.pinyin}</p>
          {/if}
          <p class="result-definition">{word.definition}</p>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="tooltip">
    <button
      id="listen-to-clipboard-btn"
      class="listen-to-clipboard-btn"
      class:on={listenToClipboard}
      class:off={!listenToClipboard}
      onclick={toggleListenToClipboard}
      aria-label={listenToClipboard ? "Stop clipboard monitoring" : "Start clipboard monitoring"}
      aria-pressed={listenToClipboard}
      title={listenToClipboard ? "Stop clipboard monitoring" : "Start clipboard monitoring"}
    >📋</button>
    <span class="tooltip-text">Clipboard monitoring</span>
  </div>
</main>

<style>
  :global(body) {
    --bg: #f6f4f0;
    --bg-card: #fff;
    --text: #1c1917;
    --text-muted: #57534e;
    --accent: #0d9488;
    --accent-hover: #0f766e;
    --header-bg: #134e4a;
    --header-bg-sticky: #0f403d;
    --header-text: #f0fdfa;
    --border: #e7e5e4;
    --error-bg: #fef2f2;
    --error-text: #b91c1c;
    --radius: 12px;
    --radius-sm: 8px;
    --shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
    --shadow-sticky: 0 4px 12px rgba(0, 0, 0, 0.08);
    font-family: "Figtree", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
      sans-serif;
    margin: 0;
    background: var(--bg);
    color: var(--text);
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
  }

  :global(body.dark-mode) {
    --bg: #1c1917;
    --bg-card: #292524;
    --text: #fafaf9;
    --text-muted: #a8a29e;
    --accent: #2dd4bf;
    --accent-hover: #5eead4;
    --header-bg: #0f766e;
    --header-bg-sticky: #0d5c55;
    --header-text: #f0fdfa;
    --border: #44403c;
    --error-bg: #450a0a;
    --error-text: #fca5a5;
    --shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
    --shadow-sticky: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .header {
    position: relative;
    padding: 1rem 1.5rem;
    padding-right: 4rem;
    background: var(--header-bg);
    color: var(--header-text);
    transition: box-shadow 0.2s ease, background 0.2s ease;
  }

  .header.sticky {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 100;
    background: var(--header-bg-sticky);
    box-shadow: var(--shadow-sticky);
  }

  .header-inner {
    padding-right: 4rem;
    max-width: 42rem;
    margin: 0 auto;
    display: flex;
    align-items: center;
    gap: 1.25rem;
  }

  .logo {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    flex-shrink: 0;
  }

  .search-wrap {
    flex: 1;
    min-width: 0;
  }

  .theme-btn {
    position: absolute;
    top: 50%;
    right: 2rem;
    transform: translateY(-50%);
    width: 2.25rem;
    height: 2.25rem;
    padding: 0;
    font-size: 1.125rem;
    line-height: 1;
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: var(--radius-sm);
    color: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .theme-btn:hover {
    background: rgba(255, 255, 255, 0.25);
    border-color: rgba(255, 255, 255, 0.4);
  }

  .search-input {
    width: 100%;
    padding: 0.625rem 1rem;
    font-size: 1rem;
    font-family: inherit;
    color: var(--text);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  .search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(13, 148, 136, 0.2);
  }

  .main {
    max-width: 42rem;
    margin: 0 auto;
    padding: 1.5rem 1.5rem 3rem;
  }

  .message {
    padding: 0.75rem 1rem;
    border-radius: var(--radius-sm);
    font-size: 0.9375rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .message-error {
    background: var(--error-bg);
    color: var(--error-text);
  }

  .message-icon {
    font-weight: 600;
    flex-shrink: 0;
  }

  .message-empty {
    color: var(--text-muted);
    margin: 0;
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    color: var(--text-muted);
    font-size: 0.9375rem;
  }

  .loading-dots {
    display: inline-flex;
    gap: 4px;
  }

  .loading-dots span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: bounce 0.6s ease-in-out infinite;
  }

  .loading-dots span:nth-child(2) {
    animation-delay: 0.1s;
  }

  .loading-dots span:nth-child(3) {
    animation-delay: 0.2s;
  }

  @keyframes bounce {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-4px);
    }
  }

  .result-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .result-card {
    padding: 1rem 1.25rem;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .result-card:hover {
    border-color: var(--accent);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-mode) .result-card:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
  }

  .result-head {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
  }

  .result-char {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--text);
    line-height: 1.3;
  }

  .result-char-alt {
    font-size: 1rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .result-pinyin {
    margin: 0 0 0.375rem 0;
    font-size: 0.9375rem;
    color: var(--text-muted);
    font-style: italic;
  }

  .result-definition {
    margin: 0;
    font-size: 1rem;
    color: var(--text);
    line-height: 1.55;
  }

  .listen-to-clipboard-btn {
    position: fixed;
    bottom: 20px;
    right: 20px;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    border: none;
    font-size: 20px;
  }

  .listen-to-clipboard-btn.on:hover {
    box-shadow: 0 0 10px var(--accent);
  }

  .tooltip {
    position: fixed;
    bottom: 20px;
    right: 20px;
  }

  .tooltip-text {
    visibility: hidden;
    background: black;
    color: white;
    padding: 6px 10px;
    border-radius: 5px;
    position: absolute;
    right: 70px;
    bottom: 15px;
    white-space: nowrap;
  }

  .tooltip:hover .tooltip-text {
    visibility: visible;
  }

  .off {
    background: #6b7280;
  }

  .on {
    background: #22c55e;
  }
</style>
