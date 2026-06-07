<script lang="ts">
  // A 3D force-directed graph of the workspace, clustered by C4 containment and with
  // relationships *routed through their gateways*. A relationship between two
  // components is not a straight line between them — it travels up through each
  // component's container and system (the gateway hubs) and across at the level where
  // the two sides meet: component → container → system → … → system → container →
  // component. Hovering a relationship (or a hub) lights the whole end-to-end chain.
  //
  // Layout is d3-force-3d (containment links cluster; gateway "bridge" links pull
  // related hubs together); render is Three.js + OrbitControls in the brand theme.
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
  import { forceSimulation, forceManyBody, forceLink, forceCenter, forceCollide, forceX, forceY, forceZ } from "d3-force-3d";

  type Node = { id: string; level: string; parent: string | null };
  type Edge = { from: string; to: string; traffic: number; kind: string };
  type Snapshot = { nodes: Node[]; edges: Edge[] };

  // Traffic flows take a colour keyed by their *destination node*, from a varied
  // categorical palette — so flows converging on a node share a hue while the graph
  // as a whole reads in many colours (archetype alone clusters to one "service" green).
  const FLOW_PALETTE = [
    "#ff6b6b", "#ffa94d", "#ffd43b", "#a9e34b", "#69db7c", "#38d9a9", "#3bc9db",
    "#4dabf7", "#748ffc", "#9775fa", "#da77f2", "#f783ac", "#ff8787", "#ffc078",
    "#94d82d", "#20c997",
  ];
  // FNV-1a (32-bit) over a node id → a stable palette index.
  const flowHash = (s: string) => {
    let h = 0x811c9dc5;
    for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193); }
    return h >>> 0;
  };
  // `onclose` present → rendered as a full-screen overlay (show a close button);
  // absent → embedded as a panel view (the activity bar switches away from it).
  // `focusFqn` → fly the camera to that node and light its relationships.
  // `highlightPath` → an ordered list of a flow's participant nodes: light the whole
  // chain between them and frame it (takes precedence over `focusFqn`).
  // One call in a flow: caller → callee node ids, plus the message label.
  type FlowHop = { from: string; to: string; label: string };
  type Props = {
    snapshot: Snapshot;
    onclose?: (() => void) | null;
    focusFqn?: string | null;
    highlightPath?: string[] | null;
    flowSequence?: FlowHop[] | null;
    flowColor?: string | null;
    ondeselect?: (() => void) | null;
  };
  let { snapshot, onclose = null, focusFqn = null, highlightPath = null, flowSequence = null, flowColor = null, ondeselect = null }: Props = $props();

  // An in-canvas relationship selection (clicking traffic), separate from the prop-
  // driven flow selection. Set by onMount's handlers.
  let localSelected = $state(false);
  let clearLocal: (() => void) | null = null;
  // Whether a selection is active (drives the Deselect button + background-click reset).
  const hasSelection = $derived(!!focusFqn || !!(highlightPath?.length) || !!(flowSequence?.length) || localSelected);

  let canvas = $state<HTMLCanvasElement | null>(null);
  let tipEl = $state<HTMLDivElement | null>(null);
  // The selected flow's ordered steps + which one the request is on, for the timeline.
  let flowSteps = $state<{ from: string; to: string; label: string }[]>([]);
  let flowStep = $state(-1);

  // Set by onMount once the scene exists; the effect drives focus/highlight on change.
  let applyFocus: ((fqn: string) => void) | null = null;
  let applyPath: ((path: string[] | null) => void) | null = null;
  let applyFlowSeq: ((hops: FlowHop[] | null, color?: string | null) => void) | null = null;
  let popToStep: ((i: number) => void) | null = null;
  $effect(() => {
    // Read every prop up front so all are tracked — a `&&` on the non-reactive
    // `apply*` handles would short-circuit before the prop is read, and the effect
    // would never re-run when the selection changes.
    const path = highlightPath, fqn = focusFqn, seq = flowSequence, col = flowColor;
    if (!applyPath) return; // not mounted yet
    if (path) applyPath(path);
    else if (fqn) applyFocus?.(fqn);
    else applyPath(null); // nothing selected → reset highlight / dim / filter
    applyFlowSeq?.(seq ?? null, col); // after focus/path so it can override the flow particles
  });
  // Manual stepping: pop the request across the selected step.
  $effect(() => { const s = flowStep; if (popToStep && s >= 0) popToStep(s); });

  const SIZE: Record<string, number> = { system: 6, container: 3.4, component: 1.9, person: 3, data: 2.2 };
  const REL_OPACITY = 0.3;
  const shortName = (id: string) => id.split("::").pop() ?? id;

  onMount(() => {
    const cv = canvas!, tip = tipEl!;
    const W = () => cv.clientWidth, H = () => cv.clientHeight;

    // Brand palette, read live off the active theme (so dark/light + accent track).
    const css = getComputedStyle(document.documentElement);
    const cssColor = (name: string, fallback: string) => new THREE.Color(css.getPropertyValue(name).trim() || fallback);
    const COLOR: Record<string, THREE.Color> = {
      system: cssColor("--k-system", "#ff6a52"),
      container: cssColor("--k-container", "#2dd4bf"),
      component: cssColor("--k-component", "#e0a93f"),
      person: cssColor("--k-person", "#6e8bff"),
    };
    const fallbackColor = cssColor("--ink-soft", "#aeb0ba");
    const bgColor = cssColor("--island-bg", "#15161c");
    const treeColor = cssColor("--ink-faint", "#71747f");
    const relColor = cssColor("--ink-soft", "#aeb0ba");
    const hotColor = cssColor("--accent", "#ff5a36");

    // ---- containment + routing ----------------------------------------------
    const ids = new Set(snapshot.nodes.map((n) => n.id));
    const levelOf = new Map(snapshot.nodes.map((n) => [n.id, n.level]));
    const parentOf = new Map(snapshot.nodes.map((n) => [n.id, n.parent]));
    const ancestors = (id: string) => {
      const chain = [id];
      let p = parentOf.get(id) ?? null;
      while (p && ids.has(p)) { chain.push(p); p = parentOf.get(p) ?? null; }
      return chain; // [self, …, root]
    };
    // The gateway route from `a` to `b`: up each side to the lowest common ancestor
    // (or, across systems, joining the two roots). `bridge` is the single segment that
    // straddles the meeting point — the pair the relationship force should pull together.
    const routeOf = (a: string, b: string): { path: string[]; bridge: [string, string] | null } => {
      const A = ancestors(a), B = ancestors(b);
      const bIdx = new Map(B.map((id, i) => [id, i] as const));
      const i = A.findIndex((id) => bIdx.has(id));
      if (i >= 0) {
        const j = bIdx.get(A[i])!;
        const path = [...A.slice(0, i + 1), ...B.slice(0, j).reverse()];
        const bridge: [string, string] | null = i - 1 >= 0 && i + 1 < path.length ? [path[i - 1], path[i + 1]] : null;
        return { path, bridge };
      }
      // disjoint roots (different systems): join the two roots directly.
      return { path: [...A, ...B.slice().reverse()], bridge: [A[A.length - 1], B[B.length - 1]] };
    };

    const routes = snapshot.edges
      .filter((e) => e.from !== e.to && ids.has(e.from) && ids.has(e.to))
      .map((e) => ({ from: e.from, to: e.to, traffic: e.traffic, kind: e.kind, ...routeOf(e.from, e.to) }));

    // ---- d3-force-3d layout --------------------------------------------------
    const nodes = snapshot.nodes.map((n) => ({ id: n.id, level: n.level }));
    // Containment links cluster: short + strong for container→component, wider for
    // system→container so component balls don't collide.
    const contain = snapshot.nodes
      .filter((n) => n.parent && ids.has(n.parent))
      .map((n) => ({ source: n.parent as string, target: n.id, dist: levelOf.get(n.parent as string) === "system" ? 60 : 16 }));
    // Relationships pull at their *gateway bridge*, not component-to-component — so
    // related systems/containers drift together while their internals stay clustered.
    const relForce = routes
      .filter((r) => r.bridge && r.bridge[0] !== r.bridge[1])
      .map((r) => {
        const [u, v] = r.bridge as [string, string];
        const lv = (x: string) => levelOf.get(x);
        const dist = lv(u) === "system" || lv(v) === "system" ? 110 : lv(u) === "container" || lv(v) === "container" ? 45 : 25;
        return { source: u, target: v, dist };
      });
    const sim = forceSimulation(nodes, 3)
      .force("charge", forceManyBody().strength(-55))
      .force("contain", forceLink(contain).id((d: { id: string }) => d.id).distance((l: { dist: number }) => l.dist).strength(0.9))
      .force("link", forceLink(relForce).id((d: { id: string }) => d.id).distance((l: { dist: number }) => l.dist).strength(0.12))
      .force("collide", forceCollide((d: { level: string }) => (SIZE[d.level] ?? 2) + 1.5))
      .force("x", forceX().strength(0.04))
      .force("y", forceY().strength(0.04))
      .force("z", forceZ().strength(0.04))
      .force("center", forceCenter())
      .stop();
    for (let i = 0; i < 400; i++) sim.tick();
    const pos = new Map<string, THREE.Vector3>(
      (nodes as unknown as { id: string; x: number; y: number; z: number }[]).map((n) => [n.id, new THREE.Vector3(n.x, n.y, n.z)]),
    );

    // ---- Three.js render ----------------------------------------------------
    const renderer = new THREE.WebGLRenderer({ canvas: cv, antialias: true });
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.setSize(W(), H());
    const scene = new THREE.Scene();
    scene.background = bgColor;
    const camera = new THREE.PerspectiveCamera(55, W() / H(), 0.1, 8000);
    const controls = new OrbitControls(camera, cv);
    controls.enableDamping = true;
    scene.add(new THREE.AmbientLight(0xffffff, 0.75));
    const keyLight = new THREE.DirectionalLight(0xffffff, 0.85); keyLight.position.set(1, 1, 1); scene.add(keyLight);

    // Nodes.
    const sphere = new THREE.SphereGeometry(1, 20, 20);
    const pickNodes: THREE.Mesh[] = [];
    const nodeMats: { id: string; mat: THREE.MeshStandardMaterial; base: THREE.Color }[] = [];
    for (const n of snapshot.nodes) {
      const p = pos.get(n.id); if (!p) continue;
      const base = COLOR[n.level] ?? fallbackColor;
      const mat = new THREE.MeshStandardMaterial({ color: base, roughness: 0.5, metalness: 0.1, transparent: true });
      const mesh = new THREE.Mesh(sphere, mat);
      mesh.position.copy(p); mesh.scale.setScalar(SIZE[n.level] ?? 2.2); (mesh as THREE.Object3D).userData = n;
      scene.add(mesh); pickNodes.push(mesh); nodeMats.push({ id: n.id, mat, base });
    }
    // Brighten the nodes in `set`, dim the rest; `null` restores all.
    const paintNodes = (set: Set<string> | null) => {
      for (const e of nodeMats) {
        const on = !set || set.has(e.id);
        e.mat.opacity = set && !on ? 0.12 : 1;
        e.mat.emissive.copy(set && on ? e.base : new THREE.Color(0x000000));
        e.mat.emissiveIntensity = set && on ? 0.6 : 0;
      }
    };

    // Labels on every node, sized relative to the node — big suns read large, small
    // components small. Billboarded sprites; `h` is the world height of the text.
    const inkHex = css.getPropertyValue("--ink").trim() || "#f4f4f6";
    const makeLabel = (text: string, h: number) => {
      const dpr = 2, font = 30;
      const c = document.createElement("canvas");
      const ctx = c.getContext("2d")!;
      ctx.font = `600 ${font}px ui-sans-serif, system-ui, sans-serif`;
      const tw = Math.ceil(ctx.measureText(text).width);
      c.width = (tw + 14) * dpr; c.height = (font + 12) * dpr;
      ctx.scale(dpr, dpr);
      ctx.font = `600 ${font}px ui-sans-serif, system-ui, sans-serif`;
      ctx.textBaseline = "middle"; ctx.fillStyle = inkHex;
      ctx.fillText(text, 7, (font + 12) / 2);
      const tex = new THREE.CanvasTexture(c); tex.minFilter = THREE.LinearFilter;
      const spr = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false }));
      spr.scale.set(h * (c.width / c.height), h, 1);
      return spr;
    };
    for (const n of snapshot.nodes) {
      const p = pos.get(n.id); if (!p) continue;
      const size = SIZE[n.level] ?? 2.2;
      const h = Math.max(1.8, size * 0.85); // label height tracks node size
      const spr = makeLabel(shortName(n.id), h);
      spr.position.copy(p).add(new THREE.Vector3(0, size + h * 0.7, 0));
      scene.add(spr);
    }

    // The faint containment tree, so structure reads even where there are no relationships.
    const treePos: number[] = [];
    for (const c of contain) {
      const a = pos.get(c.source), b = pos.get(c.target); if (!a || !b) continue;
      treePos.push(a.x, a.y, a.z, b.x, b.y, b.z);
    }
    const treeGeo = new THREE.BufferGeometry();
    treeGeo.setAttribute("position", new THREE.Float32BufferAttribute(treePos, 3));
    scene.add(new THREE.LineSegments(treeGeo, new THREE.LineBasicMaterial({ color: treeColor, transparent: true, opacity: 0.14 })));

    // ---- layout relationships vs. data flows -------------------------------
    // Two DIFFERENT things, drawn separately:
    //  • LAYOUT relationships — the structural edges routed up through the containment
    //    *parent pathway*. Pure layout: plain, faint, overlap is fine.
    //  • DATA FLOWS — the actual flows running directly between two spheres (not routed
    //    through the parent), each its own colour, with beads of light tracking along.
    const ARC_SAMPLES = 22;
    const UP = new THREE.Vector3(0, 1, 0), ALT = new THREE.Vector3(1, 0, 0);
    const ftan = new THREE.Vector3(), frt = new THREE.Vector3(), fup = new THREE.Vector3();
    // The parent-pathway polyline (gateway stops). Plain — no offset, overlap allowed.
    const parentLine = (ids: string[]): THREE.Vector3[] =>
      ids.map((id) => pos.get(id)).filter((v): v is THREE.Vector3 => !!v);
    // A data-flow filament: a direct, gently-bowed arc straight from source to dest.
    const dataArc = (fromId: string, toId: string): THREE.Vector3[] => {
      const a = pos.get(fromId), b = pos.get(toId);
      if (!a || !b) return [];
      ftan.subVectors(b, a); const len = ftan.length();
      if (len < 1e-3) return [];
      ftan.normalize();
      frt.crossVectors(ftan, Math.abs(ftan.y) > 0.9 ? ALT : UP).normalize();
      fup.crossVectors(frt, ftan).normalize();
      const bow = Math.min(len * 0.12, 14); // gentle arc so flows between the same pair don't sit dead-straight
      const pts: THREE.Vector3[] = [];
      for (let k = 0; k <= ARC_SAMPLES; k++) {
        const t = k / ARC_SAMPLES;
        pts.push(new THREE.Vector3().lerpVectors(a, b, t).addScaledVector(fup, Math.sin(Math.PI * t) * bow));
      }
      return pts;
    };
    // Every flow (relationship) its own colour, keyed by the directed pair.
    const relColorOf = (from: string, to: string) => new THREE.Color(FLOW_PALETTE[flowHash(`${from}>${to}`) % FLOW_PALETTE.length]);

    // Layout relationships: faint structural lines along the parent pathway (overlap ok).
    for (const r of routes) {
      const pts = parentLine(r.path);
      if (pts.length < 2) continue;
      scene.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts),
        new THREE.LineBasicMaterial({ color: treeColor, transparent: true, opacity: 0.16 })));
    }

    // Data flows: the direct filament you hover/click to trace, with beads of light.
    type Rel = { line: THREE.Line; mat: THREE.LineBasicMaterial; base: THREE.Color; from: string; to: string };
    const rels: Rel[] = [];
    const pickLines: THREE.Line[] = [];
    type FlowRoute = { from: string; to: string; pts: THREE.Vector3[]; cum: number[]; total: number; speed: number; color: THREE.Color };
    const flowRoutes: FlowRoute[] = [];
    const particles: { route: number; off: number }[] = [];
    for (const r of routes) {
      const pts = dataArc(r.from, r.to);
      if (pts.length < 2) continue;
      const color = relColorOf(r.from, r.to);
      const mat = new THREE.LineBasicMaterial({ color: color.clone(), transparent: true, opacity: REL_OPACITY });
      const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), mat);
      (line as THREE.Object3D).userData = { rel: rels.length }; line.renderOrder = 1;
      scene.add(line); pickLines.push(line);
      rels.push({ line, mat, base: color.clone(), from: r.from, to: r.to });
      const cum = [0];
      for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + pts[i].distanceTo(pts[i - 1]));
      flowRoutes.push({ from: r.from, to: r.to, pts, cum, total: cum[cum.length - 1] || 1, speed: 0.02 + Math.min(r.traffic, 12) * 0.008, color });
      const count = Math.min(2 + r.traffic, 10);
      for (let i = 0; i < count; i++) particles.push({ route: flowRoutes.length - 1, off: i / count });
    }
    // Does relationship `rl` belong to node `id` — is `id` an endpoint or an ancestor
    // (container/system) of one? Lets hovering a hub light all relationships under it.
    const relUnder = (rl: Rel, id: string) => ancestors(rl.from).includes(id) || ancestors(rl.to).includes(id);
    // Each bead is a comet: a bright lead point + a few fading tail points behind it.
    const TRAIL = 6, TRAIL_GAP = 0.012; // tail length (points) and phase spacing
    const fade = (k: number) => (1 - k / TRAIL) ** 1.5;
    const flowArr = new Float32Array(particles.length * TRAIL * 3);
    const flowColArr = new Float32Array(particles.length * TRAIL * 3);
    for (let i = 0; i < particles.length; i++) {
      const c = flowRoutes[particles[i].route].color;
      for (let k = 0; k < TRAIL; k++) {
        const o = (i * TRAIL + k) * 3, f = fade(k);
        flowColArr[o] = c.r * f; flowColArr[o + 1] = c.g * f; flowColArr[o + 2] = c.b * f;
      }
    }
    const flowGeo = new THREE.BufferGeometry();
    flowGeo.setAttribute("position", new THREE.BufferAttribute(flowArr, 3));
    flowGeo.setAttribute("color", new THREE.BufferAttribute(flowColArr, 3));
    // A soft round sprite so particles are dots, not squares (PointsMaterial's default).
    const dotCanvas = document.createElement("canvas");
    dotCanvas.width = dotCanvas.height = 64;
    const dctx = dotCanvas.getContext("2d")!;
    const grad = dctx.createRadialGradient(32, 32, 0, 32, 32, 32);
    grad.addColorStop(0, "rgba(255,255,255,1)");
    grad.addColorStop(0.5, "rgba(255,255,255,0.85)");
    grad.addColorStop(1, "rgba(255,255,255,0)");
    dctx.fillStyle = grad; dctx.beginPath(); dctx.arc(32, 32, 32, 0, Math.PI * 2); dctx.fill();
    const dotTex = new THREE.CanvasTexture(dotCanvas);
    const flowMat = new THREE.PointsMaterial({ vertexColors: true, map: dotTex, size: 2.2, sizeAttenuation: true, transparent: true, opacity: 0.95, blending: THREE.AdditiveBlending, depthWrite: false });
    scene.add(new THREE.Points(flowGeo, flowMat));
    // Position a particle at fraction `t` along its route's polyline.
    const along = (fr: FlowRoute, t: number, out: THREE.Vector3) => {
      const d = t * fr.total;
      let k = 0; while (k < fr.cum.length - 2 && fr.cum[k + 1] <= d) k++;
      const seg = fr.cum[k + 1] - fr.cum[k] || 1;
      out.copy(fr.pts[k]).lerp(fr.pts[k + 1], (d - fr.cum[k]) / seg);
    };
    // Show only the traffic on routes matching `keep` (a selection); `null` shows all.
    // Hidden particles get colour (0,0,0) — invisible under additive blending.
    const filterFlow = (keep: ((fr: FlowRoute) => boolean) | null) => {
      for (let i = 0; i < particles.length; i++) {
        const fr = flowRoutes[particles[i].route];
        const on = !keep || keep(fr);
        for (let k = 0; k < TRAIL; k++) {
          const o = (i * TRAIL + k) * 3, f = on ? fade(k) : 0;
          flowColArr[o] = fr.color.r * f; flowColArr[o + 1] = fr.color.g * f; flowColArr[o + 2] = fr.color.b * f;
        }
      }
      flowGeo.attributes.color.needsUpdate = true;
    };

    // A position at fraction `t` along a polyline (pts + cumulative lengths).
    const alongPoly = (pts: THREE.Vector3[], cum: number[], total: number, t: number, out: THREE.Vector3) => {
      const d = t * total;
      let k = 0; while (k < cum.length - 2 && cum[k + 1] <= d) k++;
      const seg = cum[k + 1] - cum[k] || 1;
      out.copy(pts[k]).lerp(pts[k + 1], (d - cum[k]) / seg);
    };

    // The flow request: you pick the step manually (the whole flow stays highlighted),
    // and traffic flows *continuously* along that one step's routed segment (caller →
    // … → callee through the gateways) until you advance.
    const STREAM_MAX = 128;
    const streamArr = new Float32Array(STREAM_MAX * 3);
    const streamGeo = new THREE.BufferGeometry();
    streamGeo.setAttribute("position", new THREE.BufferAttribute(streamArr, 3));
    streamGeo.setDrawRange(0, 0);
    const streamMat = new THREE.PointsMaterial({ color: hotColor, map: dotTex, size: 3.6, sizeAttenuation: true, transparent: true, opacity: 0.95, blending: THREE.AdditiveBlending, depthWrite: false });
    scene.add(new THREE.Points(streamGeo, streamMat));
    const FLOW_SPEED = 55; // world units / second — one constant crossing speed
    type StepRoute = { pts: THREE.Vector3[]; cum: number[]; total: number };
    let stepRoutes: StepRoute[] = [];
    let current: { sr: StepRoute; count: number; travel: number } | null = null;
    // The "bus route": each leg of the selected flow drawn as its own bright line, so
    // the whole route is visible at once. The current leg is lit; the rest are a faint
    // route colour. Replaces the resting filaments while a flow is selected.
    let busSegs: { line: THREE.Line; mat: THREE.LineBasicMaterial }[] = [];
    let flowColor = hotColor.clone(); // the selected flow's own colour
    const clearBus = () => {
      for (const b of busSegs) { scene.remove(b.line); b.line.geometry.dispose(); b.mat.dispose(); }
      busSegs = [];
    };
    const setFlowStream = (hops: FlowHop[] | null, color?: string | THREE.Color | null) => {
      clearBus();
      if (!hops || hops.length === 0) {
        // Clear the stream only; the per-edge filter is owned by focus/path selection.
        stepRoutes = []; current = null; streamGeo.setDrawRange(0, 0); flowSteps = []; flowStep = -1;
        return;
      }
      flowColor = color ? (typeof color === "string" ? new THREE.Color(color) : color) : relColorOf(hops[0].from, hops[hops.length - 1].to); // every flow its own colour
      streamMat.color.copy(flowColor);
      // One direct arc per call step (caller → callee) — the real flow path.
      stepRoutes = hops.map((h) => {
        const pts = dataArc(h.from, h.to); // direct data-flow arc
        const cum = [0];
        for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + pts[i].distanceTo(pts[i - 1]));
        return { pts, cum, total: cum[cum.length - 1] || 1 };
      });
      // Draw every leg of the route (the bus line).
      for (const sr of stepRoutes) {
        if (sr.pts.length < 2) { busSegs.push({ line: new THREE.Line(), mat: new THREE.LineBasicMaterial() }); continue; }
        const mat = new THREE.LineBasicMaterial({ color: relColor, transparent: true, opacity: 0.45 });
        const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(sr.pts), mat);
        line.renderOrder = 1; scene.add(line); busSegs.push({ line, mat });
      }
      filterFlow(() => false); // the stepping stream replaces the per-edge particles
      flowSteps = hops.map((h) => ({ from: shortName(h.from), to: shortName(h.to), label: h.label }));
      flowStep = 0; // the $effect on flowStep streams the dots across step 0
    };
    // Stream traffic continuously along step `i` (driven by the manual `flowStep`):
    // dots cross at a constant speed, one emitted from the origin ~per second. Also
    // re-colours the bus route so the current leg is lit and the rest are faint.
    const doStep = (i: number) => {
      for (let j = 0; j < busSegs.length; j++) {
        const b = busSegs[j];
        if (j === i) { b.mat.color.copy(flowColor); b.mat.opacity = 0.95; b.line.renderOrder = 2; }
        else { b.mat.color.copy(flowColor); b.mat.opacity = 0.28; b.line.renderOrder = 1; }
      }
      const sr = stepRoutes[i];
      if (!sr || sr.pts.length < 2) { current = null; streamGeo.setDrawRange(0, 0); return; }
      const travel = Math.max(0.6, sr.total / FLOW_SPEED); // seconds to cross the segment
      const count = Math.min(STREAM_MAX, Math.max(1, Math.round(travel))); // ~1 emission / sec
      current = { sr, count, travel };
      streamGeo.setDrawRange(0, count);
    };
    popToStep = doStep;

    // Highlight every relationship matching `match` (hovered line, or any chain through
    // a hovered hub); dim the rest. `null` resets to the resting state.
    const setHighlight = (match: ((r: Rel) => boolean) | null) => {
      for (const r of rels) {
        const on = match?.(r) ?? false;
        if (on) { r.mat.color.copy(hotColor); r.mat.opacity = 0.95; r.line.renderOrder = 1; }
        else if (match) { r.mat.color.copy(r.base); r.mat.opacity = 0.05; r.line.renderOrder = 0; }
        else { r.mat.color.copy(r.base); r.mat.opacity = REL_OPACITY; r.line.renderOrder = 0; }
      }
    };

    // Frame the whole graph.
    const box = new THREE.Box3().setFromObject(scene);
    const centre = box.getCenter(new THREE.Vector3()), size = box.getSize(new THREE.Vector3());
    controls.target.copy(centre);
    camera.position.copy(centre).add(new THREE.Vector3(size.x * 0.4, size.y * 0.3, Math.max(size.x, size.y, size.z) * 1.5 + 40));
    controls.update();

    // An *infinite* ground grid: a huge horizontal plane whose lines are drawn in
    // world space and fade with distance, so it reads as an endless floor (a
    // dimensional anchor) rather than a bounded card. It follows the camera in XZ.
    const gridBaseY = box.min.y - size.y * 0.12 - 8;
    const gridFade = Math.max(size.x, size.y, size.z, 200) * 2.4;
    const gridMat = new THREE.ShaderMaterial({
      transparent: true, depthWrite: false, side: THREE.DoubleSide,
      uniforms: {
        uColor: { value: relColor }, uCam: { value: new THREE.Vector3() },
        uCell: { value: 22 }, uMajor: { value: 110 }, uFade: { value: gridFade }, uOpacity: { value: 0.4 },
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
    let resting: ((r: Rel) => boolean) | null = null;

    // Focus: fly to a node (keeping the current view direction) and light its chains.
    // Driven by the `focusFqn` prop via `applyFocus` — used by "Show in 3D graph".
    const FOCUS_DIST: Record<string, number> = { system: 120, person: 70, container: 60, component: 38, data: 44 };
    let camFocus: { target: THREE.Vector3; dist: number } | null = null;
    const focusNode = (fqn: string) => {
      // The structure tree can select a flow (callable) that isn't a placed node — walk
      // up its FQN to the nearest enclosing node that is (its component / container).
      let id = fqn;
      while (id && !pos.has(id)) { const i = id.lastIndexOf("::"); id = i < 0 ? "" : id.slice(0, i); }
      const p = id ? pos.get(id) : undefined; if (!p) return;
      camFocus = { target: p.clone(), dist: FOCUS_DIST[levelOf.get(id) ?? "component"] ?? 60 };
      resting = (rl) => relUnder(rl, id);
      setHighlight(resting);
      const nodeSet = new Set([id]);
      for (const a of ancestors(id)) if (pos.has(a)) nodeSet.add(a);
      paintNodes(nodeSet);
      filterFlow((fr) => fr.from === id || fr.to === id);
    };
    applyFocus = focusNode;

    // Highlight a whole flow: light every relationship between two of its participant
    // nodes, frame them, and keep it lit (the resting highlight). `null` clears it.
    const focusPath = (path: string[] | null) => {
      const set = new Set((path ?? []).filter((id) => pos.has(id)));
      if (set.size === 0) { resting = null; setHighlight(null); paintNodes(null); filterFlow(null); return; }
      // The routed flow passes through each participant's gateways (its container,
      // system) — light those parent nodes too, while edges/flow still match the
      // participants themselves.
      const nodeSet = new Set(set);
      for (const id of set) for (const a of ancestors(id)) if (pos.has(a)) nodeSet.add(a);
      resting = () => false; // dim the chart; the bus route carries the highlight
      setHighlight(resting);
      paintNodes(nodeSet);
      filterFlow((fr) => set.has(fr.from) && set.has(fr.to));
      const fbox = new THREE.Box3();
      for (const id of nodeSet) fbox.expandByPoint(pos.get(id)!);
      const c = fbox.getCenter(new THREE.Vector3()), s = fbox.getSize(new THREE.Vector3());
      camFocus = { target: c, dist: Math.max(70, Math.max(s.x, s.y, s.z) * 1.4 + 50) };
    };
    applyPath = focusPath;
    applyFlowSeq = setFlowStream;

    // Click a relationship (its traffic) → reveal its routed path as a bus route: the
    // full journey through the gateways, each segment a leg, the current leg lit, with
    // the request bead and the step timeline. Dims the rest of the chart.
    const selectRelationship = (rl: Rel) => {
      if (!pos.has(rl.from) || !pos.has(rl.to)) return;
      resting = () => false; setHighlight(resting);    // dim the chart
      paintNodes(new Set([rl.from, rl.to]));           // light the two ends
      const fbox = new THREE.Box3().expandByPoint(pos.get(rl.from)!).expandByPoint(pos.get(rl.to)!);
      const c = fbox.getCenter(new THREE.Vector3()), s = fbox.getSize(new THREE.Vector3());
      camFocus = { target: c, dist: Math.max(70, Math.max(s.x, s.y, s.z) * 1.4 + 50) };
      // The real flow of this relationship: a single direct leg, in its own colour.
      setFlowStream([{ from: rl.from, to: rl.to, label: shortName(rl.to) }], rl.base);
      localSelected = true;
    };
    // Clear any selection back to the resting chart.
    const clearSelection = () => { focusPath(null); setFlowStream(null); localSelected = false; };
    clearLocal = clearSelection;

    if (highlightPath) focusPath(highlightPath);
    else if (focusFqn) focusNode(focusFqn);
    if (flowSequence) setFlowStream(flowSequence);

    // ---- hover: trace the chain ---------------------------------------------
    const ray = new THREE.Raycaster();
    ray.params.Line = { threshold: 2.4 };
    const ndc = new THREE.Vector2();
    const showTip = (x: number, y: number, html: string) => {
      const r = cv.getBoundingClientRect();
      tip.style.left = `${x - r.left}px`; tip.style.top = `${y - r.top}px`;
      tip.innerHTML = html; tip.style.opacity = "1";
    };
    const onMove = (e: PointerEvent) => {
      const r = cv.getBoundingClientRect();
      ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
      ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
      ray.setFromCamera(ndc, camera);
      // A hub (node) first: light every relationship it owns (itself or a descendant).
      const nodeHit = ray.intersectObjects(pickNodes, false)[0];
      if (nodeHit) {
        const n = (nodeHit.object as THREE.Object3D).userData as Node;
        setHighlight((rl) => relUnder(rl, n.id));
        const through = rels.filter((rl) => relUnder(rl, n.id)).length;
        showTip(e.clientX, e.clientY, `<b>${shortName(n.id)}</b><em>${n.level}${through ? ` · ${through} relationship${through === 1 ? "" : "s"}` : ""}</em>`);
        cv.style.cursor = "pointer"; return;
      }
      const lineHit = ray.intersectObjects(pickLines, false)[0];
      if (lineHit) {
        const idx = ((lineHit.object as THREE.Object3D).userData as { rel: number }).rel;
        const rl = rels[idx];
        setHighlight((x) => x === rl);
        showTip(e.clientX, e.clientY, `<b>${shortName(rl.from)} → ${shortName(rl.to)}</b>`);
        cv.style.cursor = "pointer"; return;
      }
      setHighlight(resting); tip.style.opacity = "0"; cv.style.cursor = "grab";
    };
    cv.addEventListener("pointermove", onMove);

    // A click (not a drag): on a relationship → reveal its real-flow route; on empty
    // space → clear back to the resting chart.
    let down = { x: 0, y: 0 };
    const onDown = (e: PointerEvent) => { down = { x: e.clientX, y: e.clientY }; };
    const onUp = (e: PointerEvent) => {
      if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5) return; // a drag (orbit/pan)
      const r = cv.getBoundingClientRect();
      ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
      ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
      ray.setFromCamera(ndc, camera);
      const lineHit = ray.intersectObjects(pickLines, false)[0];
      if (lineHit) {
        selectRelationship(rels[((lineHit.object as THREE.Object3D).userData as { rel: number }).rel]);
        return;
      }
      if (ray.intersectObjects(pickNodes, false)[0]) return; // a node — leave to hover/structure
      clearSelection(); ondeselect?.();
    };
    cv.addEventListener("pointerdown", onDown);
    cv.addEventListener("pointerup", onUp);

    const resize = () => { camera.aspect = W() / H(); camera.updateProjectionMatrix(); renderer.setSize(W(), H()); };
    addEventListener("resize", resize);
    const onKey = (e: KeyboardEvent) => { if (e.key !== "Escape") return; clearSelection(); if (onclose) onclose(); else ondeselect?.(); };
    addEventListener("keydown", onKey);

    let raf = 0, alive = true, last = performance.now(), flowT = 0;
    const flowTmp = new THREE.Vector3();
    const frame = () => {
      if (!alive) return;
      const now = performance.now(), dt = Math.min((now - last) / 1000, 0.05); last = now;
      if (camFocus) {
        controls.target.lerp(camFocus.target, 0.12);
        const offset = camera.position.clone().sub(controls.target);
        if (offset.lengthSq() < 1e-6) offset.set(0, 0, 1);
        camera.position.copy(controls.target).add(offset.setLength(camFocus.dist));
        if (controls.target.distanceTo(camFocus.target) < 0.4) camFocus = null;
      }
      // Stream the traffic comets along their routes (caller → callee): each is a lead
      // dot plus a short fading tail trailing behind it.
      flowT += dt;
      for (let i = 0; i < particles.length; i++) {
        const p = particles[i], fr = flowRoutes[p.route];
        const lead = flowT * fr.speed + p.off;
        for (let k = 0; k < TRAIL; k++) {
          let t = (lead - k * TRAIL_GAP) % 1; if (t < 0) t += 1;
          along(fr, t, flowTmp);
          const o = (i * TRAIL + k) * 3;
          flowArr[o] = flowTmp.x; flowArr[o + 1] = flowTmp.y; flowArr[o + 2] = flowTmp.z;
        }
      }
      flowGeo.attributes.position.needsUpdate = true;
      flowMat.opacity = 0.7 + 0.25 * Math.sin(flowT * 2.2); // gentle volume-agnostic pulse
      // Traffic flows continuously along the current step's routed segment (manual
      // control via `flowStep`): a spaced string of dots looping caller → callee.
      if (current) {
        const { sr, count, travel } = current;
        for (let i = 0; i < count; i++) {
          const t = (flowT / travel + i / count) % 1; // constant speed; evenly spaced
          alongPoly(sr.pts, sr.cum, sr.total, t, flowTmp);
          streamArr[i * 3] = flowTmp.x; streamArr[i * 3 + 1] = flowTmp.y; streamArr[i * 3 + 2] = flowTmp.z;
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
      alive = false; cancelAnimationFrame(raf);
      cv.removeEventListener("pointermove", onMove);
      cv.removeEventListener("pointerdown", onDown); cv.removeEventListener("pointerup", onUp);
      removeEventListener("resize", resize); removeEventListener("keydown", onKey);
      // Free GPU resources — renderer.dispose() doesn't reclaim user geometries /
      // materials / textures, and this component remounts on theme + workspace changes.
      scene.traverse((o) => {
        const obj = o as THREE.Mesh & { geometry?: THREE.BufferGeometry; material?: THREE.Material | THREE.Material[] };
        obj.geometry?.dispose();
        for (const m of obj.material ? (Array.isArray(obj.material) ? obj.material : [obj.material]) : []) {
          (m as THREE.Material & { map?: THREE.Texture }).map?.dispose();
          m.dispose();
        }
      });
      controls.dispose(); renderer.dispose();
    };
  });
</script>

<div class="graph">
  <canvas bind:this={canvas}></canvas>
  <div class="bar">
    {#if hasSelection}<button onclick={() => { clearLocal?.(); ondeselect?.(); }}>⤺ Deselect</button>{/if}
    {#if onclose}<button onclick={() => onclose?.()}>✕ Close (Esc)</button>{/if}
  </div>
  <div class="hint">
    {snapshot.nodes.length} nodes · {snapshot.edges.length} relations · <b>hover</b> a relationship or hub to trace it · <b>drag</b> orbit · <b>scroll</b> zoom
  </div>
  <div class="legend">
    <span><i style="background:var(--k-system)"></i>system</span>
    <span><i style="background:var(--k-container)"></i>container</span>
    <span><i style="background:var(--k-component)"></i>component</span>
    <span><i style="background:var(--k-person)"></i>person</span>
  </div>
  {#if flowSteps.length}
    <div class="timeline">
      <div class="tl-head">
        <span>Flow · step {Math.max(0, flowStep) + 1}/{flowSteps.length}</span>
        <span class="tl-ctrls">
          <button onclick={() => (flowStep = Math.max(0, flowStep - 1))} disabled={flowStep <= 0} aria-label="Previous step">‹</button>
          <button onclick={() => (flowStep = Math.min(flowSteps.length - 1, flowStep + 1))} disabled={flowStep >= flowSteps.length - 1} aria-label="Next step">›</button>
        </span>
      </div>
      <ol>
        {#each flowSteps as step, i (i)}
          <li class:done={i < flowStep} class:now={i === flowStep} class:pending={i > flowStep}>
            <button class="tl-row" onclick={() => (flowStep = i)}>
              <span class="tl-call">{step.from} → {step.to}</span>
              {#if step.label}<span class="tl-label">{step.label}</span>{/if}
            </button>
          </li>
        {/each}
      </ol>
    </div>
  {/if}
  <div class="tip" bind:this={tipEl}></div>
</div>

<style>
  .graph { position: absolute; inset: 0; background: var(--island-bg); overflow: hidden; }
  canvas { display: block; width: 100%; height: 100%; }
  .bar { position: absolute; top: 14px; right: 16px; display: flex; gap: 8px; }
  .bar button { font: 12px/1 var(--font-sans); color: var(--ink-soft); cursor: pointer; background: color-mix(in srgb, var(--surface) 80%, transparent); border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; }
  .bar button:hover { border-color: var(--accent); color: var(--ink); }
  .hint { position: absolute; top: 16px; left: 18px; font: 12px/1.6 var(--font-mono); color: var(--ink-faint); pointer-events: none; }
  .hint b { color: var(--ink-soft); }
  .legend { position: absolute; left: 18px; bottom: 16px; display: flex; gap: 14px; font: 12px/1.5 var(--font-sans); color: var(--ink-soft); pointer-events: none; }
  .legend span { display: inline-flex; align-items: center; gap: 6px; }
  .legend i { width: 9px; height: 9px; border-radius: 50%; box-shadow: 0 0 7px currentColor; }
  .timeline {
    position: absolute; right: 16px; bottom: 16px; width: 260px; max-height: 46%;
    overflow-y: auto; padding: 10px 12px; border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    border: 1px solid var(--line); box-shadow: 0 6px 24px #0006;
    font: 12px/1.5 var(--font-sans); color: var(--ink-soft);
  }
  .timeline .tl-head { display: flex; align-items: center; justify-content: space-between; font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.12em; text-transform: uppercase; color: var(--ink-faint); margin-bottom: 8px; }
  .timeline .tl-ctrls { display: inline-flex; gap: 4px; }
  .timeline .tl-ctrls button { width: 20px; height: 20px; line-height: 1; cursor: pointer; color: var(--ink-soft); background: var(--surface-2, transparent); border: 1px solid var(--line); border-radius: 5px; }
  .timeline .tl-ctrls button:hover:not(:disabled) { border-color: var(--accent); color: var(--ink); }
  .timeline .tl-ctrls button:disabled { opacity: 0.4; cursor: default; }
  .timeline ol { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .timeline li { border-left: 2px solid transparent; border-radius: 4px; transition: opacity .2s, background .2s; }
  .timeline .tl-row { display: block; width: 100%; text-align: left; padding: 4px 8px; background: none; border: 0; cursor: pointer; color: inherit; font: inherit; }
  .timeline li.pending { opacity: 0.35; } /* upcoming steps fade in as the request reaches them */
  .timeline li.done { opacity: 0.7; border-left-color: color-mix(in srgb, var(--accent) 40%, transparent); }
  .timeline li.now { opacity: 1; border-left-color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
  .timeline .tl-call { display: block; font-family: var(--font-mono); font-size: 11px; color: var(--ink); }
  .timeline li.pending .tl-call { color: var(--ink-soft); }
  .timeline .tl-label { display: block; color: var(--ink-soft); }
  .tip { position: absolute; z-index: 7; padding: 6px 9px; border-radius: 7px; background: color-mix(in srgb, var(--surface) 92%, transparent); border: 1px solid var(--line); font: 12px/1.4 var(--font-sans); color: var(--ink); pointer-events: none; opacity: 0; transition: opacity .12s; white-space: nowrap; transform: translate(-50%, -135%); }
  .tip :global(b) { color: var(--ink); } .tip :global(em) { display: block; font-style: normal; color: var(--ink-soft); margin-top: 2px; }
</style>
