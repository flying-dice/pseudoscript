// Imperative sidebar behaviours, run once after hydration (client only):
// a collapsible tree, a client-side filter over node name / FQN, and the
// active-node highlight that follows the URL hash. Diagram pan/zoom is handled
// by Svelte Flow's own controls, so it is not reproduced here.

export function initBehaviors(root) {
  initTree(root);
  initSearch(root);
  markActive(root);
  markActiveDoc(root);
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
      if (li) li.classList.toggle("collapsed");
    });
  });
}

// ---- search filter ---------------------------------------------------------

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
