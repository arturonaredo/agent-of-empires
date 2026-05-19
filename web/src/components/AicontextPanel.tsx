import { useEffect, useState, useCallback } from "react";

interface AicontextPanelProps {
  projectPath: string;
  onClose: () => void;
}

export function AicontextPanel({ projectPath, onClose }: AicontextPanelProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function launch() {
      try {
        const origin = window.location.origin;
        const res = await fetch("/api/aicontext/launch", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ path: projectPath, origin }),
        });
        if (!res.ok) {
          const data = await res.json().catch(() => ({}));
          if (!cancelled) setError(data.error || "Failed to launch aicontext console");
          return;
        }
        const data = await res.json();
        if (!cancelled) setUrl(data.url);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    launch();
    return () => { cancelled = true; };
  }, [projectPath]);

  const handleClose = useCallback(async () => {
    // Stop the subprocess when closing
    await fetch("/api/aicontext/stop", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: projectPath }),
    }).catch(() => {});
    onClose();
  }, [projectPath, onClose]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [handleClose]);

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
      onClick={handleClose}
    >
      <div
        className="bg-surface-800 border border-surface-700/50 rounded-lg w-[90vw] h-[85vh] max-w-[1200px] shadow-2xl animate-slide-up flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-5 py-3 border-b border-surface-700 flex items-center justify-between shrink-0">
          <h2 className="text-sm font-medium text-text-primary">AI Context Console</h2>
          <button
            onClick={handleClose}
            className="w-7 h-7 flex items-center justify-center rounded-md text-text-muted hover:text-text-secondary hover:bg-surface-700/50 cursor-pointer"
            aria-label="Close"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 min-h-0">
          {loading && (
            <div className="flex items-center justify-center h-full text-text-muted text-sm">
              Launching aicontext console...
            </div>
          )}
          {error && (
            <div className="flex items-center justify-center h-full text-red-400 text-sm px-4 text-center">
              {error}
            </div>
          )}
          {url && (
            <iframe
              src={url}
              className="w-full h-full border-0 rounded-b-lg"
              title="AI Context Console"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            />
          )}
        </div>
      </div>
    </div>
  );
}
