// Recent projects for the project panel: a small localStorage list of metadata
// plus an IndexedDB store of folder handles, so an opened folder re-opens
// without re-picking it (subject to a permission re-grant). Samples re-open
// directly by id. Everything is best-effort and guarded for SSR / private mode.

// The File System Access permission API (queryPermission/requestPermission) is
// shipped by browsers but absent from the base DOM lib; declare just what we use.
type FileSystemPermissionMode = "read" | "readwrite";

interface FileSystemHandlePermissionDescriptor {
  mode?: FileSystemPermissionMode;
}

declare global {
  interface FileSystemDirectoryHandle {
    queryPermission(descriptor?: FileSystemHandlePermissionDescriptor): Promise<PermissionState>;
    requestPermission(descriptor?: FileSystemHandlePermissionDescriptor): Promise<PermissionState>;
  }
}

/** A recent project entry, as persisted in localStorage. */
export interface Recent {
  key: string;
  kind: "sample" | "folder";
  name: string;
  // The picked folder's leaf name (e.g. "model"). Shown beneath the display name,
  // which for a folder with a `pds.toml` is its `[doc].name` rather than the folder.
  // Absent on sample recents and on folder recents saved before this field existed.
  dir?: string;
  sampleId?: string;
  at: number;
}

/** The subset of a sample needed to record it as recently opened. */
export interface RecentSample {
  id: string;
  name: string;
}

const LS_KEY = "pds.recent.v1";
const DB_NAME = "pds-ide";
const STORE = "handles";
const MAX = 8;

const hasLS = (): boolean => typeof localStorage !== "undefined";
const hasIDB = (): boolean => typeof indexedDB !== "undefined";

function readList(): Recent[] {
  if (!hasLS()) return [];
  try {
    return (JSON.parse(localStorage.getItem(LS_KEY) ?? "null") as Recent[] | null) ?? [];
  } catch {
    return [];
  }
}

function writeList(list: Recent[]): void {
  if (!hasLS()) return;
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(list.slice(0, MAX)));
  } catch {
    /* quota / private mode — recents are a convenience, not load-bearing */
  }
}

/** Recent projects, most recent first: `{ key, kind, name, sampleId?, at }`. */
export function getRecents(): Recent[] {
  return readList();
}

function upsert(entry: Recent): void {
  const list = readList().filter((e) => e.key !== entry.key);
  list.unshift(entry);
  writeList(list);
}

/** Record (or bump) a sample as recently opened. */
export function recordSample(sample: RecentSample): void {
  upsert({ key: `sample:${sample.id}`, kind: "sample", name: sample.name, sampleId: sample.id, at: Date.now() });
}

/**
 * Record a folder as recently opened, persisting its handle for a later re-open.
 * `name` is the display label (the `pds.toml` `[doc].name` when present, else the
 * folder name); `dir` is the folder's leaf name, keyed on and shown as the subtitle.
 */
export async function recordFolder(
  name: string,
  dir: string,
  rootHandle: FileSystemDirectoryHandle | null,
): Promise<void> {
  const key = `folder:${dir}`;
  if (rootHandle) {
    try {
      await putHandle(key, rootHandle);
    } catch {
      /* handle persistence unavailable — the entry still shows, re-open re-picks */
    }
  }
  upsert({ key, kind: "folder", name, dir, at: Date.now() });
}

/**
 * The stored directory handle for a recent folder, after re-granting permission,
 * or null if it cannot be restored (no handle, denied, or moved/deleted).
 */
export async function reopenFolder(key: string): Promise<FileSystemDirectoryHandle | null> {
  try {
    const handle = await getHandle(key);
    if (!handle) return null;
    const opts: FileSystemHandlePermissionDescriptor = { mode: "readwrite" };
    let perm = await handle.queryPermission(opts);
    if (perm !== "granted") perm = await handle.requestPermission(opts);
    return perm === "granted" ? handle : null;
  } catch {
    return null;
  }
}

/** Drop a recent entry (and any stored handle). */
export function forget(key: string): void {
  writeList(readList().filter((e) => e.key !== key));
  delHandle(key).catch(() => {});
}

// ---- IndexedDB handle store ------------------------------------------------

function db(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (!hasIDB()) return reject(new Error("no indexedDB"));
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function putHandle(key: string, handle: FileSystemDirectoryHandle): Promise<void> {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(handle, key);
    tx.oncomplete = () => res();
    tx.onerror = () => rej(tx.error);
  });
}

async function getHandle(key: string): Promise<FileSystemDirectoryHandle | null> {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readonly");
    const r = tx.objectStore(STORE).get(key);
    r.onsuccess = () => res((r.result as FileSystemDirectoryHandle | undefined) ?? null);
    r.onerror = () => rej(r.error);
  });
}

async function delHandle(key: string): Promise<void> {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readwrite");
    tx.objectStore(STORE).delete(key);
    tx.oncomplete = () => res();
    tx.onerror = () => rej(tx.error);
  });
}
