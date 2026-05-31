// Recent projects for the project panel: a small localStorage list of metadata
// plus an IndexedDB store of folder handles, so an opened folder re-opens
// without re-picking it (subject to a permission re-grant). Samples re-open
// directly by id. Everything is best-effort and guarded for SSR / private mode.

const LS_KEY = "pds.recent.v1";
const DB_NAME = "pds-ide";
const STORE = "handles";
const MAX = 8;

const hasLS = () => typeof localStorage !== "undefined";
const hasIDB = () => typeof indexedDB !== "undefined";

function readList() {
  if (!hasLS()) return [];
  try {
    return JSON.parse(localStorage.getItem(LS_KEY)) ?? [];
  } catch {
    return [];
  }
}

function writeList(list) {
  if (!hasLS()) return;
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(list.slice(0, MAX)));
  } catch {
    /* quota / private mode — recents are a convenience, not load-bearing */
  }
}

/** Recent projects, most recent first: `{ key, kind, name, sampleId?, at }`. */
export function getRecents() {
  return readList();
}

function upsert(entry) {
  const list = readList().filter((e) => e.key !== entry.key);
  list.unshift(entry);
  writeList(list);
}

/** Record (or bump) a sample as recently opened. */
export function recordSample(sample) {
  upsert({ key: `sample:${sample.id}`, kind: "sample", name: sample.name, sampleId: sample.id, at: Date.now() });
}

/** Record a folder as recently opened, persisting its handle for a later re-open. */
export async function recordFolder(name, rootHandle) {
  const key = `folder:${name}`;
  if (rootHandle) {
    try {
      await putHandle(key, rootHandle);
    } catch {
      /* handle persistence unavailable — the entry still shows, re-open re-picks */
    }
  }
  upsert({ key, kind: "folder", name, at: Date.now() });
}

/**
 * The stored directory handle for a recent folder, after re-granting permission,
 * or null if it cannot be restored (no handle, denied, or moved/deleted).
 */
export async function reopenFolder(key) {
  try {
    const handle = await getHandle(key);
    if (!handle) return null;
    const opts = { mode: "readwrite" };
    let perm = await handle.queryPermission(opts);
    if (perm !== "granted") perm = await handle.requestPermission(opts);
    return perm === "granted" ? handle : null;
  } catch {
    return null;
  }
}

/** Drop a recent entry (and any stored handle). */
export function forget(key) {
  writeList(readList().filter((e) => e.key !== key));
  delHandle(key).catch(() => {});
}

// ---- IndexedDB handle store ------------------------------------------------

function db() {
  return new Promise((resolve, reject) => {
    if (!hasIDB()) return reject(new Error("no indexedDB"));
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function putHandle(key, handle) {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(handle, key);
    tx.oncomplete = () => res();
    tx.onerror = () => rej(tx.error);
  });
}

async function getHandle(key) {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readonly");
    const r = tx.objectStore(STORE).get(key);
    r.onsuccess = () => res(r.result ?? null);
    r.onerror = () => rej(r.error);
  });
}

async function delHandle(key) {
  const d = await db();
  return new Promise((res, rej) => {
    const tx = d.transaction(STORE, "readwrite");
    tx.objectStore(STORE).delete(key);
    tx.oncomplete = () => res();
    tx.onerror = () => rej(tx.error);
  });
}
