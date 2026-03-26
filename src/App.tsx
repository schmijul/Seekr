import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import type { ReindexStatus, SearchResult } from "./types";
import "./App.css";

function formatModified(unixTs: number) {
  const dt = new Date(unixTs * 1000);
  if (Number.isNaN(dt.getTime())) {
    return "unknown";
  }
  return dt.toLocaleString();
}

function App() {
  const [query, setQuery] = useState("");
  const [folders, setFolders] = useState<string[]>([]);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [status, setStatus] = useState<string>("Ready.");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const run = async () => {
      try {
        await invoke("init_backend");
        const saved = await invoke<string[]>("get_index_roots");
        setFolders(saved);
      } catch (err) {
        setStatus(`Init error: ${String(err)}`);
      }
    };

    void run();
  }, []);

  const canSearch = useMemo(() => query.trim().length > 0, [query]);

  const onPickFolders = async () => {
    try {
      const picked = await open({
        directory: true,
        multiple: true,
        title: "Select folders to index",
      });

      if (!picked) {
        return;
      }

      const next = Array.isArray(picked) ? picked : [picked];
      const saved = await invoke<string[]>("set_index_roots", { roots: next });
      setFolders(saved);
      setStatus(`Saved ${saved.length} index folder(s).`);
    } catch (err) {
      setStatus(`Folder selection failed: ${String(err)}`);
    }
  };

  const onReindex = async () => {
    setBusy(true);
    try {
      const result = await invoke<ReindexStatus>("run_full_reindex");
      setStatus(
        `Reindex done. Indexed: ${result.indexed}, Removed: ${result.removed}, Failed: ${result.failed}.`,
      );
    } catch (err) {
      setStatus(`Reindex failed: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const onSearch = async () => {
    if (!canSearch) {
      setResults([]);
      return;
    }

    try {
      const found = await invoke<SearchResult[]>("search_index", {
        query,
        limit: 100,
      });
      setResults(found);
      setStatus(`Found ${found.length} result(s).`);
    } catch (err) {
      setStatus(`Search failed: ${String(err)}`);
    }
  };

  return (
    <main className="app-shell">
      <header className="app-header">
        <h1>Seekr</h1>
        <p>Local-first desktop search. Offline. Fast. Yours.</p>
      </header>

      <section className="controls">
        <div className="folder-row">
          <button type="button" onClick={onPickFolders} disabled={busy}>
            Select folders
          </button>
          <button type="button" onClick={onReindex} disabled={busy || folders.length === 0}>
            Reindex now
          </button>
        </div>

        <ul className="folder-list">
          {folders.length === 0 ? <li>No folders selected yet.</li> : null}
          {folders.map((folder) => (
            <li key={folder}>{folder}</li>
          ))}
        </ul>

        <div className="search-row">
          <input
            className="search-input"
            placeholder="Search files, text, and PDFs..."
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void onSearch();
              }
            }}
          />
          <button type="button" onClick={onSearch} disabled={busy || !canSearch}>
            Search
          </button>
        </div>

        <p className="hint">{status}</p>
      </section>

      <section className="results">
        {results.length === 0 ? (
          <p className="empty">No results yet.</p>
        ) : (
          results.map((result) => (
            <article key={result.path} className="result-item">
              <div className="result-top">
                <h2>{result.title}</h2>
                <span>{result.fileType}</span>
              </div>
              <p className="path">{result.path}</p>
              <p className="snippet" dangerouslySetInnerHTML={{ __html: result.snippet }} />
              <p className="meta">Modified: {formatModified(result.modifiedTs)}</p>
            </article>
          ))
        )}
      </section>
    </main>
  );
}

export default App;
