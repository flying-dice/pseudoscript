// Propless progressive enhancement, run once on DOMContentLoaded (client
// only — never under QuickJS SSR, never hydrated). Everything here wires the
// server-rendered markup by DOM: the mobile drawer, the theme toggle, figure
// pan/zoom, code-block copy buttons, the search palette, the sidebar tree
// (collapse + filter), and the active-node highlight.

export function initBehaviors(root) {
  initDrawer(root);
  initTheme(root);
  initFigures(root);
  initCopyButtons(root);
  initPalette(root);
  initTree(root);
  initSearch(root);
  markActive(root);
  markActiveDoc(root);
}

/// The `../`-to-root prefix the SSR pass stamped on the shell.
function sitePrefix(root) {
  const el = root.querySelector("[data-prefix]");
  return el ? el.getAttribute("data-prefix") || "" : "";
}

// ---- mobile drawer ----------------------------------------------------------

function initDrawer(root) {
  const hamburger = root.querySelector(".hamburger");
  const sidebar = root.querySelector("#sidebar");
  const closeBtn = root.querySelector(".sidebar-close");
  const backdrop = root.querySelector("[data-backdrop]");
  if (!hamburger || !sidebar) return;

  let restoreFocus = null;

  function setOpen(open) {
    document.body.classList.toggle("sidebar-open", open);
    hamburger.setAttribute("aria-expanded", open ? "true" : "false");
    if (open) {
      restoreFocus = document.activeElement;
      if (closeBtn) closeBtn.focus();
    } else if (restoreFocus && restoreFocus.focus) {
      restoreFocus.focus();
      restoreFocus = null;
    }
  }

  hamburger.addEventListener("click", () => {
    setOpen(!document.body.classList.contains("sidebar-open"));
  });
  if (closeBtn) closeBtn.addEventListener("click", () => setOpen(false));
  if (backdrop) backdrop.addEventListener("click", () => setOpen(false));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && document.body.classList.contains("sidebar-open")) {
      setOpen(false);
    }
  });
}

// ---- theme toggle -----------------------------------------------------------

const THEME_KEY = "pds-doc-theme";

function initTheme(root) {
  const toggle = root.querySelector(".theme-toggle");
  if (!toggle) return;

  function stored() {
    try {
      const t = localStorage.getItem(THEME_KEY);
      return t === "light" || t === "dark" ? t : null;
    } catch (e) {
      return null;
    }
  }

  function resolve(theme) {
    if (theme !== "system") return theme;
    return window.matchMedia && matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }

  function apply(theme) {
    document.documentElement.setAttribute("data-theme", resolve(theme));
    try {
      if (theme === "system") localStorage.removeItem(THEME_KEY);
      else localStorage.setItem(THEME_KEY, theme);
    } catch (e) {
      /* storage unavailable (file://, private mode) — theme still applies */
    }
    window.dispatchEvent(new CustomEvent("pds-themechange"));
  }

  toggle.addEventListener("click", () => {
    const current = stored() || "system";
    const next = current === "system" ? "light" : current === "light" ? "dark" : "system";
    apply(next);
  });
}

// ---- figure pan/zoom --------------------------------------------------------

const ZOOM_MIN = 0.25;
const ZOOM_MAX = 8;

function initFigures(root) {
  root.querySelectorAll(".figure[data-diagram]").forEach(initFigure);
}

function initFigure(figure) {
  const viewport = figure.querySelector(".fig-viewport");
  const canvas = figure.querySelector(".fig-canvas");
  if (!viewport || !canvas) return;

  let scale = 1;
  let tx = 0;
  let ty = 0;

  function render() {
    canvas.style.transform = "translate(" + tx + "px, " + ty + "px) scale(" + scale + ")";
  }

  function reset() {
    scale = 1;
    tx = 0;
    ty = 0;
    render();
  }

  // Zoom by `factor` anchored at viewport point (px, py): the model point
  // under the cursor stays under the cursor.
  function zoomAt(factor, px, py) {
    const next = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, scale * factor));
    const applied = next / scale;
    tx = px - (px - tx) * applied;
    ty = py - (py - ty) * applied;
    scale = next;
    render();
  }

  function center() {
    const rect = viewport.getBoundingClientRect();
    return { x: rect.width / 2, y: rect.height / 2 };
  }

  viewport.style.touchAction = "none";

  viewport.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const rect = viewport.getBoundingClientRect();
      const factor = Math.exp(-event.deltaY * 0.0015);
      zoomAt(factor, event.clientX - rect.left, event.clientY - rect.top);
    },
    { passive: false }
  );

  let dragging = null;
  viewport.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    dragging = { x: event.clientX, y: event.clientY };
    viewport.setPointerCapture(event.pointerId);
    viewport.classList.add("is-panning");
  });
  viewport.addEventListener("pointermove", (event) => {
    if (!dragging) return;
    tx += event.clientX - dragging.x;
    ty += event.clientY - dragging.y;
    dragging = { x: event.clientX, y: event.clientY };
    render();
  });
  function endDrag(event) {
    if (!dragging) return;
    dragging = null;
    viewport.classList.remove("is-panning");
    if (event.pointerId !== undefined && viewport.hasPointerCapture(event.pointerId)) {
      viewport.releasePointerCapture(event.pointerId);
    }
  }
  viewport.addEventListener("pointerup", endDrag);
  viewport.addEventListener("pointercancel", endDrag);

  viewport.addEventListener("dblclick", (event) => {
    event.preventDefault();
    reset();
  });

  function fullscreen() {
    if (figure.requestFullscreen) {
      if (document.fullscreenElement === figure) document.exitFullscreen();
      else figure.requestFullscreen();
    } else {
      figure.classList.toggle("fig-fullscreen");
    }
  }

  figure.querySelectorAll(".fig-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const action = btn.getAttribute("data-fig");
      const c = center();
      if (action === "zoom-in") zoomAt(1.25, c.x, c.y);
      else if (action === "zoom-out") zoomAt(0.8, c.x, c.y);
      else if (action === "reset") reset();
      else if (action === "fullscreen") fullscreen();
    });
  });

  figure.addEventListener("keydown", (event) => {
    if (event.target !== figure) return;
    const c = center();
    if (event.key === "+" || event.key === "=") zoomAt(1.25, c.x, c.y);
    else if (event.key === "-") zoomAt(0.8, c.x, c.y);
    else if (event.key === "0") reset();
    else if (event.key === "f") fullscreen();
    else return;
    event.preventDefault();
  });
}

// ---- code-block copy buttons ------------------------------------------------

function initCopyButtons(root) {
  root.querySelectorAll("pre.code-block").forEach((pre) => {
    const code = pre.querySelector("code");
    if (!code) return;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "copy-btn";
    btn.textContent = "Copy";
    btn.setAttribute("aria-label", "Copy code");
    btn.addEventListener("click", () => {
      copyText(code.textContent || "");
      btn.textContent = "✓";
      btn.classList.add("copied");
      setTimeout(() => {
        btn.textContent = "Copy";
        btn.classList.remove("copied");
      }, 1200);
    });
    pre.appendChild(btn);
  });
}

function copyText(text) {
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).catch(() => fallbackCopy(text));
  } else {
    fallbackCopy(text);
  }
}

function fallbackCopy(text) {
  const area = document.createElement("textarea");
  area.value = text;
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  try {
    document.execCommand("copy");
  } catch (e) {
    /* nothing left to try */
  }
  area.remove();
}

// ---- search palette ---------------------------------------------------------

// A plain-JS command palette over the static index (`search-index.js`, a
// classic script assigning window.__PDS_SEARCH__). The index script is
// injected on first open, so pages that never search never load it.

function initPalette(root) {
  const prefix = sitePrefix(root);
  let overlay = null;
  let input = null;
  let list = null;
  let results = [];
  let selected = 0;
  let restoreFocus = null;
  let indexRequested = false;

  function ensureIndex(onReady) {
    if (window.__PDS_SEARCH__) {
      onReady();
      return;
    }
    if (!indexRequested) {
      indexRequested = true;
      const script = document.createElement("script");
      script.src = prefix + "search-index.js";
      document.head.appendChild(script);
    }
    const poll = setInterval(() => {
      if (window.__PDS_SEARCH__) {
        clearInterval(poll);
        onReady();
      }
    }, 50);
    setTimeout(() => clearInterval(poll), 5000);
  }

  function build() {
    overlay = document.createElement("div");
    overlay.className = "palette-overlay";

    const dialog = document.createElement("div");
    dialog.className = "palette";
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-label", "Search");

    input = document.createElement("input");
    input.type = "text";
    input.className = "palette-input";
    input.placeholder = "Search the model…";
    input.setAttribute("aria-label", "Search the model");
    input.autocomplete = "off";
    input.spellcheck = false;

    list = document.createElement("ul");
    list.className = "palette-results";

    dialog.appendChild(input);
    dialog.appendChild(list);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) close();
    });
    input.addEventListener("input", () => {
      ensureIndex(() => renderResults(search(input.value)));
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown") {
        selected = Math.min(selected + 1, results.length - 1);
        renderSelection();
        event.preventDefault();
      } else if (event.key === "ArrowUp") {
        selected = Math.max(selected - 1, 0);
        renderSelection();
        event.preventDefault();
      } else if (event.key === "Enter") {
        const hit = results[selected];
        if (hit) location.href = prefix + hit.href;
        event.preventDefault();
      } else if (event.key === "Escape") {
        close();
        event.preventDefault();
      }
    });
  }

  function open() {
    if (!overlay) build();
    restoreFocus = document.activeElement;
    overlay.classList.add("open");
    input.value = "";
    results = [];
    selected = 0;
    list.innerHTML = "";
    input.focus();
    ensureIndex(() => {});
  }

  function close() {
    if (!overlay) return;
    overlay.classList.remove("open");
    if (restoreFocus && restoreFocus.focus) restoreFocus.focus();
    restoreFocus = null;
  }

  function isOpen() {
    return Boolean(overlay && overlay.classList.contains("open"));
  }

  // Ranked substring search: name hits first, then fqn, then summary/text.
  function search(query) {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const index = window.__PDS_SEARCH__ || [];
    const scored = [];
    for (const entry of index) {
      let score = null;
      if ((entry.name || "").toLowerCase().indexOf(q) !== -1) score = 0;
      else if ((entry.fqn || "").toLowerCase().indexOf(q) !== -1) score = 1;
      else if (
        (entry.summary || "").toLowerCase().indexOf(q) !== -1 ||
        (entry.text || "").toLowerCase().indexOf(q) !== -1
      ) {
        score = 2;
      }
      if (score !== null) scored.push({ score, entry });
    }
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, 20).map((s) => s.entry);
  }

  function renderResults(hits) {
    results = hits;
    selected = 0;
    list.innerHTML = "";
    if (!hits.length) {
      const li = document.createElement("li");
      li.className = "palette-empty";
      li.textContent = input.value.trim() ? "No matches" : "";
      list.appendChild(li);
      return;
    }
    hits.forEach((entry, i) => {
      const li = document.createElement("li");
      li.className = "palette-item";

      const kind = document.createElement("span");
      kind.className = "palette-kind kind-" + entry.kind;
      kind.textContent = entry.kind;

      const name = document.createElement("span");
      name.className = "palette-name";
      name.textContent = entry.name;

      const fqn = document.createElement("span");
      fqn.className = "palette-fqn";
      fqn.textContent = entry.fqn;

      const head = document.createElement("div");
      head.className = "palette-head";
      head.appendChild(kind);
      head.appendChild(name);
      head.appendChild(fqn);
      li.appendChild(head);

      if (entry.summary) {
        const summary = document.createElement("div");
        summary.className = "palette-summary";
        summary.textContent = entry.summary;
        li.appendChild(summary);
      }

      li.addEventListener("click", () => {
        location.href = prefix + entry.href;
      });
      li.addEventListener("mousemove", () => {
        if (selected !== i) {
          selected = i;
          renderSelection();
        }
      });
      list.appendChild(li);
    });
    renderSelection();
  }

  function renderSelection() {
    list.querySelectorAll(".palette-item").forEach((li, i) => {
      li.classList.toggle("is-selected", i === selected);
      li.setAttribute("aria-selected", i === selected ? "true" : "false");
      if (i === selected && li.scrollIntoView) li.scrollIntoView({ block: "nearest" });
    });
  }

  const trigger = root.querySelector(".search-btn");
  if (trigger) trigger.addEventListener("click", open);

  document.addEventListener("keydown", (event) => {
    const inField =
      event.target instanceof HTMLElement &&
      (event.target.tagName === "INPUT" || event.target.tagName === "TEXTAREA");
    if ((event.metaKey || event.ctrlKey) && (event.key === "k" || event.key === "K")) {
      event.preventDefault();
      if (isOpen()) close();
      else open();
    } else if (event.key === "/" && !inField && !isOpen()) {
      event.preventDefault();
      open();
    } else if (event.key === "Escape" && isOpen()) {
      close();
    }
  });
}

// ---- active doc page highlight ---------------------------------------------

// Doc pages (`[[doc.sidebar]]`) are whole-page navigations, not hash anchors, so
// the active link is the one whose href resolves to the current document.
function markActiveDoc(root) {
  const here = location.pathname.split("/").pop();
  if (!here) return;
  root.querySelectorAll(".docs-link").forEach((link) => {
    const target = link.getAttribute("href").split("/").pop().split("#")[0];
    if (target === here) link.classList.add("is-active");
  });
}

// ---- collapsible tree ------------------------------------------------------

function initTree(root) {
  root.querySelectorAll(".tree .toggle").forEach((toggle) => {
    toggle.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const li = toggle.closest("li");
      if (!li) return;
      const collapsed = li.classList.toggle("collapsed");
      toggle.setAttribute("aria-expanded", collapsed ? "false" : "true");
    });
  });
}

// ---- sidebar tree filter ---------------------------------------------------

function initSearch(root) {
  const input = root.querySelector(".search input");
  const tree = root.querySelector(".tree");
  if (!input || !tree) return;

  const empty = document.createElement("li");
  empty.className = "no-results";
  empty.textContent = "No matches";
  empty.hidden = true;
  tree.appendChild(empty);

  input.addEventListener("input", () => {
    const query = input.value.trim().toLowerCase();
    const anyVisible = filter(tree, query);
    empty.hidden = anyVisible || query === "";
  });
}

// Filters the tree to nodes matching `query` at any depth: a node shows when it
// matches, an ancestor matches (subtree revealed), or a descendant matches (the
// path stays open). An empty query restores every node.
function filter(treeRoot, query) {
  if (query === "") {
    treeRoot.querySelectorAll("li").forEach((li) => li.classList.remove("hidden"));
    return true;
  }
  let anyVisible = false;
  treeRoot.querySelectorAll(":scope > li").forEach((li) => {
    if (filterNode(li, query, false)) anyVisible = true;
  });
  return anyVisible;
}

function filterNode(li, query, forced) {
  const hay = (li.dataset.search || "").toLowerCase();
  const match = forced || hay.indexOf(query) !== -1;
  let anyChild = false;
  li.querySelectorAll(":scope > ul.children > li").forEach((child) => {
    if (filterNode(child, query, match)) anyChild = true;
  });
  const visible = match || anyChild;
  li.classList.toggle("hidden", !visible);
  if (visible) li.classList.remove("collapsed");
  return visible;
}

// ---- active node highlight -------------------------------------------------

function markActive(root) {
  function sync() {
    const hash = decodeURIComponent(location.hash.replace(/^#/, ""));
    root.querySelectorAll(".tree .node-link.is-active").forEach((el) => {
      el.classList.remove("is-active");
    });
    if (!hash) return;
    const link = root.querySelector('.tree a[href$="#' + cssEscape(hash) + '"]');
    if (link) {
      link.classList.add("is-active");
      let li = link.closest("li");
      while (li) {
        li.classList.remove("collapsed");
        const parent = li.parentElement;
        li = parent ? parent.closest("li") : null;
      }
    }
  }
  window.addEventListener("hashchange", sync);
  sync();
}

function cssEscape(value) {
  if (window.CSS && window.CSS.escape) return window.CSS.escape(value);
  return value.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
}
