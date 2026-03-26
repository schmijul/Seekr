import { useMemo, useState } from "react";
import "./App.css";

type SearchResult = {
  id: string;
  title: string;
  path: string;
  snippet: string;
  fileType: string;
  modifiedAt: string;
};

const demoResults: SearchResult[] = [
  {
    id: "1",
    title: "project-notes.md",
    path: "/home/user/notes/project-notes.md",
    snippet: "Today we define the scope for Seekr and prioritize a local-first index.",
    fileType: "md",
    modifiedAt: "2026-03-26 16:00",
  },
  {
    id: "2",
    title: "todo.txt",
    path: "/home/user/notes/todo.txt",
    snippet: "Implement fast incremental indexing and preview highlighting.",
    fileType: "txt",
    modifiedAt: "2026-03-26 15:42",
  },
];

function App() {
  const [query, setQuery] = useState("");
  const [folders, setFolders] = useState<string[]>([]);

  const shownResults = useMemo(() => {
    if (!query.trim()) {
      return demoResults;
    }

    const q = query.toLowerCase();
    return demoResults.filter(
      (r) =>
        r.title.toLowerCase().includes(q) ||
        r.path.toLowerCase().includes(q) ||
        r.snippet.toLowerCase().includes(q),
    );
  }, [query]);

  return (
    <main className="app-shell">
      <header className="app-header">
        <h1>Seekr</h1>
        <p>Local-first desktop search. Offline. Fast. Yours.</p>
      </header>

      <section className="controls">
        <div className="folder-row">
          <button
            type="button"
            onClick={() =>
              setFolders((prev) =>
                prev.length === 0
                  ? ["/home/user/Documents", "/home/user/Notes"]
                  : prev,
              )
            }
          >
            Select folders
          </button>
          <span className="hint">Folder picker wiring comes in next milestones.</span>
        </div>

        <ul className="folder-list">
          {folders.length === 0 ? <li>No folders selected yet.</li> : null}
          {folders.map((folder) => (
            <li key={folder}>{folder}</li>
          ))}
        </ul>

        <input
          className="search-input"
          placeholder="Search files, text, and PDFs..."
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
        />
      </section>

      <section className="results">
        {shownResults.length === 0 ? (
          <p className="empty">No matching results.</p>
        ) : (
          shownResults.map((result) => (
            <article key={result.id} className="result-item">
              <div className="result-top">
                <h2>{result.title}</h2>
                <span>{result.fileType}</span>
              </div>
              <p className="path">{result.path}</p>
              <p className="snippet">{result.snippet}</p>
              <p className="meta">Modified: {result.modifiedAt}</p>
            </article>
          ))
        )}
      </section>
    </main>
  );
}

export default App;
