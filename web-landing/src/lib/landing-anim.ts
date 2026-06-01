/* =============================================================================
   PseudoScript landing — interactions (ported from the design's landing.js).
   - live .pds typing in the hero, syncing a C4 diagram that assembles
   - static syntax-highlighted panels (Describe step, IDE editor)
   - diagnostics resolving (Refine step)
   - a projected sequence diagram (Generate step)
   - scroll reveals + the convergence core pulse
   Triggers are scroll-driven (with an immediate initial pass) so they fire
   reliably regardless of IntersectionObserver quirks.

   Exposed as initLanding(): call once after the page DOM has mounted. Returns
   a teardown function that removes the scroll/resize listeners.
   ========================================================================== */
import { pdsTokenize, pdsHighlight } from './pds-syntax.js';

/* a single tokenised span emitted by the .pds tokenizer */
interface Token {
  c: string;
  s: string;
}

/* one-shot scroll trigger: fires fn once when el crosses ratio of the viewport */
interface OneShot {
  el: Element;
  ratio: number;
  fired: boolean;
  fn: () => void;
}

/* always-run watcher, invoked every scroll check with the viewport height */
type Watcher = (vh: number) => void;

/* zero-arg trigger keyed by hero line index */
type LineTrigger = () => void;

/* a sequence-diagram lane (vertical actor lifeline) */
interface SeqLane {
  x: number;
  label: string;
  kind: 'person' | 'component' | 'container';
}

/* a sequence-diagram message: [from lane, to lane, y, label, kind] */
type SeqMessage = [number, number, number, string, 'call' | 'ret'];

const H = pdsHighlight;
const T = pdsTokenize;

export function initLanding(): () => void {
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    || document.body.classList.contains('no-motion');

  function $all(s: string, r?: ParentNode | null): Element[] { return Array.prototype.slice.call((r || document).querySelectorAll(s)); }
  function gutter(el: Element | null, n: number): void {
    if (!el) return;
    let s = '';
    for (let i = 1; i <= n; i++) s += '<div>' + i + '</div>';
    el.innerHTML = s;
  }
  function top(el: Element): number { return el.getBoundingClientRect().top; }

  /* registry of one-shot scroll triggers + always-run watchers */
  const oneShots: OneShot[] = [];   // { el, ratio, fired, fn }
  const watchers: Watcher[] = [];   // function()  (run every check)
  function onScroll(): void {
    const vh = window.innerHeight || document.documentElement.clientHeight;
    for (let i = 0; i < oneShots.length; i++) {
      const t = oneShots[i];
      if (!t.fired && top(t.el) < vh * t.ratio) { t.fired = true; t.fn(); }
    }
    for (let j = 0; j < watchers.length; j++) watchers[j](vh);
  }

  /* ---- source snippets --------------------------------------------------- */
  const HERO_SRC = [
    '//! context — who uses the platform and',
    '//! what it integrates with.',
    '',
    'public person Attendee {',
    '  /// Hold seats once admitted.',
    '  public hold(req: ReserveRequest): void {',
    '    gateway::ReservationApi.reserve(req)',
    '  }',
    '}',
    '',
    'public person Organizer;',
    '',
    'public system AcmeTickets;',
    '',
    'public system Payments {',
    '  public authorize(amt: Money): Result<Auth, Error>;',
    '}',
    '',
    'public system Notifications;'
  ].join('\n');

  const DESCRIBE_SRC = [
    '//! inventory — finite seat pools and time-boxed',
    '//! holds. The no-oversell guarantee lives here.',
    '',
    '/// A finite pool of seats for one price tier.',
    '/// Allocation is atomic: two requests for the',
    '/// last seat cannot both succeed.',
    'public container Pool {',
    '  /// Take `n` seats, or fail if the pool',
    '  /// cannot cover it.',
    '  public allocate(tier: TierId, n: number): Result<Hold, SoldOut> {',
    '    if remaining(tier) < n {',
    '      return Err(SoldOut)',
    '    }',
    '    return Ok(reserve(tier, n))',
    '  }',
    '}',
    '',
    '/// A time-boxed claim, released if checkout stalls.',
    'public data Hold { id: HoldId, seats: number }'
  ].join('\n');

  const IDE_SRC = [
    '//! context — the C4 system context.',
    '',
    'public person Attendee {',
    '  #[manual]',
    '  public hold(req: ReserveRequest): void {',
    '    gateway::ReservationApi.reserve(req)',
    '  }',
    '}',
    '',
    '/// ACME Tickets — the ticketing platform.',
    '/// #headline',
    'public system AcmeTickets;',
    '',
    'public system PaymentProvider {',
    '  public authorize(amt: Money): Result<Auth, Error>;',
    '}'
  ].join('\n');

  /* ---- static highlighted panels ----------------------------------------- */
  function paintStatic(codeId: string, gutId: string, src: string): void {
    const code = document.getElementById(codeId);
    if (!code) return;
    code.innerHTML = H(src);
    gutter(document.getElementById(gutId), src.split('\n').length);
  }
  paintStatic('describe-code', 'describe-gutter', DESCRIBE_SRC);
  paintStatic('ide-code', 'ide-gutter', IDE_SRC);

  /* ---- manifest (pds.toml) ----------------------------------------------- */
  (function manifest(): void {
    const el = document.getElementById('manifest');
    if (!el) return;
    const rows = [
      '<span class="tag">[workspace]</span>',
      '<span class="var">name</span>    <span class="punct">=</span> <span class="str">"acme-tickets"</span>',
      '<span class="var">version</span> <span class="punct">=</span> <span class="str">"1.4.0"</span>',
      '<span class="var">modules</span> <span class="punct">=</span> <span class="punct">[</span><span class="str">"context"</span>, <span class="str">"inventory"</span>,',
      '           <span class="str">"orders"</span>, <span class="str">"payments"</span><span class="punct">]</span>',
      '',
      '<span class="tag">[dependencies]</span>',
      '<span class="var">acme/inventory</span> <span class="punct">=</span> <span class="str">"1.4.0"</span>',
      '<span class="var">std/money</span>      <span class="punct">=</span> <span class="str">"2.1.0"</span>',
      '<span class="var">c4/patterns</span>    <span class="punct">=</span> <span class="str">"0.9.2"</span>'
    ];
    el.innerHTML = rows.join('\n');
  })();

  /* ---- hero typing + diagram assembly ------------------------------------ */
  (function hero(): void {
    const code = document.getElementById('hero-code');
    const gut = document.getElementById('hero-gutter');
    const status = document.getElementById('hero-status');
    const stext = status ? status.querySelector('.stext') : null;
    const stage = document.getElementById('hero-stage');
    const term = document.getElementById('hero-terminal');
    if (!code || !stage || !term) return;

    const nodes = $all('.c4-node', stage);
    const edges = $all('.edge', document.getElementById('hero-edges'));

    function setStatus(state: string, text: string): void { if (status) status.className = 'status ' + state; if (stext) stext.textContent = text; }
    function showNode(i: number): void { if (nodes[i]) nodes[i].classList.add('in'); }
    function drawEdge(i: number): void { const e = edges[i] as SVGElement | undefined; if (!e) return; e.style.transition = 'stroke-dashoffset .7s ease'; e.style.strokeDashoffset = '0'; }

    const triggers: Record<number, LineTrigger> = {
      3: function () { showNode(0); },
      10: function () { showNode(1); },
      12: function () { showNode(2); drawEdge(0); drawEdge(1); },
      14: function () { showNode(3); drawEdge(2); },
      18: function () { showNode(4); drawEdge(3); }
    };
    const lines = HERO_SRC.split('\n');

    function renderInstant(): void {
      code!.innerHTML = H(HERO_SRC);
      gutter(gut, lines.length);
      Object.keys(triggers).forEach(function (k) { triggers[Number(k)](); });
      setStatus('ok', 'well-formed');
    }
    if (reduce) { renderInstant(); return; }

    let li = 0, html = '';
    function typeLine(): void {
      if (li >= lines.length) { code!.innerHTML = html; setStatus('ok', 'well-formed'); return; }
      const toks: Token[] = T(lines[li]);
      let ti = 0, lineHtml = '';
      function step(): void {
        if (ti < toks.length) {
          const tk = toks[ti++];
          const esc = tk.s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
          lineHtml += tk.c === 'ws' ? esc : '<span class="' + tk.c + '">' + esc + '</span>';
          code!.innerHTML = html + '<span class="ln">' + lineHtml + '<span class="cursor"></span>\n</span>';
          setTimeout(step, 14 + Math.random() * 22);
        } else {
          html += '<span class="ln">' + lineHtml + '\n</span>';
          gutter(gut, li + 1);
          if (triggers[li + 1]) triggers[li + 1]();
          li++;
          setTimeout(typeLine, lines[li - 1] === '' ? 40 : 90);
        }
      }
      if (toks.length === 0) { code!.innerHTML = html + '<span class="ln"><span class="cursor"></span>\n</span>'; setTimeout(step, 60); }
      else { step(); }
    }
    oneShots.push({ el: term, ratio: 0.85, fired: false, fn: function () {
      setStatus('busy', 'analyzing…');
      setTimeout(typeLine, 350);
    } });
  })();

  /* ---- refine: diagnostics resolving ------------------------------------- */
  (function refine(): void {
    const box = document.getElementById('refine-visual');
    if (!box) return;
    const items = $all('#problems li');
    const count = document.getElementById('prob-count');
    const well = document.getElementById('well-formed');
    function resolveSeq(): void {
      if (reduce) {
        items.forEach(function (li) { li.classList.add('resolved'); });
        if (count) count.textContent = '0 problems';
        if (well) well.classList.add('show');
        return;
      }
      [0, 1, 2].forEach(function (idx, k) {
        setTimeout(function () {
          if (items[idx]) items[idx].classList.add('resolved');
          const left = items.length - (k + 1);
          if (count) count.textContent = left + (left === 1 ? ' problem' : ' problems');
          if (k === 2 && well) setTimeout(function () { well.classList.add('show'); }, 250);
        }, 600 + k * 650);
      });
    }
    oneShots.push({ el: box, ratio: 0.7, fired: false, fn: resolveSeq });
  })();

  /* ---- generate: sequence diagram ---------------------------------------- */
  (function sequence(): void {
    const svg = document.getElementById('seq-svg');
    if (!svg) return;
    const NS = 'http://www.w3.org/2000/svg';
    const col: Record<SeqLane['kind'], string> = { person: 'var(--k-person)', component: 'var(--k-component)', container: 'var(--k-container)' };
    const lanes: SeqLane[] = [
      { x: 55, label: 'Attendee', kind: 'person' },
      { x: 185, label: 'Checkout', kind: 'component' },
      { x: 305, label: 'inventory', kind: 'container' },
      { x: 420, label: 'payments', kind: 'container' }
    ];
    function mk(tag: string, attrs: Record<string, string | number>): SVGElement { const e = document.createElementNS(NS, tag) as SVGElement; for (const k in attrs) e.setAttribute(k, String(attrs[k])); return e; }
    function text(x: number, y: number, str: string, cls: string): SVGElement { const t = mk('text', { x: x, y: y, 'text-anchor': 'middle', class: cls }); t.textContent = str; return t; }

    const style = mk('style', {});
    style.textContent =
      '.seq-head{font-family:var(--font-mono);font-size:10px;font-weight:600;fill:var(--ink);}' +
      '.seq-lbl{font-family:var(--font-mono);font-size:9.5px;fill:var(--ink-soft);}' +
      '.seq-lbl.ret{fill:var(--seq-ok);}' +
      '.seq-msg{opacity:0;transition:opacity .4s ease;}.seq-msg.in{opacity:1;}';
    svg.appendChild(style);

    const defs = mk('defs', {});
    [['seqarrow', 'var(--ink-soft)'], ['seqarrowok', 'var(--seq-ok)']].forEach(function (a) {
      const m = mk('marker', { id: a[0], viewBox: '0 0 10 10', refX: '8', refY: '5', markerWidth: '6', markerHeight: '6', orient: 'auto-start-reverse' });
      m.appendChild(mk('path', { d: 'M0 0 L10 5 L0 10 z', fill: a[1] }));
      defs.appendChild(m);
    });
    svg.appendChild(defs);

    const topY = 24, botY = 286;
    lanes.forEach(function (l) {
      const w = 96, g = mk('g', {});
      g.appendChild(mk('rect', { x: l.x - w / 2, y: 8, width: w, height: 26, rx: 6, fill: 'var(--surface-2)', stroke: 'var(--line-strong)' }));
      g.appendChild(mk('rect', { x: l.x - w / 2, y: 8, width: 3, height: 26, fill: col[l.kind] }));
      g.appendChild(text(l.x + 2, 25, l.label, 'seq-head'));
      g.appendChild(mk('line', { x1: l.x, y1: topY + 10, x2: l.x, y2: botY, stroke: 'var(--line)', 'stroke-dasharray': '3 4' }));
      svg.appendChild(g);
    });
    function lx(i: number): number { return lanes[i].x; }
    const msgs: SeqMessage[] = [
      [0, 1, 64, 'checkout(req)', 'call'],
      [1, 2, 96, 'allocate(n)', 'call'],
      [2, 1, 124, 'Ok(Hold)', 'ret'],
      [1, 3, 156, 'authorize(amt)', 'call'],
      [3, 1, 184, 'Ok(Auth)', 'ret'],
      [1, 2, 216, 'commit(hold)', 'call'],
      [1, 0, 252, 'Confirmation', 'ret']
    ];
    const groups: SVGElement[] = [];
    msgs.forEach(function (m) {
      const x1 = lx(m[0]), x2 = lx(m[1]), y = m[2], ret = m[4] === 'ret';
      const g = mk('g', { class: 'seq-msg' });
      const ln = mk('line', { x1: x1, y1: y, x2: x2, y2: y, stroke: ret ? 'var(--seq-ok)' : 'var(--ink-soft)', 'stroke-width': '1.5', 'marker-end': ret ? 'url(#seqarrowok)' : 'url(#seqarrow)' });
      if (ret) ln.setAttribute('stroke-dasharray', '4 3');
      g.appendChild(ln);
      g.appendChild(text((x1 + x2) / 2, y - 6, m[3], ret ? 'seq-lbl ret' : 'seq-lbl'));
      svg.appendChild(g);
      groups.push(g);
    });
    function play(): void {
      if (reduce) { groups.forEach(function (g) { g.classList.add('in'); }); return; }
      groups.forEach(function (g, i) { setTimeout(function () { g.classList.add('in'); }, 250 + i * 320); });
    }
    oneShots.push({ el: svg, ratio: 0.6, fired: false, fn: play });
  })();

  /* ---- convergence core pulse -------------------------------------------- */
  (function core(): void {
    const c = document.getElementById('core-mark');
    if (!c || reduce) return;
    watchers.push(function (vh: number) {
      const r = c.getBoundingClientRect();
      c.classList.toggle('live', r.top < vh * 0.8 && r.bottom > vh * 0.2);
    });
  })();

  /* ---- scroll reveals ----------------------------------------------------- */
  (function reveals(): void {
    const els = $all('.reveal');
    if (reduce) { els.forEach(function (el) { el.classList.add('in'); }); return; }
    els.forEach(function (el) {
      oneShots.push({ el: el, ratio: 0.92, fired: false, fn: function () { el.classList.add('in'); } });
    });
  })();

  /* ---- drive triggers ----------------------------------------------------- */
  let ticking = false;
  function requestCheck(): void {
    if (ticking) return; ticking = true;
    window.requestAnimationFrame(function () { ticking = false; onScroll(); });
  }
  window.addEventListener('scroll', requestCheck, { passive: true });
  window.addEventListener('resize', requestCheck);
  // initial passes (cover late layout / font load)
  onScroll();
  const t1 = setTimeout(onScroll, 60);
  const t2 = setTimeout(onScroll, 300);
  window.addEventListener('load', onScroll);

  // watchdog: if entrance transitions never advance (throttled/background
  // contexts), snap any already-revealed-but-still-invisible element visible
  // so above-the-fold content can never be stuck blank on first paint.
  const t3 = setTimeout(function () {
    $all('.reveal.in').forEach(function (el) {
      const he = el as HTMLElement;
      if (getComputedStyle(he).opacity === '0') { he.style.transition = 'none'; he.style.opacity = '1'; he.style.transform = 'none'; }
    });
  }, 600);

  return function teardown(): void {
    window.removeEventListener('scroll', requestCheck);
    window.removeEventListener('resize', requestCheck);
    window.removeEventListener('load', onScroll);
    clearTimeout(t1); clearTimeout(t2); clearTimeout(t3);
  };
}
