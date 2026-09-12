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
