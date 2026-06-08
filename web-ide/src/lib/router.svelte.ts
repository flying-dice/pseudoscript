// A tiny react-router-style hash router for the single-page IDE shell. The whole
// app stays mounted in one `+page.svelte`, so there is no page swap to route —
// the "route" is the live session encoded in `location.hash`:
//
//   #/<base>/<view>?f=<fileFqn>&n=<selectedFqn>&l=<line>&c=<col>&d=<seqDepth>
//
// `<base>` is kind-tagged in one path segment: `f.<folder name>` for a disk-backed
// project (the FileSystemDirectoryHandle.name — the only path the File System
// Access API exposes) or `w.<base64url>` for an embedded in-memory workspace
// (shared/imported). `parseHash`/`serializeRoute` are pure (the unit-tested core);
// `HashRouter` adds the reactive `route` and the `location.hash` read/write.

/** The centre view the route addresses. */
export type RouteView = "code" | "canvas" | "space";

/** A route base: a folder reference (`f`), an embedded workspace (`w`), or none. */
export type RouteBase = { kind: "f" | "w" | null; value: string };

/** The parsed URL session: workspace base + the current location within it. */
export interface Route {
  base: RouteBase;
  view: RouteView;
  /** Active module file FQN (modules only; docs/manifest carry no FQN). */
  file?: string;
  /** Selected node FQN (canvas/space scope, or an editor node scope). */
  node?: string;
  /** Caret line (1-based) and column (0-based), code view only. */
  line?: number;
  col?: number;
  /** Sequence-diagram collapse depth (canvas view). */
  depth?: string;
}

const VIEWS: readonly RouteView[] = ["code", "canvas", "space"];

/** The empty route (no workspace) — boot lands here when the hash is bare. */
export function emptyRoute(): Route {
  return { base: { kind: null, value: "" }, view: "code" };
}

function asView(seg: string | undefined): RouteView {
  return seg && (VIEWS as readonly string[]).includes(seg) ? (seg as RouteView) : "code";
}

function parseBase(seg: string | undefined): RouteBase {
  // `f.<name>` or `w.<payload>`. The `f` value is percent-encoded; the `w` value
  // is base64url (already URL-safe, so decoding is a harmless no-op).
  if (!seg || (seg[0] !== "f" && seg[0] !== "w") || seg[1] !== ".") return { kind: null, value: "" };
  return { kind: seg[0] as "f" | "w", value: decodeURIComponent(seg.slice(2)) };
}

function numParam(v: string | null): number | undefined {
  if (v === null) return undefined;
  const n = Number(v);
  return Number.isFinite(n) ? n : undefined;
}

/** Parse a `location.hash` string into a {@link Route}. Tolerant: anything that
 *  isn't the `#/<base>/…` shape (e.g. a legacy `#w=…` share link, or an empty
 *  hash) yields a kind-`null` base for the caller to handle. */
export function parseHash(hash: string): Route {
  const s = hash.replace(/^#/, "");
  if (!s.startsWith("/")) return emptyRoute();
  const [path, queryStr = ""] = s.split("?");
  const segs = path.split("/").filter(Boolean);
  const base = parseBase(segs[0]);
  const view = asView(segs[1]);
  const q = new URLSearchParams(queryStr);
  const route: Route = { base, view };
  const file = q.get("f");
  const node = q.get("n");
  const depth = q.get("d");
  if (file) route.file = file;
  if (node) route.node = node;
  const line = numParam(q.get("l"));
  const col = numParam(q.get("c"));
  if (line !== undefined) route.line = line;
  if (col !== undefined) route.col = col;
  if (depth) route.depth = depth;
  return route;
}

/** Serialize a {@link Route} to a `#/<base>/<view>?…` hash string. A kind-`null`
 *  base serializes to a bare `#` (the launcher URL). */
export function serializeRoute(route: Route): string {
  if (route.base.kind === null) return "#";
  const value = route.base.kind === "f" ? encodeURIComponent(route.base.value) : route.base.value;
  const q = new URLSearchParams();
  if (route.file) q.set("f", route.file);
  if (route.node) q.set("n", route.node);
  if (route.line !== undefined) q.set("l", String(route.line));
  if (route.col !== undefined) q.set("c", String(route.col));
  if (route.depth) q.set("d", route.depth);
  const query = q.toString();
  return `#/${route.base.kind}.${value}/${route.view}${query ? `?${query}` : ""}`;
}

/** Writes a URL to the history, replacing or pushing. Injected so the SvelteKit
 *  app can route writes through `$app/navigation` (raw `history.replaceState`
 *  conflicts with SvelteKit's client router); the default is the native API, used
 *  outside SvelteKit / in tests. */
export type HistoryWriter = (url: string, replace: boolean) => void;

const nativeWriter: HistoryWriter = (url, replace) => {
  if (typeof window === "undefined") return;
  if (replace) window.history.replaceState(null, "", url);
  else window.history.pushState(null, "", url);
};

/** The reactive hash router: holds the current {@link Route} and reads/writes
 *  `location.hash`. `navigate` merges a patch and writes via `replaceState`
 *  (no history spam); `start` wires the `hashchange`/`popstate` listeners. */
class HashRouter {
  route = $state<Route>(emptyRoute());
  // The last hash this router wrote, so the listener ignores our own writes
  // (defence-in-depth — replaceState doesn't fire hashchange, but back/forward and
  // manual edits do, and a no-op guard keeps navigate idempotent).
  #last = "";
  #write: HistoryWriter = nativeWriter;

  /** Route history writes through a host-supplied writer (e.g. SvelteKit's
   *  `$app/navigation`). Call before {@link start}. */
  configureWriter(write: HistoryWriter): void {
    this.#write = write;
  }

  /** Begin tracking `location.hash` (call once on mount). */
  start(): void {
    if (typeof window === "undefined") return;
    this.#last = location.hash;
    this.route = parseHash(location.hash);
    window.addEventListener("hashchange", this.#onExternal);
    window.addEventListener("popstate", this.#onExternal);
  }

  stop(): void {
    if (typeof window === "undefined") return;
    window.removeEventListener("hashchange", this.#onExternal);
    window.removeEventListener("popstate", this.#onExternal);
  }

  #onExternal = (): void => {
    if (location.hash === this.#last) return; // our own write
    this.#last = location.hash;
    this.route = parseHash(location.hash);
  };

  /** Merge `patch` into the current route and write it to the hash. No-op when the
   *  resulting hash is unchanged (idempotent). */
  navigate(patch: Partial<Route>, { replace = true }: { replace?: boolean } = {}): void {
    const next: Route = { ...this.route, ...patch };
    const hash = serializeRoute(next);
    if (hash === this.#last) {
      this.route = next;
      return;
    }
    this.route = next;
    this.#last = hash;
    if (typeof window === "undefined") return;
    this.#write(location.pathname + location.search + hash, replace);
  }

  /** Clear the route to the bare launcher URL (on close-project). */
  clear(): void {
    this.route = emptyRoute();
    if (typeof window === "undefined") return;
    this.#last = "";
    this.#write(location.pathname + location.search, true);
  }
}

export const router = new HashRouter();
