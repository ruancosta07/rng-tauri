import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {openUrl} from "@tauri-apps/plugin-opener"
import { UpdateDialog } from "./components/UpdateDialog"


type AppState = "ready";

interface FileStatus {
  filename: string;
  folder: string;
  outputDir: string;
  folderTotal: number;
  status: "processing" | "done" | "error";
  code?: string;
  error?: string;
}

interface ProgressPayload {
  current: number;
  total: number;
  filename: string;
  folder: string;
  outputDir: string;
  folderTotal: number;
  status: string;
  code?: string;
  error?: string;
}

interface HistoryEntry {
  id: string;
  timestamp: string;
  folderName: string;
  outputDir: string;
  total: number;
  done: number;
  errors: number;
}

const HISTORY_KEY = "rng-history";


export default function App() {
  const [appState] = useState<AppState>("ready");

  const [files, setFiles] = useState<FileStatus[]>([]);
  const [isDragging, setIsDragging] = useState(false);
  const [done, setDone] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  const [diagLog, setDiagLog] = useState<string[]>([]);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const processingRef = useRef(false);
  const handleFoldersRef = useRef<((paths: string[]) => void) | undefined>(undefined);

  // ── load history on mount ───────────────────────────────────────────────
  useEffect(() => {
    const stored = localStorage.getItem(HISTORY_KEY);
    if (stored) setHistory(JSON.parse(stored));
  }, []);

  // ── processing ──────────────────────────────────────────────────────────
  const handleFolders = useCallback(async (paths: string[]) => {
    if (processingRef.current || paths.length === 0) return;

    processingRef.current = true;
    setDone(false);
    setFiles([]);
    setStatusMsg("Escaneando pastas...");

    let unlisten: (() => void) | undefined;
    let unlistenDone: (() => void) | undefined;

    const cleanup = () => {
      unlisten?.();
      unlistenDone?.();
    };

    try {
      unlisten = await listen<ProgressPayload>("progress", (event) => {
        const p = event.payload;

        if (p.status === "fatal") {
          setStatusMsg(`Erro: ${p.error}`);
          processingRef.current = false;
          cleanup();
          return;
        }

        setFiles((prev) => {
          if (!p.filename) return prev;
          const entry: FileStatus = {
            filename: p.filename,
            folder: p.folder ?? "",
            outputDir: p.outputDir ?? "",
            folderTotal: p.folderTotal ?? 0,
            status: p.status as FileStatus["status"],
            code: p.code ?? undefined,
            error: p.error ?? undefined,
          };
          const idx = prev.findIndex((f) => f.filename === p.filename);
          if (idx >= 0) {
            const next = [...prev];
            next[idx] = entry;
            return next;
          }
          return [...prev, entry];
        });
      });

      unlistenDone = await listen("process-done", () => {
        processingRef.current = false;
        setDone(true);
        setStatusMsg(null);
        cleanup();
      });

      const total = await invoke<number>("process_folders", { folders: paths });

      if (total === 0) {
        setStatusMsg("Nenhum PDF encontrado nas pastas selecionadas.");
        processingRef.current = false;
        cleanup();
        return;
      }

      setStatusMsg(null);
    } catch (e) {
      setStatusMsg(`Erro: ${String(e)}`);
      processingRef.current = false;
      cleanup();
    }
  }, []);

  useEffect(() => {
    handleFoldersRef.current = handleFolders;
  });

  // ── drag & drop ─────────────────────────────────────────────────────────
  useEffect(() => {
    if (appState !== "ready") return;
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    win
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") setIsDragging(true);
        else if (event.payload.type === "leave") setIsDragging(false);
        else if (event.payload.type === "drop") {
          setIsDragging(false);
          handleFoldersRef.current?.(event.payload.paths);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [appState]);

  const pickFolders = async () => {
    try {
      const selected = await open({ directory: true, multiple: true });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      handleFolders(paths);
    } catch (e) {
      setStatusMsg(`Erro ao abrir dialog: ${String(e)}`);
    }
  };

  const folderGroups = useMemo(() => {
    const map = new Map<
      string,
      { total: number; done: number; errors: number; active: boolean; outputDir: string }
    >();
    for (const f of files) {
      const key = f.folder || "—";
      if (!map.has(key))
        map.set(key, { total: f.folderTotal, done: 0, errors: 0, active: false, outputDir: f.outputDir ?? "" });
      const g = map.get(key)!;
      if (f.outputDir) g.outputDir = f.outputDir;
      if (f.folderTotal > g.total) g.total = f.folderTotal;
      if (f.status === "done") g.done++;
      else if (f.status === "error") {
        g.done++;
        g.errors++;
      } else if (f.status === "processing") g.active = true;
    }
    return Array.from(map.entries()).map(([name, s]) => ({ name, ...s }));
  }, [files]);

  useEffect(() => {
    if (!done || folderGroups.length === 0) return;
    const existing: HistoryEntry[] = JSON.parse(
      localStorage.getItem(HISTORY_KEY) ?? "[]"
    );
    const newEntries: HistoryEntry[] = folderGroups.map((fg) => ({
      id: `${Date.now()}-${fg.name}`,
      timestamp: new Date().toISOString(),
      folderName: fg.name,
      outputDir: fg.outputDir,
      total: fg.total,
      done: fg.done,
      errors: fg.errors,
    }));
    const merged = [...newEntries, ...existing];
    localStorage.setItem(HISTORY_KEY, JSON.stringify(merged));
    setHistory(merged);
  }, [done]);
  

  const openLinkHandler = async()=> await openUrl("https://www.ruancostadev.com.br/")
  // ── render ──────────────────────────────────────────────────────────────
  return (
    <div className="min-h-screen bg-black text-white  flex flex-col items-center justify-center p-6 gap-5 select-none ">
      <UpdateDialog />
      <div className="h-px bg-white/10" />

      {appState === "ready" && (
        <>
          {/* drag overlay */}
          {isDragging && (
            <div className="min-h-[70vh] w-[90%] border-2 border-dashed rounded-xl border-white/30 flex flex-col justify-center items-center bg-white/10 fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2">
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" className="size-10 fill-white">
                <path stroke="none" d="M0 0h24v24H0z" fill="none" />
                <path d="M5 4h4l3 3h7a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2" />
              </svg>
              <h1>Soltar pasta</h1>
            </div>
          )}

          {/* header — only when idle and no history */}
          { !isDragging && (
            <div className="flex flex-col items-center cursor-pointer" onClick={pickFolders}>
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" className="fill-white size-12 mb-2">
                <path stroke="none" d="M0 0h24v24H0z" fill="none" />
                <path d="M5 4h4l3 3h7a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2" />
              </svg>
              <h1 className="text-xl font-semibold">Renomeador de guias</h1>
            </div>
          )}

          {statusMsg && (
            <div className="border border-white/20 rounded-md px-3 py-2 text-xs text-white/60">
              {statusMsg}
            </div>
          )}

          {diagLog.length > 0 && (
            <div className="flex flex-col gap-1 border border-white/10 rounded-md p-3 max-h-48 overflow-y-auto w-full">
              <div className="flex justify-between items-center mb-1">
                <span className="text-xs text-white/30 tracking-widest uppercase">Diagnóstico</span>
                <button onClick={() => setDiagLog([])} className="text-xs text-white/20 hover:text-white/50">✕</button>
              </div>
              {diagLog.map((line, i) => (
                <p key={i} className="text-xs text-white/50 font-mono leading-relaxed">{line}</p>
              ))}
            </div>
          )}

          {/* active folder groups */}
          {folderGroups.some((fg) => fg.active || fg.done < fg.total) && (
            <div className="flex flex-col w-full gap-2">
              {folderGroups.filter((fg) => fg.active || fg.done < fg.total).map((fg) => {
                const folderPct = fg.total > 0 ? Math.round((fg.done / fg.total) * 100) : 0;
                const allDone = fg.done === fg.total && fg.total > 0 && !fg.active;
                const hasErrors = fg.errors > 0;
                const destName = fg.name !== "—" ? `${fg.name} (renomeado)` : fg.outputDir;
                return (
                  <div key={fg.name} className="flex flex-col gap-2 border border-white/10 rounded-md p-4">
                    <div className="flex items-center gap-2">
                      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" className="fill-white size-10 shrink-0">
                        <path stroke="none" d="M0 0h24v24H0z" fill="none" />
                        <path d="M5 4h4l3 3h7a2 2 0 0 1 2 2v8a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-11a2 2 0 0 1 2 -2" />
                      </svg>
                      <div className="flex flex-col w-full gap-1.5">
                        <span className="text-white truncate text-base tracking-wide font-semibold">{destName}</span>
                        {folderPct < 100 && (
                          <div className="h-1 bg-white/10 rounded-full flex gap-2 items-center">
                            <div className="h-full bg-white/70 rounded-full transition-all duration-300 ease-out" style={{ width: `${folderPct}%` }} />
                            <div className="flex items-center gap-2.5 shrink-0">
                              {hasErrors && <span className="text-white/25 text-xs">{fg.errors} erro{fg.errors > 1 ? "s" : ""}</span>}
                              <span className="text-xs tabular-nums tracking-widest text-white! leading-none font-semibold">{folderPct}%</span>
                            </div>
                          </div>
                        )}
                        {allDone && fg.outputDir && (
                          <button onClick={() => invoke("open_folder", { path: fg.outputDir })} className="self-start mt-0.5 text-xs text-white/40 hover:text-white/80 transition-colors tracking-wide underline underline-offset-2">
                            Abrir pasta de destino
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {/* history — always visible when there are entries */}
          {history.length > 0 && !isDragging && (
            <div className="w-full flex flex-col gap-2">
              {folderGroups.some((fg) => fg.active || fg.done < fg.total) && <div className="h-px bg-white/10" />}
              <div className="flex items-center justify-between">
                <span className="text-xs text-white tracking-widest uppercase font-semibold">Histórico</span>
                <button
                  onClick={() => { localStorage.removeItem(HISTORY_KEY); setHistory([]); }}
                  className="text-xs text-red-500 cursor-pointer"
                >
                  Limpar
                </button>
              </div>
              <div className="flex flex-col gap-1.5 overflow-y-auto max-h-64">
                {history.map((entry) => (
                  <div key={entry.id} className="flex items-center justify-between border border-white/10 rounded-md px-3 py-2.5 gap-3">
                    <div className="flex flex-col gap-0.5 min-w-0">
                      <span className="text-sm text-white/80 truncate font-medium">
                        {entry.folderName} (renomeado)
                      </span>
                      <span className="text-xs text-white/25">
                        {new Date(entry.timestamp).toLocaleString("pt-BR")}
                        {" · "}
                        {entry.done - entry.errors}/{entry.total} ok
                        {entry.errors > 0 && ` · ${entry.errors} erro${entry.errors > 1 ? "s" : ""}`}
                      </span>
                    </div>
                    {entry.outputDir && (
                      <button onClick={() => invoke("open_folder", { path: entry.outputDir })} className="text-xs text-white/50 hover:text-white/70 transition-colors shrink-0 cursor-pointer">
                        Abrir na pasta
                      </button>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
      <div className="absolute left-2/4 -translate-x-1/2 bottom-8">
        <span className="text-zinc-600 ">Desenvolvido por <button onClick={openLinkHandler} className="text-white cursor-pointer">Ruan Costa</button></span>
      </div>
    </div>
  );
}
