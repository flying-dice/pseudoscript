// PseudoScript documentation site — vanilla, dependency-free.
// Three behaviours: a collapsible sidebar tree, a client-side filter over the
// tree (by node name / FQN), and wheel-zoom + drag-pan on each inline diagram.
// Everything degrades gracefully: with JS off, the tree is fully expanded, the
// search box does nothing, and the diagrams are static SVG.

(function () {
  "use strict";

  document.addEventListener("DOMContentLoaded", function () {
    initTree();
    initSearch();
    initDiagrams();
    markActive();
  });

  // ---- collapsible tree ----------------------------------------------------

  function initTree() {
    document.querySelectorAll(".tree .toggle").forEach(function (toggle) {
      toggle.addEventListener("click", function (event) {
        event.preventDefault();
        event.stopPropagation();
        var li = toggle.closest("li");
        if (li) li.classList.toggle("collapsed");
      });
    });
  }

  // ---- search filter -------------------------------------------------------

  function initSearch() {
    var input = document.querySelector(".search input");
    var tree = document.querySelector(".tree");
    if (!input || !tree) return;

    var empty = document.createElement("li");
    empty.className = "no-results";
    empty.textContent = "No matches";
    empty.hidden = true;
    tree.appendChild(empty);

    input.addEventListener("input", function () {
      var query = input.value.trim().toLowerCase();
      var anyVisible = filter(tree, query);
      empty.hidden = anyVisible || query === "";
    });
  }

  // Filters the tree to nodes matching `query`, at any depth. A node is shown
  // when it matches, an ancestor matches (whole subtree revealed), or a
  // descendant matches (the path to it stays open). An empty query clears the
  // filter and restores every node. Returns true when anything is visible.
  function filter(root, query) {
    if (query === "") {
      root.querySelectorAll("li").forEach(function (li) {
        li.classList.remove("hidden");
      });
      return true;
    }
    var anyVisible = false;
    root.querySelectorAll(":scope > li").forEach(function (li) {
      if (filterNode(li, query, false)) anyVisible = true;
    });
    return anyVisible;
  }

  // Recursively show/hide one `<li>`. `forced` is true when an ancestor already
  // matched, which reveals this node and its whole subtree.
  function filterNode(li, query, forced) {
    var hay = (li.dataset.search || "").toLowerCase();
    var match = forced || hay.indexOf(query) !== -1;
    var anyChild = false;
    li.querySelectorAll(":scope > ul.children > li").forEach(function (child) {
      if (filterNode(child, query, match)) anyChild = true;
    });
    var visible = match || anyChild;
    li.classList.toggle("hidden", !visible);
    if (visible) li.classList.remove("collapsed");
    return visible;
  }

  // ---- diagram zoom / pan --------------------------------------------------

  function initDiagrams() {
    document.querySelectorAll(".diagram").forEach(function (frame) {
      var pan = frame.querySelector(".pan");
      if (!pan) return;

      var state = { scale: 1, x: 0, y: 0 };
      var dragging = false;
      var startX = 0;
      var startY = 0;

      function apply() {
        pan.style.transform =
          "translate(" + state.x + "px," + state.y + "px) scale(" + state.scale + ")";
      }

      frame.addEventListener(
        "wheel",
        function (event) {
          event.preventDefault();
          var rect = frame.getBoundingClientRect();
          var px = event.clientX - rect.left;
          var py = event.clientY - rect.top;
          var factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
          var next = clamp(state.scale * factor, 0.3, 6);
          var ratio = next / state.scale;
          // zoom toward the cursor
          state.x = px - (px - state.x) * ratio;
          state.y = py - (py - state.y) * ratio;
          state.scale = next;
          apply();
        },
        { passive: false }
      );

      frame.addEventListener("pointerdown", function (event) {
        if (event.target.closest(".controls")) return;
        dragging = true;
        startX = event.clientX - state.x;
        startY = event.clientY - state.y;
        frame.classList.add("is-panning");
        frame.setPointerCapture(event.pointerId);
      });

      frame.addEventListener("pointermove", function (event) {
        if (!dragging) return;
        state.x = event.clientX - startX;
        state.y = event.clientY - startY;
        apply();
      });

      function endDrag(event) {
        dragging = false;
        frame.classList.remove("is-panning");
        if (frame.hasPointerCapture && frame.hasPointerCapture(event.pointerId)) {
          frame.releasePointerCapture(event.pointerId);
        }
      }
      frame.addEventListener("pointerup", endDrag);
      frame.addEventListener("pointercancel", endDrag);

      function reset() {
        state.scale = 1;
        state.x = 0;
        state.y = 0;
        apply();
      }

      var resetBtn = frame.querySelector(".zoom-reset");
      if (resetBtn) resetBtn.addEventListener("click", reset);

      var fsBtn = frame.querySelector(".fs-toggle");
      if (fsBtn) {
        fsBtn.addEventListener("click", function () {
          var fig = frame.closest(".figure");
          if (!fig) return;
          if (fullscreenElement() === fig) {
            exitFullscreen();
          } else {
            requestFullscreen(fig);
            reset(); // start fullscreen from a clean, fitted view
          }
        });
      }
    });

    syncFullscreenButtons();
    document.addEventListener("fullscreenchange", syncFullscreenButtons);
    document.addEventListener("webkitfullscreenchange", syncFullscreenButtons);
  }

  // ---- fullscreen helpers --------------------------------------------------

  function fullscreenElement() {
    return document.fullscreenElement || document.webkitFullscreenElement || null;
  }
  function requestFullscreen(el) {
    if (el.requestFullscreen) el.requestFullscreen();
    else if (el.webkitRequestFullscreen) el.webkitRequestFullscreen();
  }
  function exitFullscreen() {
    if (document.exitFullscreen) document.exitFullscreen();
    else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
  }
  // Reflect the current fullscreen state on every toggle (glyph + label), so a
  // button shows "exit" for the active figure and "enter" otherwise. Keeps the
  // UI correct when the user leaves fullscreen with Esc.
  function syncFullscreenButtons() {
    var active = fullscreenElement();
    document.querySelectorAll(".fs-toggle").forEach(function (btn) {
      var on = active !== null && btn.closest(".figure") === active;
      btn.textContent = on ? "✕" : "⛶";
      btn.title = on ? "Exit fullscreen" : "Fullscreen";
    });
  }

  // ---- active node highlight ----------------------------------------------

  function markActive() {
    function sync() {
      var hash = decodeURIComponent(location.hash.replace(/^#/, ""));
      document.querySelectorAll(".tree .node-link.is-active").forEach(function (el) {
        el.classList.remove("is-active");
      });
      if (!hash) return;
      var link = document.querySelector('.tree a[href$="#' + cssEscape(hash) + '"]');
      if (link) {
        link.classList.add("is-active");
        // Expand every ancestor branch so the active node is revealed.
        var li = link.closest("li");
        while (li) {
          li.classList.remove("collapsed");
          var parent = li.parentElement;
          li = parent ? parent.closest("li") : null;
        }
      }
    }
    window.addEventListener("hashchange", sync);
    sync();
  }

  function clamp(value, lo, hi) {
    return Math.min(hi, Math.max(lo, value));
  }

  function cssEscape(value) {
    if (window.CSS && window.CSS.escape) return window.CSS.escape(value);
    return value.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }
})();
