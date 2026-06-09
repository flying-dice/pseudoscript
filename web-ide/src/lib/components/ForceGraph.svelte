<script module lang="ts">
  // Baked layout cache, shared across remounts. The component remounts on every model
  // edit and theme toggle; the force sim is deterministic, so when the graph topology
  // (node ids + edges) is unchanged we reuse the settled positions instead of re-running
  // ~400 ticks on the main thread. Theme toggles and non-structural edits become cheap.
  type Baked = { x: number; y: number; z: number };
  let layoutCache: { sig: string; pos: Map<string, Baked> } | null = null;
</script>

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
  import { flowColor as flowHexOf } from "$lib/flow-color";
  import { ancestors, routeOf, simpleName } from "$lib/graph-route";
  import type { UniverseSnapshot } from "$lib/pds";

  // The graph shape is the Rust-derived DTO (single source of truth across the wasm
  // boundary); the node/edge element types are read off it.
  type Snapshot = UniverseSnapshot;
  type Node = Snapshot["nodes"][number];

  // `onclose` present → rendered as a full-screen overlay (show a close button);
  // absent → embedded as a panel view (the activity bar switches away from it).
  // `focusFqn` → fly the camera to that node and light its relationships.
  // `highlightPath` → an ordered list of a flow's participant nodes: light the whole
  // chain between them and frame it (takes precedence over `focusFqn`).
  // One call in a flow: caller → callee node ids, plus the message label.
  type FlowHop = { from: string; to: string; label: string };
  // A whole flow: an entry point's call chain as ordered legs, plus the colour keyed
  // to its start. Each leg is drawn as its own filament in the resting view.
  type FlowDef = { fqn: string; color: string; hops: FlowHop[] };
  type Props = {
    snapshot: Snapshot;
    onclose?: (() => void) | null;
    focusFqn?: string | null;
    highlightPath?: string[] | null;
    flowSequence?: FlowHop[] | null;
    flowColor?: string | null;
    // The selected flow's name, shown in the timeline header.
    flowName?: string | null;
    ondeselect?: (() => void) | null;
    // Every flow in the model, drawn as resting filaments (one per leg, flow-coloured).
    flows?: FlowDef[] | null;
    // Clicking a node or a flow filament → pick it (same effect as the structure panel).
    onpick?: ((fqn: string) => void) | null;
  };
  let { snapshot, onclose = null, focusFqn = null, highlightPath = null, flowSequence = null, flowColor = null, flowName = null, ondeselect = null, flows = null, onpick = null }: Props = $props();

  // Reset handle into onMount's scene (clears the in-canvas highlight on Deselect/Esc).
  let clearLocal: (() => void) | null = null;
  // Whether a selection is active (drives the Deselect button + background-click reset).
  const hasSelection = $derived(!!focusFqn || !!(highlightPath?.length) || !!(flowSequence?.length));

  let canvas = $state<HTMLCanvasElement | null>(null);
  let tipEl = $state<HTMLDivElement | null>(null);
  // The selected flow's ordered steps + which one the request is on, for the timeline.
  let flowSteps = $state<{ from: string; to: string; label: string }[]>([]);
  let flowStep = $state(-1);
  // When a clicked leg carries several flows, the chooser: where to anchor it (canvas
  // coords) and the flows to pick from. Null = closed.
  let flowChoice = $state<{ x: number; y: number; items: { fqn: string; name: string; color: string }[] } | null>(null);

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

  const SIZE: Record<string, number> = { system: 6, container: 3.4, component: 1.9, person: 3 };
  const REL_OPACITY = 0.15;

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
    // A pale sheet vs the dark void: drives additive-vs-alpha choices throughout (additive
    // glow is a no-op on near-white, so the bright effects switch to alpha there).
    const lightBg = bgColor.r + bgColor.g + bgColor.b > 1.5;

    // ---- containment + routing ----------------------------------------------
    // `ancestors`/`routeOf` are the pure gateway-routing logic (tested in graph-route.ts).
    const ids = new Set(snapshot.nodes.map((n) => n.id));
    const levelOf = new Map(snapshot.nodes.map((n) => [n.id, n.level]));
    const parentOf = new Map(snapshot.nodes.map((n) => [n.id, n.parent ?? null]));
    const anc = (id: string) => ancestors(id, parentOf);

    const routes = snapshot.edges
      .filter((e) => e.from !== e.to && ids.has(e.from) && ids.has(e.to))
      .map((e) => ({ from: e.from, to: e.to, traffic: e.traffic, ...routeOf(e.from, e.to, parentOf) }));

    // ---- d3-force-3d layout (baked once; reused across remounts) -------------
    // Containment links cluster: short + strong for container→component, wider for
    // system→container so component balls don't collide. Also drives the faint tree.
    const contain = snapshot.nodes
      .filter((n) => n.parent && ids.has(n.parent))
      .map((n) => ({ source: n.parent as string, target: n.id, dist: levelOf.get(n.parent as string) === "system" ? 60 : 16 }));
    // The graph topology — the only thing the layout depends on. Same signature → the
    // cached positions are still valid, so skip the (deterministic) force sim entirely.
    const sig = snapshot.nodes.map((n) => n.id).join(",") + "|" + snapshot.edges.map((e) => e.from + ">" + e.to).join(",");
    const pos = new Map<string, THREE.Vector3>();
    if (layoutCache && layoutCache.sig === sig) {
      for (const [id, p] of layoutCache.pos) pos.set(id, new THREE.Vector3(p.x, p.y, p.z));
    } else {
      const nodes = snapshot.nodes.map((n) => ({ id: n.id, level: n.level }));
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
      const baked = new Map<string, Baked>();
      for (const n of nodes as unknown as { id: string; x: number; y: number; z: number }[]) {
        baked.set(n.id, { x: n.x, y: n.y, z: n.z });
        pos.set(n.id, new THREE.Vector3(n.x, n.y, n.z));
      }
      layoutCache = { sig, pos: baked };
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
    const keyLight = new THREE.DirectionalLight(0xffffff, 0.8); keyLight.position.set(1, 1.3, 0.7); scene.add(keyLight);
    const fillLight = new THREE.DirectionalLight(0xffffff, 0.22); fillLight.position.set(-1, -0.2, -0.6); scene.add(fillLight);

    // Nodes: every placed sphere in ONE InstancedMesh — a single draw call for the lot
    // (vs. one per node). Selection dims/brightens via per-instance colour, not opacity,
    // so the mesh stays opaque (no per-object transparency sort).
    const placed = snapshot.nodes.filter((n) => pos.has(n.id));
    const nodeIdx = new Map(placed.map((n, i) => [n.id, i] as const));
    const baseCol = placed.map((n) => (COLOR[n.level] ?? fallbackColor).clone());
    const nodeGeo = new THREE.SphereGeometry(1, 24, 16); // doubled segments per axis — smoother orbs (shared by one InstancedMesh, so cost is paid once)
    const nodeMat = new THREE.MeshStandardMaterial({ roughness: 0.5, metalness: 0.15 });
    const nodes3d = new THREE.InstancedMesh(nodeGeo, nodeMat, placed.length);
    const dummy = new THREE.Object3D();
    for (let i = 0; i < placed.length; i++) {
      const n = placed[i];
      dummy.position.copy(pos.get(n.id)!);
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
    const paintNodes = (set: Set<string> | null) => {
      for (let i = 0; i < placed.length; i++) {
        const on = !set || set.has(placed[i].id);
        dimC.copy(baseCol[i]); if (set && !on) dimC.multiplyScalar(0.18);
        nodes3d.setColorAt(i, dimC);
      }
      if (nodes3d.instanceColor) nodes3d.instanceColor.needsUpdate = true;
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
      const spr = makeLabel(simpleName(n.id), h);
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
    //  • DATA FLOWS — the actual flows running directly between two spheres, each leg a
    //    straight line, with beads of light tracking along.
    // The parent-pathway polyline (gateway stops). Plain — no offset, overlap allowed.
    const parentLine = (ids: string[]): THREE.Vector3[] =>
      ids.map((id) => pos.get(id)).filter((v): v is THREE.Vector3 => !!v);
    // A straight segment between two nodes — endpoints only. Used for both the resting
    // leg filaments and the selected flow's bus route, so flows read dead-straight.
    const straight = (fromId: string, toId: string): THREE.Vector3[] => {
      const a = pos.get(fromId), b = pos.get(toId);
      return a && b ? [a.clone(), b.clone()] : [];
    };

    // A polyline with cumulative segment lengths, so a fraction `t` can be resolved to a
    // point along it in constant time (see `along`). `total` is floored at 1 to keep
    // degenerate (zero-length) lines safe to divide by.
    type Poly = { pts: THREE.Vector3[]; cum: number[]; total: number };
    const polyline = (pts: THREE.Vector3[]): Poly => {
      const cum = [0];
      for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + pts[i].distanceTo(pts[i - 1]));
      return { pts, cum, total: cum[cum.length - 1] || 1 };
    };

    // Layout relationships: faint structural lines along the parent pathway (overlap ok).
    for (const r of routes) {
      const pts = parentLine(r.path);
      if (pts.length < 2) continue;
      scene.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts),
        new THREE.LineBasicMaterial({ color: treeColor, transparent: true, opacity: 0.16 })));
    }

    // Data flows: one filament per leg -- a straight line between two nodes, shared by
    // every flow that crosses it. The line is neutral; the TRAFFIC carries the colour:
    // each flow on the leg streams beads in its own hue, so a leg used by five flows is
    // one line with five colours of dots. Click a leg to choose which flow to open.
    const flowDefs = flows ?? [];
    type LegFlow = { fqn: string; name: string; hex: string; color: THREE.Color };
    type Filament = Poly & { from: string; to: string; legFlows: LegFlow[]; line: THREE.Line; mat: THREE.LineBasicMaterial };
    const filaments: Filament[] = [];
    const pickLines: THREE.Line[] = [];
    const legByKey = new Map<string, number>();
    const particles: { fil: number; off: number; color: THREE.Color }[] = [];
    const BEADS = 2; // beads per flow on a leg
    // 1. One straight filament per directed leg; record which flows run over it.
    for (const fl of flowDefs) {
      const color = new THREE.Color(fl.color);
      const name = simpleName(fl.fqn);
      for (const h of fl.hops) {
        if (h.from === h.to || !pos.has(h.from) || !pos.has(h.to)) continue;
        const key = h.from + ">" + h.to;
        let fi = legByKey.get(key);
        if (fi === undefined) {
          const pts = straight(h.from, h.to); // straight line
          const mat = new THREE.LineBasicMaterial({ color: relColor, transparent: true, opacity: REL_OPACITY });
          const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), mat);
          (line as THREE.Object3D).userData = { fil: filaments.length }; line.renderOrder = 1;
          scene.add(line); pickLines.push(line);
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
      const fs = filaments[fi].legFlows, F = fs.length;
      for (let j = 0; j < F; j++)
        for (let i = 0; i < BEADS; i++) particles.push({ fil: fi, off: (i + j / F) / BEADS, color: fs[j].color });
    }
    // Does filament `f` touch node `id` — is `id` an endpoint or an ancestor (its
    // container / system) of one? Lets hovering a hub light every flow under it.
    const filUnder = (f: Filament, id: string) => anc(f.from).includes(id) || anc(f.to).includes(id);
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
      const c = particles[i].color, o = i * 3; // each bead carries its flow's colour
      flowColArr[o] = c.r * beadDim; flowColArr[o + 1] = c.g * beadDim; flowColArr[o + 2] = c.b * beadDim;
    }
    const flowGeo = new THREE.BufferGeometry();
    flowGeo.setAttribute("position", new THREE.BufferAttribute(flowArr, 3));
    flowGeo.setAttribute("color", new THREE.BufferAttribute(flowColArr, 3));
    // A round sprite so particles are dots, not squares (PointsMaterial's default). The
    // dark sheet wants a soft glow (additive); the pale sheet wants a near-solid disc —
    // a soft halo just reads as a pale smudge there, so hold full alpha almost to the rim.
    const dotCanvas = document.createElement("canvas");
    dotCanvas.width = dotCanvas.height = 64;
    const dctx = dotCanvas.getContext("2d")!;
    const grad = dctx.createRadialGradient(32, 32, 0, 32, 32, 32);
    grad.addColorStop(0, "rgba(255,255,255,1)");
    grad.addColorStop(lightBg ? 0.82 : 0.5, lightBg ? "rgba(255,255,255,1)" : "rgba(255,255,255,0.85)");
    grad.addColorStop(1, "rgba(255,255,255,0)");
    dctx.fillStyle = grad; dctx.beginPath(); dctx.arc(32, 32, 32, 0, Math.PI * 2); dctx.fill();
    const dotTex = new THREE.CanvasTexture(dotCanvas);
    const flowMat = new THREE.PointsMaterial({ vertexColors: true, map: dotTex, size: 2.2, sizeAttenuation: true, transparent: true, opacity: 0.95, blending: beadBlend, depthWrite: false });
    scene.add(new THREE.Points(flowGeo, flowMat));
    // Position `out` at fraction `t` along a polyline (its points + cumulative lengths).
    const along = ({ pts, cum, total }: Poly, t: number, out: THREE.Vector3) => {
      const d = t * total;
      let k = 0; while (k < cum.length - 2 && cum[k + 1] <= d) k++;
      const seg = cum[k + 1] - cum[k] || 1;
      out.copy(pts[k]).lerp(pts[k + 1], (d - cum[k]) / seg);
    };
    // Show only the beads on filaments matching `keep` (a selection); `null` shows all.
    // Hidden beads are parked at NaN — the GPU discards the point, so they vanish under
    // any blend mode (a recolour-to-transparent would smudge over orbs the bead overlaps).
    // `beadsOn` lets the frame loop skip moving beads no one can see; the loop only
    // repositions shown beads, so parked ones stay parked.
    const filterFlow = (keep: ((f: Filament) => boolean) | null) => {
      let any = false;
      for (let i = 0; i < particles.length; i++) {
        const on = !keep || keep(filaments[particles[i].fil]);
        if (on) any = true;
        beadShown[i] = on ? 1 : 0;
        if (!on) { const o = i * 3; flowArr[o] = flowArr[o + 1] = flowArr[o + 2] = NaN; }
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
    const streamMat = new THREE.PointsMaterial({ color: hotColor.clone().multiplyScalar(beadDim), map: dotTex, size: 3.6, sizeAttenuation: true, transparent: true, opacity: 0.95, blending: beadBlend, depthWrite: false });
    scene.add(new THREE.Points(streamGeo, streamMat));
    const FLOW_SPEED = 55; // world units / second — one constant crossing speed
    let stepRoutes: Poly[] = [];
    let current: { sr: Poly; count: number; travel: number } | null = null;
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
      flowColor = color ? (typeof color === "string" ? new THREE.Color(color) : color) : new THREE.Color(flowHexOf(`${hops[0].from}>${hops[hops.length - 1].to}`)); // every flow its own colour
      streamMat.color.copy(flowColor);
      // One direct arc per call step (caller → callee) — the real flow path.
      stepRoutes = hops.map((h) => polyline(straight(h.from, h.to))); // straight leg per hop
      // Draw every leg of the route (the bus line).
      for (const sr of stepRoutes) {
        if (sr.pts.length < 2) { busSegs.push({ line: new THREE.Line(), mat: new THREE.LineBasicMaterial() }); continue; }
        const mat = new THREE.LineBasicMaterial({ color: relColor, transparent: true, opacity: 0.45 });
        const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(sr.pts), mat);
        line.renderOrder = 1; scene.add(line); busSegs.push({ line, mat });
      }
      filterFlow(() => false); // the stepping stream replaces the per-edge particles
      flowSteps = hops.map((h) => ({ from: simpleName(h.from), to: simpleName(h.to), label: h.label }));
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

    // Highlight every leg matching `match` (the hovered leg, or every leg through a
    // hovered hub); dim the rest. The line is neutral — its colour brightens, the beads
    // keep their per-flow hues. `null` resets to resting.
    const setHighlight = (match: ((f: Filament) => boolean) | null) => {
      for (const f of filaments) {
        const on = match?.(f) ?? false;
        if (on) { f.mat.color.copy(hotColor); f.mat.opacity = 0.9; f.line.renderOrder = 1; }
        else if (match) { f.mat.color.copy(relColor); f.mat.opacity = 0.05; f.line.renderOrder = 0; }
        else { f.mat.color.copy(relColor); f.mat.opacity = REL_OPACITY; f.line.renderOrder = 0; }
      }
    };

    // Frame the whole graph.
    const box = new THREE.Box3().setFromObject(scene);
    const centre = box.getCenter(new THREE.Vector3()), size = box.getSize(new THREE.Vector3());
    controls.target.copy(centre);
    camera.position.copy(centre).add(new THREE.Vector3(size.x * 0.4, size.y * 0.3, Math.max(size.x, size.y, size.z) * 1.5 + 40));
    controls.update();

    // Distance fog tinted to the backdrop: nearer orbs read crisp, far ones recede into
    // the sheet, adding aerial depth. Camera-relative, so it tracks orbit/zoom for free.
    // Scaled off the framed scene so it fits any model (near just ahead of the front face,
    // far past the back so distant nodes fade without vanishing).
    const maxDim = Math.max(size.x, size.y, size.z, 1);
    const camDist = camera.position.distanceTo(centre);
    scene.fog = new THREE.Fog(bgColor, camDist - maxDim * 0.5, camDist + maxDim * 1.3);

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
    let resting: ((f: Filament) => boolean) | null = null;

    // Focus: fly to a node (keeping the current view direction) and light its chains.
    // Driven by the `focusFqn` prop via `applyFocus` — used by "Show in 3D graph".
    const FOCUS_DIST: Record<string, number> = { system: 120, person: 70, container: 60, component: 38 };
    let camFocus: { target: THREE.Vector3; dist: number } | null = null;
    const focusNode = (fqn: string) => {
      // The structure tree can select a flow (callable) that isn't a placed node — walk
      // up its FQN to the nearest enclosing node that is (its component / container).
      let id = fqn;
      while (id && !pos.has(id)) { const i = id.lastIndexOf("::"); id = i < 0 ? "" : id.slice(0, i); }
      const p = id ? pos.get(id) : undefined; if (!p) return;
      camFocus = { target: p.clone(), dist: FOCUS_DIST[levelOf.get(id) ?? "component"] ?? 60 };
      resting = (f) => filUnder(f, id);
      setHighlight(resting);
      const nodeSet = new Set([id]);
      for (const a of anc(id)) if (pos.has(a)) nodeSet.add(a);
      paintNodes(nodeSet);
      filterFlow((f) => filUnder(f, id));
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
      for (const id of set) for (const a of anc(id)) if (pos.has(a)) nodeSet.add(a);
      resting = () => false; // dim the chart; the bus route carries the highlight
      setHighlight(resting);
      paintNodes(nodeSet);
      filterFlow((f) => set.has(f.from) && set.has(f.to));
      const fbox = new THREE.Box3();
      for (const id of nodeSet) fbox.expandByPoint(pos.get(id)!);
      const c = fbox.getCenter(new THREE.Vector3()), s = fbox.getSize(new THREE.Vector3());
      camFocus = { target: c, dist: Math.max(70, Math.max(s.x, s.y, s.z) * 1.4 + 50) };
    };
    applyPath = focusPath;
    applyFlowSeq = setFlowStream;

    // Clear any selection back to the resting chart.
    const clearSelection = () => { focusPath(null); setFlowStream(null); flowChoice = null; };
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
      // A hub (node) first: light every leg through it (itself or a descendant).
      const nodeHit = ray.intersectObject(nodes3d, false)[0];
      if (nodeHit && nodeHit.instanceId != null) {
        const n = placed[nodeHit.instanceId];
        setHighlight((f) => filUnder(f, n.id));
        const through = new Set(filaments.filter((f) => filUnder(f, n.id)).flatMap((f) => f.legFlows.map((x) => x.fqn))).size;
        showTip(e.clientX, e.clientY, `<b>${simpleName(n.id)}</b><em>${n.level}${through ? ` · ${through} flow${through === 1 ? "" : "s"}` : ""}</em>`);
        cv.style.cursor = "pointer"; return;
      }
      // A leg: light it and say how many flows it carries (click to choose / open).
      const lineHit = ray.intersectObjects(pickLines, false)[0];
      if (lineHit) {
        const fil = filaments[((lineHit.object as THREE.Object3D).userData as { fil: number }).fil];
        setHighlight((f) => f === fil);
        const n = fil.legFlows.length;
        showTip(e.clientX, e.clientY, `<b>${simpleName(fil.from)} → ${simpleName(fil.to)}</b><em>${n} flow${n === 1 ? "" : "s"} · click to ${n === 1 ? "open" : "choose"}</em>`);
        cv.style.cursor = "pointer"; return;
      }
      setHighlight(resting); tip.style.opacity = "0"; cv.style.cursor = "grab";
    };
    cv.addEventListener("pointermove", onMove);

    // A click (not a drag): on a node → pick it (same as the structure panel); on a leg
    // → open its flow, or offer a choice when several flows share the leg; on empty
    // space → dismiss any open chooser, else clear the chart.
    let down = { x: 0, y: 0 };
    const onDown = (e: PointerEvent) => { down = { x: e.clientX, y: e.clientY }; };
    const onUp = (e: PointerEvent) => {
      if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5) return; // a drag (orbit/pan)
      const r = cv.getBoundingClientRect();
      ndc.x = ((e.clientX - r.left) / r.width) * 2 - 1;
      ndc.y = -((e.clientY - r.top) / r.height) * 2 + 1;
      ray.setFromCamera(ndc, camera);
      const nodeHit = ray.intersectObject(nodes3d, false)[0];
      if (nodeHit && nodeHit.instanceId != null) {
        flowChoice = null;
        onpick?.(placed[nodeHit.instanceId].id);
        return;
      }
      const lineHit = ray.intersectObjects(pickLines, false)[0];
      if (lineHit) {
        const fil = filaments[((lineHit.object as THREE.Object3D).userData as { fil: number }).fil];
        if (fil.legFlows.length === 1) { flowChoice = null; onpick?.(fil.legFlows[0].fqn); }
        else flowChoice = { x: e.clientX - r.left, y: e.clientY - r.top, items: fil.legFlows.map((f) => ({ fqn: f.fqn, name: f.name, color: f.hex })) };
        return;
      }
      if (flowChoice) { flowChoice = null; return; } // a click-away dismisses the chooser
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
      // Stream the traffic beads along their legs (caller → callee), one dot each. Skip
      // it entirely while a flow is selected (resting beads are hidden then).
      flowT += dt;
      if (beadsOn) {
        for (let i = 0; i < particles.length; i++) {
          if (!beadShown[i]) continue; // parked at NaN by filterFlow — leave it hidden
          const p = particles[i];
          let t = (flowT * REST_SPEED + p.off) % 1; if (t < 0) t += 1;
          along(filaments[p.fil], t, flowTmp);
          const o = i * 3;
          flowArr[o] = flowTmp.x; flowArr[o + 1] = flowTmp.y; flowArr[o + 2] = flowTmp.z;
        }
        flowGeo.attributes.position.needsUpdate = true;
      }
      flowMat.opacity = 0.7 + 0.25 * Math.sin(flowT * 2.2); // gentle volume-agnostic pulse
      // Traffic flows continuously along the current step's routed segment (manual
      // control via `flowStep`): a spaced string of dots looping caller → callee.
      if (current) {
        const { sr, count, travel } = current;
        for (let i = 0; i < count; i++) {
          const t = (flowT / travel + i / count) % 1; // constant speed; evenly spaced
          along(sr, t, flowTmp);
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
      // Collect each resource ONCE (a texture shared by two materials, e.g. the bead
      // sprite, must not be disposed twice), then free. Sets dedupe; the InstancedMesh's
      // instance buffers need its own dispose(); live bus-route segments are freed first.
      clearBus();
      const geos = new Set<THREE.BufferGeometry>();
      const mats = new Set<THREE.Material>();
      const texs = new Set<THREE.Texture>();
      scene.traverse((o) => {
        const obj = o as THREE.Mesh;
        if (obj.geometry) geos.add(obj.geometry);
        const m = obj.material;
        if (m) for (const mm of Array.isArray(m) ? m : [m]) {
          mats.add(mm);
          const map = (mm as THREE.Material & { map?: THREE.Texture | null }).map;
          if (map) texs.add(map);
        }
      });
      texs.forEach((t) => t.dispose());
      mats.forEach((m) => m.dispose());
      geos.forEach((g) => g.dispose());
      nodes3d.dispose(); // frees instanceMatrix + instanceColor
      controls.dispose(); renderer.dispose();
    };
  });
</script>

<div class="graph" data-testid="universe">
  <canvas bind:this={canvas} data-testid="universe-canvas"></canvas>
  <div class="bar">
    {#if hasSelection}<button onclick={() => { clearLocal?.(); ondeselect?.(); }}>⤺ Deselect</button>{/if}
    {#if onclose}<button onclick={() => onclose?.()}>✕ Close (Esc)</button>{/if}
  </div>
  <div class="hint">
    {snapshot.nodes.length} nodes · {flows?.length ?? 0} flows · <b>hover</b> a leg · <b>click</b> to open a flow · <b>drag</b> orbit · <b>scroll</b> zoom
  </div>
  {#if flowChoice}
    <div class="flow-choice" style="left:{flowChoice.x}px; top:{flowChoice.y}px">
      <div class="fc-head">Open flow</div>
      {#each flowChoice.items as f (f.fqn)}
        <button onclick={() => { onpick?.(f.fqn); flowChoice = null; }}>
          <i style="background:{f.color}"></i>{f.name}
        </button>
      {/each}
    </div>
  {/if}
  <div class="legend">
    <span><i style="background:var(--k-system)"></i>system</span>
    <span><i style="background:var(--k-container)"></i>container</span>
    <span><i style="background:var(--k-component)"></i>component</span>
    <span><i style="background:var(--k-person)"></i>person</span>
  </div>
  {#if flowSteps.length}
    <div class="timeline" data-testid="flow-timeline">
      {#if flowName}
        <div class="tl-flow">
          {#if flowColor}<i style="background:{flowColor}"></i>{/if}<span data-testid="flow-name">{flowName}</span>
        </div>
      {/if}
      <div class="tl-head">
        <span>step {Math.max(0, flowStep) + 1}/{flowSteps.length}</span>
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
  /* The flow chooser: anchored where you clicked a leg carrying several flows. */
  .flow-choice {
    position: absolute; z-index: 8; min-width: 150px; max-height: 50%; overflow-y: auto;
    padding: 5px; border-radius: 9px; transform: translate(8px, 8px);
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    border: 1px solid var(--line); box-shadow: 0 8px 28px #0007;
  }
  .flow-choice .fc-head { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.12em; text-transform: uppercase; color: var(--ink-faint); padding: 3px 7px 5px; }
  .flow-choice button {
    display: flex; align-items: center; gap: 8px; width: 100%; text-align: left;
    padding: 5px 7px; border: 0; border-radius: 6px; background: none; cursor: pointer;
    font: 12px/1.4 var(--font-sans); color: var(--ink-soft);
  }
  .flow-choice button:hover { background: var(--surface-3); color: var(--ink); }
  .flow-choice i { width: 9px; height: 9px; flex: none; border-radius: 50%; box-shadow: 0 0 7px currentColor; }
  .timeline {
    position: absolute; right: 16px; bottom: 16px; width: 260px; max-height: 46%;
    overflow-y: auto; padding: 10px 12px; border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    border: 1px solid var(--line); box-shadow: 0 6px 24px #0006;
    font: 12px/1.5 var(--font-sans); color: var(--ink-soft);
  }
  .timeline .tl-flow { display: flex; align-items: center; gap: 7px; font-size: 13px; font-weight: 600; color: var(--ink); margin-bottom: 6px; }
  .timeline .tl-flow i { width: 9px; height: 9px; flex: none; border-radius: 50%; box-shadow: 0 0 7px currentColor; }
  .timeline .tl-flow span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
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
