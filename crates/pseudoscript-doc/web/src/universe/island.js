// The universe page's 3D island — the web IDE's ForceGraph.svelte ported to plain
// JS. A 3D force-directed graph of the model, clustered by C4 containment and with
// relationships *routed through their gateways*: a relationship between two
// components travels up through each side's container and system (the gateway hubs)
// and across at the level where the two sides meet. Hovering a relationship (or a
// hub) lights the whole end-to-end chain; clicking a node focuses it and opens an
// info card with its documentation link; clicking a flow filament opens the flow's
// bus-route highlight and step timeline.
//
// Layout is d3-force-3d (containment links cluster; gateway "bridge" links pull
// related hubs together); render is Three.js + OrbitControls in the doc-site theme.
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import {
  forceSimulation,
  forceManyBody,
  forceLink,
  forceCenter,
  forceCollide,
  forceX,
  forceY,
  forceZ,
} from "d3-force-3d";
import { ancestors, routeOf, simpleName } from "./graph-route.js";

const SIZE = { system: 6, container: 3.4, component: 1.9, person: 3 };
const REL_OPACITY = 0.15;

/**
 * Mount the 3D universe into `host`. `data` is the universe page body plus hrefs:
 * `{ nodes, edges, flows, hrefs }` (serde camelCase, straight off window.__DATA__.page).
 * Returns a dispose function that tears the whole scene down (GPU resources, DOM
 * chrome, listeners) — the entry remounts on theme change so colours re-read the
 * CSS custom properties.
 *
 * @param {HTMLElement} host
 * @param {{
 *   nodes: { id: string, level: string, parent: string | null }[],
 *   edges: { from: string, to: string, traffic: number }[],
 *   flows: { fqn: string, name: string, color: string, hops: { from: string, to: string, label: string }[] }[],
 *   hrefs: { id: string, href: string }[],
 * }} data
 * @returns {() => void}
 */
export function mountUniverse(host, data) {
  // ---- DOM chrome (plain DOM — no client Svelte runtime) -------------------
  host.textContent = "";
  /** @type {(tag: string, cls: string, parent?: Element) => HTMLElement} */
  const el = (tag, cls, parent) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    (parent ?? host).appendChild(e);
    return e;
  };
  const cv = /** @type {HTMLCanvasElement} */ (el("canvas", "uv-canvas"));
  const hint = el("div", "uv-hint");
  const flowCount = (data.flows ?? []).length;
  hint.innerHTML = `${data.nodes.length} nodes &middot; ${flowCount} flows &middot; <b>hover</b> a leg &middot; <b>click</b> to open a flow &middot; <b>drag</b> orbit &middot; <b>scroll</b> zoom`;
  const recenterBtn = el("button", "uv-recenter");
  recenterBtn.type = "button";
  recenterBtn.title = "Re-center the graph";
  recenterBtn.setAttribute("aria-label", "Re-center the graph");
  recenterBtn.textContent = "⌖ Re-center";
  const legend = el("div", "uv-legend");
  for (const kind of ["system", "container", "component", "person"]) {
    const s = el("span", "", legend);
    el("i", "", s).style.background = `var(--k-${kind})`;
    s.append(kind);
  }
  const tip = el("div", "uv-tip");
  const timeline = el("div", "uv-timeline");
  timeline.hidden = true;
  const card = el("div", "uv-card");
  card.hidden = true;
  const hrefOf = new Map((data.hrefs ?? []).map((h) => [h.id, h.href]));

  const W = () => cv.clientWidth,
    H = () => cv.clientHeight;

  // Brand palette, read live off the active theme (so dark/light + accent track).
  const css = getComputedStyle(document.documentElement);
  const cssColor = (name, fallback) => new THREE.Color(css.getPropertyValue(name).trim() || fallback);
  const COLOR = {
    system: cssColor("--k-system", "#ff6a52"),
    container: cssColor("--k-container", "#2dd4bf"),
    component: cssColor("--k-component", "#e0a93f"),
    person: cssColor("--k-person", "#6e8bff"),
  };
  const fallbackColor = cssColor("--ink-soft", "#aeb0ba");
  const bgColor = cssColor("--surface", "#15161c");
  const treeColor = cssColor("--ink-faint", "#71747f");
  const relColor = cssColor("--ink-soft", "#aeb0ba");
  const hotColor = cssColor("--accent", "#ff5a36");
  // A pale sheet vs the dark void: drives additive-vs-alpha choices throughout (additive
  // glow is a no-op on near-white, so the bright effects switch to alpha there).
  const lightBg = bgColor.r + bgColor.g + bgColor.b > 1.5;

  // ---- containment + routing ----------------------------------------------
  // `ancestors`/`routeOf` are the pure gateway-routing logic (graph-route.js).
  const ids = new Set(data.nodes.map((n) => n.id));
  const levelOf = new Map(data.nodes.map((n) => [n.id, n.level]));
  const parentOf = new Map(data.nodes.map((n) => [n.id, n.parent ?? null]));
  const anc = (id) => ancestors(id, parentOf);

  const routes = data.edges
    .filter((e) => e.from !== e.to && ids.has(e.from) && ids.has(e.to))
    .map((e) => ({ from: e.from, to: e.to, traffic: e.traffic, ...routeOf(e.from, e.to, parentOf) }));

  // ---- d3-force-3d layout (one mount per page load — no cache needed) ------
  // Containment links cluster: short + strong for container→component, wider for
  // system→container so component balls don't collide. Also drives the faint tree.
  const contain = data.nodes
    .filter((n) => n.parent && ids.has(n.parent))
    .map((n) => ({ source: n.parent, target: n.id, dist: levelOf.get(n.parent) === "system" ? 60 : 16 }));
  const pos = new Map();
  {
    const nodes = data.nodes.map((n) => ({ id: n.id, level: n.level }));
    // Relationships pull at their *gateway bridge*, not component-to-component — so
    // related systems/containers drift together while their internals stay clustered.
    const relForce = routes
      .filter((r) => r.bridge && r.bridge[0] !== r.bridge[1])
      .map((r) => {
        const [u, v] = r.bridge;
        const lv = (x) => levelOf.get(x);
        const dist = lv(u) === "system" || lv(v) === "system" ? 110 : lv(u) === "container" || lv(v) === "container" ? 45 : 25;
        return { source: u, target: v, dist };
      });
    const sim = forceSimulation(nodes, 3)
      .force("charge", forceManyBody().strength(-55))
      .force("contain", forceLink(contain).id((d) => d.id).distance((l) => l.dist).strength(0.9))
      .force("link", forceLink(relForce).id((d) => d.id).distance((l) => l.dist).strength(0.12))
      .force("collide", forceCollide((d) => (SIZE[d.level] ?? 2) + 1.5))
      .force("x", forceX().strength(0.04))
      .force("y", forceY().strength(0.04))
      .force("z", forceZ().strength(0.04))
      .force("center", forceCenter())
      .stop();
    for (let i = 0; i < 400; i++) sim.tick();
    for (const n of nodes) pos.set(n.id, new THREE.Vector3(n.x, n.y, n.z));
  }

  // ---- Three.js render ----------------------------------------------------
  const renderer = new THREE.WebGLRenderer({ canvas: cv, antialias: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5)); // cap retina cost (4x→2.25x pixels)
  renderer.setSize(W(), H());
  const scene = new THREE.Scene();
  scene.background = bgColor;
  const camera = new THREE.PerspectiveCamera(55, W() / H(), 0.1, 8000);
  const controls = new OrbitControls(camera, cv);
  controls.enableDamping = true;
  // Lighting builds form, not just brightness: a hemisphere light (white sky → backdrop-
  // tinted ground) wraps each orb in a top-to-bottom gradient, an angled key light carves
  // the highlight, and a dim opposite fill keeps the shadow side from going flat-black.
  scene.add(new THREE.HemisphereLight(0xffffff, bgColor, 0.5));
  const keyLight = new THREE.DirectionalLight(0xffffff, 0.8);
  keyLight.position.set(1, 1.3, 0.7);
  scene.add(keyLight);
  const fillLight = new THREE.DirectionalLight(0xffffff, 0.22);
  fillLight.position.set(-1, -0.2, -0.6);
  scene.add(fillLight);

  // Nodes: every placed sphere in ONE InstancedMesh — a single draw call for the lot
  // (vs. one per node). Selection dims/brightens via per-instance colour, not opacity,
  // so the mesh stays opaque (no per-object transparency sort).
  const placed = data.nodes.filter((n) => pos.has(n.id));
  const baseCol = placed.map((n) => (COLOR[n.level] ?? fallbackColor).clone());
  const nodeGeo = new THREE.SphereGeometry(1, 24, 16); // smooth orbs (shared by one InstancedMesh, so cost is paid once)
  const nodeMat = new THREE.MeshStandardMaterial({ roughness: 0.5, metalness: 0.15 });
  const nodes3d = new THREE.InstancedMesh(nodeGeo, nodeMat, placed.length);
  const dummy = new THREE.Object3D();
  for (let i = 0; i < placed.length; i++) {
    const n = placed[i];
    dummy.position.copy(pos.get(n.id));
    dummy.scale.setScalar(SIZE[n.level] ?? 2.2);
    dummy.updateMatrix();
    nodes3d.setMatrixAt(i, dummy.matrix);
    nodes3d.setColorAt(i, baseCol[i]);
  }
  nodes3d.instanceMatrix.needsUpdate = true;
  if (nodes3d.instanceColor) nodes3d.instanceColor.needsUpdate = true;
  scene.add(nodes3d);

  // Brighten the nodes in `set`, darken the rest; `null` restores all. Colour-only.
  const dimC = new THREE.Color();
  const paintNodes = (set) => {
    for (let i = 0; i < placed.length; i++) {
      const on = !set || set.has(placed[i].id);
      dimC.copy(baseCol[i]);
      if (set && !on) dimC.multiplyScalar(0.18);
      nodes3d.setColorAt(i, dimC);
    }
    if (nodes3d.instanceColor) nodes3d.instanceColor.needsUpdate = true;
  };

  // Labels on every node, sized relative to the node — big suns read large, small
  // components small. Billboarded sprites; `h` is the world height of the text.
  const inkHex = css.getPropertyValue("--ink").trim() || "#f4f4f6";
  const makeLabel = (text, h) => {
    const dpr = 2,
      font = 30;
    const c = document.createElement("canvas");
    const ctx = c.getContext("2d");
    ctx.font = `600 ${font}px ui-sans-serif, system-ui, sans-serif`;
    const tw = Math.ceil(ctx.measureText(text).width);
    c.width = (tw + 14) * dpr;
    c.height = (font + 12) * dpr;
    ctx.scale(dpr, dpr);
    ctx.font = `600 ${font}px ui-sans-serif, system-ui, sans-serif`;
    ctx.textBaseline = "middle";
    ctx.fillStyle = inkHex;
    ctx.fillText(text, 7, (font + 12) / 2);
    const tex = new THREE.CanvasTexture(c);
    tex.minFilter = THREE.LinearFilter;
    const spr = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false }));
    spr.scale.set(h * (c.width / c.height), h, 1);
    return spr;
  };
  for (const n of data.nodes) {
    const p = pos.get(n.id);
    if (!p) continue;
    const size = SIZE[n.level] ?? 2.2;
    const h = Math.max(1.8, size * 0.85); // label height tracks node size
    const spr = makeLabel(simpleName(n.id), h);
    spr.position.copy(p).add(new THREE.Vector3(0, size + h * 0.7, 0));
    scene.add(spr);
  }

  // The faint containment tree, so structure reads even where there are no relationships.
  const treePos = [];
  for (const c of contain) {
    const a = pos.get(c.source),
      b = pos.get(c.target);
    if (!a || !b) continue;
    treePos.push(a.x, a.y, a.z, b.x, b.y, b.z);
  }
  const treeGeo = new THREE.BufferGeometry();
  treeGeo.setAttribute("position", new THREE.Float32BufferAttribute(treePos, 3));
  scene.add(new THREE.LineSegments(treeGeo, new THREE.LineBasicMaterial({ color: treeColor, transparent: true, opacity: 0.14 })));

  // ---- layout relationships vs. data flows -------------------------------
  // Two DIFFERENT things, drawn separately:
  //  • LAYOUT relationships — the structural edges routed up through the containment
  //    *parent pathway*. Pure layout: plain, faint, overlap is fine.
  //  • DATA FLOWS — the actual flows running directly between two spheres, each leg a
  //    straight line, with beads of light tracking along.
  // The parent-pathway polyline (gateway stops). Plain — no offset, overlap allowed.
  const parentLine = (pathIds) => pathIds.map((id) => pos.get(id)).filter((v) => !!v);
  // A straight segment between two nodes — endpoints only. Used for both the resting
  // leg filaments and the selected flow's bus route, so flows read dead-straight.
  const straight = (fromId, toId) => {
    const a = pos.get(fromId),
      b = pos.get(toId);
    return a && b ? [a.clone(), b.clone()] : [];
  };

  // A polyline with cumulative segment lengths, so a fraction `t` can be resolved to a
  // point along it in constant time (see `along`). `total` is floored at 1 to keep
  // degenerate (zero-length) lines safe to divide by.
  const polyline = (pts) => {
    const cum = [0];
    for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + pts[i].distanceTo(pts[i - 1]));
    return { pts, cum, total: cum[cum.length - 1] || 1 };
  };

  // Layout relationships: faint structural lines along the parent pathway (overlap ok).
  for (const r of routes) {
    const pts = parentLine(r.path);
    if (pts.length < 2) continue;
    scene.add(
      new THREE.Line(
        new THREE.BufferGeometry().setFromPoints(pts),
        new THREE.LineBasicMaterial({ color: treeColor, transparent: true, opacity: 0.16 })
      )
    );
  }

  // Data flows: one filament per leg — a straight line between two nodes, shared by
  // every flow that crosses it. The line is neutral; the TRAFFIC carries the colour:
  // each flow on the leg streams beads in its own hue, so a leg used by five flows is
  // one line with five colours of dots. Click a leg to choose which flow to open.
  // Flow colours arrive precomputed from Rust (`fl.color`) — used directly.
  const flowDefs = data.flows ?? [];
  const filaments = [];
  const pickLines = [];
  const legByKey = new Map();
  const particles = [];
  const BEADS = 2; // beads per flow on a leg
  // 1. One straight filament per directed leg; record which flows run over it.
  for (const fl of flowDefs) {
    const color = new THREE.Color(fl.color);
    const name = fl.name || simpleName(fl.fqn);
    for (const h of fl.hops) {
      if (h.from === h.to || !pos.has(h.from) || !pos.has(h.to)) continue;
      const key = h.from + ">" + h.to;
      let fi = legByKey.get(key);
      if (fi === undefined) {
        const pts = straight(h.from, h.to); // straight line
        const mat = new THREE.LineBasicMaterial({ color: relColor, transparent: true, opacity: REL_OPACITY });
        const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), mat);
        line.userData = { fil: filaments.length };
        line.renderOrder = 1;
        scene.add(line);
        pickLines.push(line);
        fi = filaments.length;
        filaments.push({ from: h.from, to: h.to, legFlows: [], line, mat, ...polyline(pts) });
        legByKey.set(key, fi);
      }
      const leg = filaments[fi];
      if (!leg.legFlows.some((f) => f.fqn === fl.fqn)) leg.legFlows.push({ fqn: fl.fqn, name, hex: fl.color, color });
    }
  }
  // 2. Beads: per leg, every flow streams its own colour, phase-staggered so the hues
  //    interleave into one multi-coloured stream.
  for (let fi = 0; fi < filaments.length; fi++) {
    const fs = filaments[fi].legFlows,
      F = fs.length;
    for (let j = 0; j < F; j++) for (let i = 0; i < BEADS; i++) particles.push({ fil: fi, off: (i + j / F) / BEADS, color: fs[j].color });
  }
  // Does filament `f` touch node `id` — is `id` an endpoint or an ancestor (its
  // container / system) of one? Lets hovering a hub light every flow under it.
  const filUnder = (f, id) => anc(f.from).includes(id) || anc(f.to).includes(id);
  // One round dot per bead (no trail — cheaper, and reads cleanly on a busy graph).
  const REST_SPEED = 0.06; // one gentle constant bead speed along every filament
  let beadsOn = true; // resting beads are hidden while a flow is selected — skip their per-frame work
  const beadShown = new Uint8Array(particles.length).fill(1); // per-bead visibility; hidden beads are parked off-screen, not recoloured
  // Additive glow reads on the dark sheet but is invisible on a pale one (adding light
  // to near-white is a no-op) — so alpha-blend the beads as solid coloured dots there.
  const beadBlend = lightBg ? THREE.NormalBlending : THREE.AdditiveBlending;
  // Alpha-blended dots sit at full brightness on the pale sheet; darken them so they
  // read as crisp coloured beads, not pastel smudges. (Additive beads stay untouched.)
  const beadDim = lightBg ? 0.55 : 1;
  const flowArr = new Float32Array(particles.length * 3);
  const flowColArr = new Float32Array(particles.length * 3);
  for (let i = 0; i < particles.length; i++) {
    const c = particles[i].color,
      o = i * 3; // each bead carries its flow's colour
    flowColArr[o] = c.r * beadDim;
    flowColArr[o + 1] = c.g * beadDim;
    flowColArr[o + 2] = c.b * beadDim;
  }
  const flowGeo = new THREE.BufferGeometry();
  flowGeo.setAttribute("position", new THREE.BufferAttribute(flowArr, 3));
  flowGeo.setAttribute("color", new THREE.BufferAttribute(flowColArr, 3));
  // A round sprite so particles are dots, not squares (PointsMaterial's default). The
  // dark sheet wants a soft glow (additive); the pale sheet wants a near-solid disc —
  // a soft halo just reads as a pale smudge there, so hold full alpha almost to the rim.
  const dotCanvas = document.createElement("canvas");
  dotCanvas.width = dotCanvas.height = 64;
  const dctx = dotCanvas.getContext("2d");
  const grad = dctx.createRadialGradient(32, 32, 0, 32, 32, 32);
  grad.addColorStop(0, "rgba(255,255,255,1)");
  grad.addColorStop(lightBg ? 0.82 : 0.5, lightBg ? "rgba(255,255,255,1)" : "rgba(255,255,255,0.85)");
  grad.addColorStop(1, "rgba(255,255,255,0)");
  dctx.fillStyle = grad;
  dctx.beginPath();
  dctx.arc(32, 32, 32, 0, Math.PI * 2);
  dctx.fill();
  const dotTex = new THREE.CanvasTexture(dotCanvas);
  const flowMat = new THREE.PointsMaterial({
    vertexColors: true,
    map: dotTex,
    size: 2.2,
    sizeAttenuation: true,
    transparent: true,
    opacity: 0.95,
    blending: beadBlend,
    depthWrite: false,
  });
  const beadCloud = new THREE.Points(flowGeo, flowMat);
  beadCloud.frustumCulled = false; // beads park at NaN when filtered — a NaN bounding sphere breaks culling
  scene.add(beadCloud);
  // Position `out` at fraction `t` along a polyline (its points + cumulative lengths).
  const along = ({ pts, cum, total }, t, out) => {
    const d = t * total;
    let k = 0;
    while (k < cum.length - 2 && cum[k + 1] <= d) k++;
    const seg = cum[k + 1] - cum[k] || 1;
    out.copy(pts[k]).lerp(pts[k + 1], (d - cum[k]) / seg);
  };
  // Show only the beads on filaments matching `keep` (a selection); `null` shows all.
  // Hidden beads are parked at NaN — the GPU discards the point, so they vanish under
  // any blend mode (a recolour-to-transparent would smudge over orbs the bead overlaps).
  // `beadsOn` lets the frame loop skip moving beads no one can see; the loop only
  // repositions shown beads, so parked ones stay parked.
  const filterFlow = (keep) => {
    let any = false;
    for (let i = 0; i < particles.length; i++) {
      const on = !keep || keep(filaments[particles[i].fil]);
      if (on) any = true;
      beadShown[i] = on ? 1 : 0;
      if (!on) {
        const o = i * 3;
        flowArr[o] = flowArr[o + 1] = flowArr[o + 2] = NaN;
      }
    }
    beadsOn = any;
    flowGeo.attributes.position.needsUpdate = true;
  };

  // The flow request: you pick the step manually (the whole flow stays highlighted),
  // and traffic flows *continuously* along that one step's routed segment (caller →
  // … → callee through the gateways) until you advance.
  const STREAM_MAX = 128;
  const streamArr = new Float32Array(STREAM_MAX * 3);
  const streamGeo = new THREE.BufferGeometry();
  streamGeo.setAttribute("position", new THREE.BufferAttribute(streamArr, 3));
  streamGeo.setDrawRange(0, 0);
  const streamMat = new THREE.PointsMaterial({
    color: hotColor.clone().multiplyScalar(beadDim),
    map: dotTex,
    size: 3.6,
    sizeAttenuation: true,
    transparent: true,
    opacity: 0.95,
    blending: beadBlend,
    depthWrite: false,
  });
  const streamCloud = new THREE.Points(streamGeo, streamMat);
  streamCloud.frustumCulled = false; // drawRange starts at 0 and dots reposition every frame
  scene.add(streamCloud);
  const FLOW_SPEED = 55; // world units / second — one constant crossing speed
  let stepRoutes = [];
  let current = null;
  // The flow timeline's state: ordered steps + which one the request is on.
  let flowSteps = [];
  let flowStep = -1;
  let flowMeta = null; // { name, color } of the selected flow, for the timeline header
  // The "bus route": each leg of the selected flow drawn as its own bright line, so
  // the whole route is visible at once. The current leg is lit; the rest are a faint
  // route colour. Replaces the resting filaments while a flow is selected.
  let busSegs = [];
  let selColor = hotColor.clone(); // the selected flow's own colour
  const clearBus = () => {
    for (const b of busSegs) {
      scene.remove(b.line);
      b.line.geometry.dispose();
      b.mat.dispose();
    }
    busSegs = [];
  };
  const setFlowStream = (hops, color) => {
    clearBus();
    if (!hops || hops.length === 0) {
      // Clear the stream only; the per-edge filter is owned by focus/path selection.
      stepRoutes = [];
      current = null;
      streamGeo.setDrawRange(0, 0);
      flowSteps = [];
      flowStep = -1;
      flowMeta = null;
      renderTimeline();
      return;
    }
    selColor = color ? (typeof color === "string" ? new THREE.Color(color) : color) : hotColor.clone();
    streamMat.color.copy(selColor);
    // One direct arc per call step (caller → callee) — the real flow path.
    stepRoutes = hops.map((h) => polyline(straight(h.from, h.to))); // straight leg per hop
    // Draw every leg of the route (the bus line).
    for (const sr of stepRoutes) {
      if (sr.pts.length < 2) {
        busSegs.push({ line: new THREE.Line(), mat: new THREE.LineBasicMaterial() });
        continue;
      }
      const mat = new THREE.LineBasicMaterial({ color: relColor, transparent: true, opacity: 0.45 });
      const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(sr.pts), mat);
      line.renderOrder = 1;
      scene.add(line);
      busSegs.push({ line, mat });
    }
    filterFlow(() => false); // the stepping stream replaces the per-edge particles
    flowSteps = hops.map((h) => ({ from: simpleName(h.from), to: simpleName(h.to), label: h.label }));
    flowStep = 0;
    doStep(0); // stream the dots across step 0
    renderTimeline();
  };
  // Stream traffic continuously along step `i` (driven by the manual `flowStep`):
  // dots cross at a constant speed, one emitted from the origin ~per second. Also
  // re-colours the bus route so the current leg is lit and the rest are faint.
  const doStep = (i) => {
    for (let j = 0; j < busSegs.length; j++) {
      const b = busSegs[j];
      if (j === i) {
        b.mat.color.copy(selColor);
        b.mat.opacity = 0.95;
        b.line.renderOrder = 2;
      } else {
        b.mat.color.copy(selColor);
        b.mat.opacity = 0.28;
        b.line.renderOrder = 1;
      }
    }
    const sr = stepRoutes[i];
    if (!sr || sr.pts.length < 2) {
      current = null;
      streamGeo.setDrawRange(0, 0);
      return;
    }
    const travel = Math.max(0.6, sr.total / FLOW_SPEED); // seconds to cross the segment
    const count = Math.min(STREAM_MAX, Math.max(1, Math.round(travel))); // ~1 emission / sec
    current = { sr, count, travel };
    streamGeo.setDrawRange(0, count);
  };
  const setStep = (i) => {
    if (i < 0 || i >= flowSteps.length) return;
    flowStep = i;
    doStep(i);
    renderTimeline();
  };

  // The flow timeline panel (steps list + prev/next), rebuilt on every step change —
  // the lists are small, so a full rebuild is simpler than diffing.
  function renderTimeline() {
    timeline.textContent = "";
    if (!flowSteps.length) {
      timeline.hidden = true;
      return;
    }
    timeline.hidden = false;
    if (flowMeta) {
      const head = el("div", "uv-tl-flow", timeline);
      el("i", "", head).style.background = flowMeta.color;
      const nm = el("span", "", head);
      nm.textContent = flowMeta.name;
    }
    const hd = el("div", "uv-tl-head", timeline);
    const cnt = el("span", "", hd);
    cnt.textContent = `step ${Math.max(0, flowStep) + 1}/${flowSteps.length}`;
    const ctrls = el("span", "uv-tl-ctrls", hd);
    const prev = el("button", "", ctrls);
    prev.type = "button";
    prev.textContent = "‹";
    prev.disabled = flowStep <= 0;
    prev.setAttribute("aria-label", "Previous step");
    prev.addEventListener("click", () => setStep(Math.max(0, flowStep - 1)));
    const next = el("button", "", ctrls);
    next.type = "button";
    next.textContent = "›";
    next.disabled = flowStep >= flowSteps.length - 1;
    next.setAttribute("aria-label", "Next step");
    next.addEventListener("click", () => setStep(Math.min(flowSteps.length - 1, flowStep + 1)));
    const ol = el("ol", "", timeline);
    flowSteps.forEach((step, i) => {
      const li = el("li", i < flowStep ? "done" : i === flowStep ? "now" : "pending", ol);
      const row = el("button", "uv-tl-row", li);
      row.type = "button";
      const call = el("span", "uv-tl-call", row);
      call.textContent = `${step.from} → ${step.to}`;
      if (step.label) {
        const lb = el("span", "uv-tl-label", row);
        lb.textContent = step.label;
      }
      row.addEventListener("click", () => setStep(i));
    });
  }

  // Highlight every leg matching `match` (the hovered leg, or every leg through a
  // hovered hub); dim the rest. The line is neutral — its colour brightens, the beads
  // keep their per-flow hues. `null` resets to resting.
  const setHighlight = (match) => {
    for (const f of filaments) {
      const on = match ? match(f) : false;
      if (on) {
        f.mat.color.copy(hotColor);
        f.mat.opacity = 0.9;
        f.line.renderOrder = 1;
      } else if (match) {
        f.mat.color.copy(relColor);
        f.mat.opacity = 0.05;
        f.line.renderOrder = 0;
      } else {
        f.mat.color.copy(relColor);
        f.mat.opacity = REL_OPACITY;
        f.line.renderOrder = 0;
      }
    }
  };

  // Frame the whole graph — at mount, on re-center, and on a host resize
  // (skipped once the user has orbited, whose viewpoint a refit would yank).
  // Bounds come from the baked node positions, NOT the scene graph: the scene also
  // holds the huge ground grid and beads parked at NaN while a flow is selected —
  // a scene box chases those and parks the camera (or NaNs it) far off the content.
  const contentBox = new THREE.Box3();
  for (const p of pos.values()) contentBox.expandByPoint(p);
  if (contentBox.isEmpty()) contentBox.set(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1));
  contentBox.expandByScalar(14); // node radii + labels above
  const centre = contentBox.getCenter(new THREE.Vector3());
  const size = contentBox.getSize(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z, 1);
  let userMoved = false;
  const frameGraph = () => {
    controls.target.copy(centre);
    camera.position.copy(centre).add(new THREE.Vector3(size.x * 0.4, size.y * 0.3, maxDim * 1.5 + 40));
    controls.update();
    // Distance fog tinted to the backdrop: nearer orbs read crisp, far ones recede
    // into the sheet, adding aerial depth. Re-derived from the framed distance so a
    // refit keeps the content inside the fog's far plane.
    const camDist = camera.position.distanceTo(centre);
    scene.fog = new THREE.Fog(bgColor, camDist - maxDim * 0.5, camDist + maxDim * 1.3);
    userMoved = false;
  };
  controls.addEventListener("start", () => (userMoved = true));
  recenterBtn.addEventListener("click", frameGraph);
  frameGraph();

  // An *infinite* ground grid: a huge horizontal plane whose lines are drawn in
  // world space and fade with distance, so it reads as an endless floor (a
  // dimensional anchor) rather than a bounded card. It follows the camera in XZ.
  const gridBaseY = contentBox.min.y - size.y * 0.12 - 8;
  const gridFade = Math.max(size.x, size.y, size.z, 200) * 2.4;
  const gridMat = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    uniforms: {
      uColor: { value: relColor },
      uCam: { value: new THREE.Vector3() },
      uCell: { value: 22 },
      uMajor: { value: 110 },
      uFade: { value: gridFade },
      uOpacity: { value: 0.4 },
    },
    vertexShader: `
      varying vec3 vW;
      void main() { vec4 wp = modelMatrix * vec4(position, 1.0); vW = wp.xyz; gl_Position = projectionMatrix * viewMatrix * wp; }
    `,
    fragmentShader: `
      varying vec3 vW; uniform vec3 uColor; uniform vec3 uCam;
      uniform float uCell; uniform float uMajor; uniform float uFade; uniform float uOpacity;
      float lineAA(vec2 p, float cell) {
        vec2 c = p / cell; vec2 g = abs(fract(c - 0.5) - 0.5) / fwidth(c);
        return 1.0 - min(min(g.x, g.y), 1.0);
      }
      void main() {
        float a = max(lineAA(vW.xz, uCell) * 0.5, lineAA(vW.xz, uMajor));
        a *= 1.0 - clamp(distance(vW.xz, uCam.xz) / uFade, 0.0, 1.0);
        if (a <= 0.001) discard;
        gl_FragColor = vec4(uColor, a * uOpacity);
      }
    `,
  });
  const gridMesh = new THREE.Mesh(new THREE.PlaneGeometry(gridFade * 2.2, gridFade * 2.2), gridMat);
  gridMesh.rotation.x = -Math.PI / 2;
  gridMesh.position.set(centre.x, gridBaseY, centre.z);
  scene.add(gridMesh);

  // The persistent (selection-driven) highlight; hover overrides it transiently and
  // falls back to it when the pointer leaves a target.
  let resting = null;

  // Focus: fly to a node (keeping the current view direction) and light its chains.
  const FOCUS_DIST = { system: 120, person: 70, container: 60, component: 38 };
  let camFocus = null;
  const focusNode = (fqn) => {
    // Walk up the FQN to the nearest enclosing node that is placed (its component /
    // container) — mirrors the IDE, where a flow callable isn't a placed node.
    let id = fqn;
    while (id && !pos.has(id)) {
      const i = id.lastIndexOf("::");
      id = i < 0 ? "" : id.slice(0, i);
    }
    const p = id ? pos.get(id) : undefined;
    if (!p) return;
    camFocus = { target: p.clone(), dist: FOCUS_DIST[levelOf.get(id) ?? "component"] ?? 60 };
    resting = (f) => filUnder(f, id);
    setHighlight(resting);
    const nodeSet = new Set([id]);
    for (const a of anc(id)) if (pos.has(a)) nodeSet.add(a);
    paintNodes(nodeSet);
    filterFlow((f) => filUnder(f, id));
  };

  // Highlight a whole flow: light every relationship between two of its participant
  // nodes, frame them, and keep it lit (the resting highlight). `null` clears it.
  const focusPath = (path) => {
    const set = new Set((path ?? []).filter((id) => pos.has(id)));
    if (set.size === 0) {
      resting = null;
      setHighlight(null);
      paintNodes(null);
      filterFlow(null);
      return;
    }
    // The routed flow passes through each participant's gateways (its container,
    // system) — light those parent nodes too, while edges/flow still match the
    // participants themselves.
    const nodeSet = new Set(set);
    for (const id of set) for (const a of anc(id)) if (pos.has(a)) nodeSet.add(a);
    resting = () => false; // dim the chart; the bus route carries the highlight
    setHighlight(resting);
    paintNodes(nodeSet);
    filterFlow((f) => set.has(f.from) && set.has(f.to));
    const fbox = new THREE.Box3();
    for (const id of nodeSet) fbox.expandByPoint(pos.get(id));
    const c = fbox.getCenter(new THREE.Vector3()),
      s = fbox.getSize(new THREE.Vector3());
    camFocus = { target: c, dist: Math.max(70, Math.max(s.x, s.y, s.z) * 1.4 + 50) };
  };

  // ---- selection: node info card / flow timeline / chooser -----------------
  const showCard = (id) => {
    card.textContent = "";
    card.hidden = false;
    const level = levelOf.get(id) ?? "component";
    const kind = el("div", "uv-card-kind", card);
    el("i", "", kind).style.background = `var(--k-${level}, var(--ink-faint))`;
    kind.append(level);
    const name = el("div", "uv-card-name", card);
    name.textContent = simpleName(id);
    const fq = el("div", "uv-card-fqn", card);
    fq.textContent = id;
    const href = hrefOf.get(id);
    if (href) {
      // hrefs are site-root-relative; universe.html sits at the root, so as-is.
      const a = el("a", "uv-card-link", card);
      a.href = href;
      a.textContent = "View documentation →";
    }
  };
  const hideCard = () => {
    card.hidden = true;
    card.textContent = "";
  };

  // The flow chooser: anchored where you clicked a leg carrying several flows.
  let choice = null;
  const closeChoice = () => {
    if (choice) choice.remove();
    choice = null;
  };
  const openChoice = (x, y, items) => {
    closeChoice();
    choice = el("div", "uv-choice");
    choice.style.left = `${x}px`;
    choice.style.top = `${y}px`;
    const head = el("div", "uv-fc-head", choice);
    head.textContent = "Open flow";
    for (const f of items) {
      const b = el("button", "", choice);
      b.type = "button";
      el("i", "", b).style.background = f.hex;
      b.append(f.name);
      b.addEventListener("click", () => {
        closeChoice();
        selectFlow(f.fqn);
      });
    }
  };

  const selectNode = (id) => {
    closeChoice();
    setFlowStream(null);
    focusNode(id);
    showCard(id);
  };
  const selectFlow = (fqn) => {
    const fl = flowDefs.find((f) => f.fqn === fqn);
    if (!fl) return;
    hideCard();
    // Participants in hop order (deduped) — frames + lights the whole chain; the
    // bus route + stepping stream then override the resting filaments.
    const parts = [];
    for (const h of fl.hops) for (const id of [h.from, h.to]) if (!parts.includes(id)) parts.push(id);
    focusPath(parts);
    flowMeta = { name: fl.name || simpleName(fl.fqn), color: fl.color };
    setFlowStream(fl.hops, fl.color);
  };
  // Clear any selection back to the resting chart.
  const clearSelection = () => {
    focusPath(null);
    setFlowStream(null);
    hideCard();
    closeChoice();
  };

  // ---- hover: trace the chain ---------------------------------------------
  const ray = new THREE.Raycaster();
  ray.params.Line = { threshold: 2.4 };
  const ndc = new THREE.Vector2();
  const showTip = (x, y, html) => {
    const r = cv.getBoundingClientRect();
    tip.style.left = `${x - r.left}px`;
    tip.style.top = `${y - r.top}px`;
    tip.innerHTML = html;
    tip.style.opacity = "1";
  };
  const onMove = (e) => {
    const r = cv.getBoundingClientRect();
    ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
    ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
    ray.setFromCamera(ndc, camera);
    // A hub (node) first: light every leg through it (itself or a descendant).
    const nodeHit = ray.intersectObject(nodes3d, false)[0];
    if (nodeHit && nodeHit.instanceId != null) {
      const n = placed[nodeHit.instanceId];
      setHighlight((f) => filUnder(f, n.id));
      const through = new Set(filaments.filter((f) => filUnder(f, n.id)).flatMap((f) => f.legFlows.map((x) => x.fqn))).size;
      showTip(e.clientX, e.clientY, `<b>${simpleName(n.id)}</b><em>${n.level}${through ? ` &middot; ${through} flow${through === 1 ? "" : "s"}` : ""}</em>`);
      cv.style.cursor = "pointer";
      return;
    }
    // A leg: light it and say how many flows it carries (click to choose / open).
    const lineHit = ray.intersectObjects(pickLines, false)[0];
    if (lineHit) {
      const fil = filaments[lineHit.object.userData.fil];
      setHighlight((f) => f === fil);
      const n = fil.legFlows.length;
      showTip(e.clientX, e.clientY, `<b>${simpleName(fil.from)} → ${simpleName(fil.to)}</b><em>${n} flow${n === 1 ? "" : "s"} &middot; click to ${n === 1 ? "open" : "choose"}</em>`);
      cv.style.cursor = "pointer";
      return;
    }
    setHighlight(resting);
    tip.style.opacity = "0";
    cv.style.cursor = "grab";
  };
  cv.addEventListener("pointermove", onMove);

  // A click (not a drag): on a node → focus it and open its info card; on a leg →
  // open its flow, or offer a choice when several flows share the leg; on empty
  // space → dismiss any open chooser, else clear the chart.
  let down = { x: 0, y: 0 };
  const onDown = (e) => {
    down = { x: e.clientX, y: e.clientY };
  };
  const onUp = (e) => {
    if (e.button !== 0) return;
    if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5) return; // a drag (orbit/pan)
    const r = cv.getBoundingClientRect();
    ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
    ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
    ray.setFromCamera(ndc, camera);
    const nodeHit = ray.intersectObject(nodes3d, false)[0];
    if (nodeHit && nodeHit.instanceId != null) {
      selectNode(placed[nodeHit.instanceId].id);
      return;
    }
    const lineHit = ray.intersectObjects(pickLines, false)[0];
    if (lineHit) {
      const fil = filaments[lineHit.object.userData.fil];
      if (fil.legFlows.length === 1) selectFlow(fil.legFlows[0].fqn);
      else openChoice(e.clientX - r.left, e.clientY - r.top, fil.legFlows);
      return;
    }
    if (choice) {
      closeChoice(); // a click-away dismisses the chooser
      return;
    }
    clearSelection();
  };
  cv.addEventListener("pointerdown", onDown);
  cv.addEventListener("pointerup", onUp);

  const resize = () => {
    camera.aspect = W() / H();
    camera.updateProjectionMatrix();
    renderer.setSize(W(), H());
    // Keep the graph framed through window and panel resizes; an orbiting
    // user's viewpoint is left alone.
    if (!userMoved) frameGraph();
  };
  addEventListener("resize", resize);
  // A window listener misses host (layout) resizes — observe the host too.
  const ro = new ResizeObserver(resize);
  ro.observe(host);
  const onKey = (e) => {
    if (e.key !== "Escape") return;
    // An open chooser owns the first Escape: dismiss it without touching the
    // selection. Esc only deselects — it never closes the page's chrome.
    if (choice) {
      closeChoice();
      return;
    }
    clearSelection();
  };
  addEventListener("keydown", onKey);

  let raf = 0,
    alive = true,
    last = performance.now(),
    flowT = 0;
  const flowTmp = new THREE.Vector3();
  const frame = () => {
    if (!alive) return;
    const now = performance.now(),
      dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    if (camFocus) {
      controls.target.lerp(camFocus.target, 0.12);
      const offset = camera.position.clone().sub(controls.target);
      if (offset.lengthSq() < 1e-6) offset.set(0, 0, 1);
      camera.position.copy(controls.target).add(offset.setLength(camFocus.dist));
      if (controls.target.distanceTo(camFocus.target) < 0.4) camFocus = null;
    }
    // Stream the traffic beads along their legs (caller → callee), one dot each. Skip
    // it entirely while a flow is selected (resting beads are hidden then).
    flowT += dt;
    if (beadsOn) {
      for (let i = 0; i < particles.length; i++) {
        if (!beadShown[i]) continue; // parked at NaN by filterFlow — leave it hidden
        const p = particles[i];
        let t = (flowT * REST_SPEED + p.off) % 1;
        if (t < 0) t += 1;
        along(filaments[p.fil], t, flowTmp);
        const o = i * 3;
        flowArr[o] = flowTmp.x;
        flowArr[o + 1] = flowTmp.y;
        flowArr[o + 2] = flowTmp.z;
      }
      flowGeo.attributes.position.needsUpdate = true;
    }
    flowMat.opacity = 0.7 + 0.25 * Math.sin(flowT * 2.2); // gentle volume-agnostic pulse
    // Traffic flows continuously along the current step's routed segment (manual
    // control via the timeline): a spaced string of dots looping caller → callee.
    if (current) {
      const { sr, count, travel } = current;
      for (let i = 0; i < count; i++) {
        const t = (flowT / travel + i / count) % 1; // constant speed; evenly spaced
        along(sr, t, flowTmp);
        streamArr[i * 3] = flowTmp.x;
        streamArr[i * 3 + 1] = flowTmp.y;
        streamArr[i * 3 + 2] = flowTmp.z;
      }
      streamGeo.attributes.position.needsUpdate = true;
    }
    // Keep the infinite grid centred under the camera; its lines are world-space.
    gridMat.uniforms.uCam.value.copy(camera.position);
    gridMesh.position.set(controls.target.x, gridBaseY, controls.target.z);
    controls.update();
    renderer.render(scene, camera);
    raf = requestAnimationFrame(frame);
  };
  frame();

  return () => {
    alive = false;
    cancelAnimationFrame(raf);
    cv.removeEventListener("pointermove", onMove);
    cv.removeEventListener("pointerdown", onDown);
    cv.removeEventListener("pointerup", onUp);
    removeEventListener("resize", resize);
    removeEventListener("keydown", onKey);
    ro.disconnect();
    // Free GPU resources — renderer.dispose() doesn't reclaim user geometries /
    // materials / textures, and the island remounts on theme change. Collect each
    // resource ONCE (a texture shared by two materials, e.g. the bead sprite, must
    // not be disposed twice), then free. Sets dedupe; the InstancedMesh's instance
    // buffers need its own dispose(); live bus-route segments are freed first.
    clearBus();
    const geos = new Set();
    const mats = new Set();
    const texs = new Set();
    scene.traverse((o) => {
      if (o.geometry) geos.add(o.geometry);
      const m = o.material;
      if (m)
        for (const mm of Array.isArray(m) ? m : [m]) {
          mats.add(mm);
          if (mm.map) texs.add(mm.map);
        }
    });
    texs.forEach((t) => t.dispose());
    mats.forEach((m) => m.dispose());
    geos.forEach((g) => g.dispose());
    nodes3d.dispose(); // frees instanceMatrix + instanceColor
    controls.dispose();
    renderer.dispose();
    host.textContent = "";
  };
}
