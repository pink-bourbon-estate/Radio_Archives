/* BBC Radio 6 Music Collection - the whole front end, in one plain file (no frameworks, no build step,
   nothing loaded from other sites).

   How it works, in short:
   - The data are JSON files in ./data, built by `bbc6 site` (see src/site.rs).
   - The address bar's path chooses the page ("/shows/", "/artist/b/<id>/"). Most pages also exist
     as plain pre-built HTML (see src/pages.rs) so search engines can read them; this script then
     replaces that with the interactive version. Pages without a pre-built copy come through
     404.html and are drawn from the data here.
   - Links are written in the code as "#/shows" and turned into real paths by h().
   - route() reads the path, fetches the JSON it needs, and draws the page into <main>.
   - dataTable() and barChart() are the two reusable pieces. Both can download what they show.
*/
(() => {
  'use strict';

  const NOGENRES = '{{NOGENRES}}' === '1';
  const REPO = 'https://github.com/pink-bourbon-estate/bbc-radio-6-music-collection';
  const main = document.getElementById('main');
  /* the folder the site lives in, e.g. "/" or "/BBC-Radio-6-Music-Collection/" */
  const BASE = new URL(document.currentScript.src).pathname.replace(/assets\/app\.js$/, '');
  /** "#/show/x" -> "/<base>/show/x/" */
  const toPath = (v) => { const r = v.slice(2); return BASE + (r && !r.endsWith('/') ? r + '/' : r); };

  /* ------------------------------------------------------------ helpers */

  /** Make an element: h('a', {href:'#/x', class:'btn'}, 'text', childNode) */
  function h(tag, attrs, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (v === false || v == null) continue;
      if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
      else if (k === 'class') el.className = v;
      else el.setAttribute(k, k === 'href' && typeof v === 'string' && v.startsWith('#/') ? toPath(v) : v === true ? '' : v);
    }
    for (const kid of kids.flat()) {
      if (kid == null || kid === false) continue;
      el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
    }
    return el;
  }

  const cache = new Map();
  /** Fetch a JSON file from ./data once, then remember it. */
  function getJSON(path) {
    if (!cache.has(path)) {
      const p = fetch(BASE + 'data/' + path).then((r) => {
        if (!r.ok) throw new Error(path + ' (' + r.status + ')');
        return r.json();
      });
      p.catch(() => cache.delete(path));
      cache.set(path, p);
    }
    return cache.get(path);
  }

  const nf = new Intl.NumberFormat('en-GB');
  const fmt = (n) => nf.format(n);
  const fmtDate = (iso) =>
    new Date(iso + 'T12:00:00Z').toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' });
  const artistHref = (letter, id) => '#/artist/' + letter + '/' + encodeURIComponent(id);
  const artistLink = (name, letter, id) => (id ? h('a', { href: artistHref(letter, id) }, name) : name);
  const mbLink = (mbid) => h('a', { href: 'https://musicbrainz.org/artist/' + mbid, rel: 'noopener', 'aria-label': 'View on MusicBrainz' }, 'MB');
  /** Plain search links for a track. They are ordinary links: nothing from those sites is loaded. */
  const listenLinks = (artist, title) => {
    const q = encodeURIComponent((artist + ' ' + title).trim());
    const lk = (name, short, url) => h('a', { class: 'lk', href: url, rel: 'noopener noreferrer nofollow', title: 'Search ' + name, 'aria-label': 'Search ' + name + ' for ' + artist + ' - ' + title }, short);
    return h('span', { class: 'listen' }, lk('YouTube', 'YT', 'https://www.youtube.com/results?search_query=' + q), ' ', lk('Spotify', 'SP', 'https://open.spotify.com/search/' + q),
      ' ', lk('SoundCloud', 'SC', 'https://soundcloud.com/search?q=' + q), ' ', lk('Bandcamp', 'BC', 'https://bandcamp.com/search?q=' + q));
  };
  const bbcEpisode = (pid) => (pid ? h('a', { href: 'https://www.bbc.co.uk/programmes/' + encodeURIComponent(pid), rel: 'noopener' }, 'This episode on the BBC') : null);
  /** A playlist download (artist, title) plus links to the transfer services that can import it. */
  function playlistTools(name, pairs) {
    // Tune My Music's own column names, with the playlist name on every row (it reads rows without one as "playlist not found").
    const get = () => ({ head: ['Track name', 'Artist name', 'Playlist name'], rows: pairs.map((p) => [p[1], p[0], name]) });
    const save = () => { const d = get(); saveBlob(name + '-playlist.csv', new Blob([toCSV(d.head, d.rows)], { type: 'text/csv;charset=utf-8' })); };
    return h('p', { class: 'playlist' },
      h('button', { type: 'button', onclick: save, 'aria-label': 'Download this tracklist as a playlist CSV' }, 'Playlist CSV'),
      ' then import it at ', h('a', { href: 'https://www.tunemymusic.com/', rel: 'noopener noreferrer' }, 'Tune My Music'), ' or ', h('a', { href: 'https://soundiiz.com/', rel: 'noopener noreferrer' }, 'Soundiiz'),
      ' to make a playlist in the service you use.');
  }
  const strip = (s) => String(s || '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

  /** Save text as a file in the visitor's browser. */
  function saveFile(name, text, type) {
    // A byte-order mark makes Excel read the CSV as UTF-8, so accented names stay right.
    const body = type === 'text/csv' ? ['\ufeff', text] : [text];
    saveBlob(name, new Blob(body, { type: type + ';charset=utf-8' }));
  }
  function saveBlob(name, blob) {
    if (navigator.msSaveOrOpenBlob) return navigator.msSaveOrOpenBlob(blob, name);
    const url = URL.createObjectURL(blob);
    const a = h('a', { href: url, download: name, rel: 'noopener', style: 'display:none' });
    document.body.append(a);
    a.click();
    // Some browsers (Safari, Firefox) need the link and URL to outlive the click, so tidy up later.
    setTimeout(() => { a.remove(); URL.revokeObjectURL(url); }, 60000);
  }
  const csvCell = (v) => '"' + String(v ?? '').replace(/"/g, '""') + '"';
  const toCSV = (head, rows) => [head, ...rows].map((r) => r.map(csvCell).join(',')).join('\r\n') + '\r\n';
  const toTXT = (head, rows) => {
    const all = [head, ...rows].map((r) => r.map((c) => String(c ?? '')));
    const w = head.map((_, i) => Math.min(48, Math.max(...all.map((r) => r[i].length))));
    return all.map((r) => r.map((c, i) => (i === r.length - 1 ? c : c.padEnd(w[i]))).join('  ')).join('\n') + '\n';
  };
  /** The pair of download buttons that every table and chart gets. */
  function downloadButtons(filename, getData) {
    const go = (kind) => () => {
      const { head, rows } = getData();
      if (kind === 'csv') saveFile(filename + '.csv', toCSV(head, rows), 'text/csv');
      else if (kind === 'txt') saveFile(filename + '.txt', toTXT(head, rows), 'text/plain');
      else saveBlob(filename + '.html', new Blob(csvToHTMLParts(toCSV(head, rows), filename), { type: 'text/html;charset=utf-8' }));
    };
    return ['csv', 'txt', 'html'].map((k) => h('button', { type: 'button', onclick: go(k), 'aria-label': 'Download this table as ' + k.toUpperCase() }, k.toUpperCase()));
  }

  /* ------------------------------------------------------------ table */

  /**
   * A sortable, filterable table that can download itself.
   *   cols: [{label, value(row) -> text or number (used to sort, filter and download),
   *           cell(row) -> Node (optional, how it looks), num: true for right-aligned numbers}]
   */
  function dataTable({ cols, rows, filename, pageSize = 100, rank = true, sortCol = null, sortDir = 'desc', filter = '', caption }) {
    let sortIdx = sortCol, dir = sortDir, q = filter, limit = pageSize;
    const wrap = h('div');
    const count = h('span', { class: 'count', 'aria-live': 'polite' });
    const search = h('input', { type: 'search', placeholder: 'Filter this table', 'aria-label': 'Filter this table', value: q });
    const tbl = h('table');
    const more = h('div', { class: 'more-row' });
    let view = [];

    const sortedView = () => {
      let v = rows;
      if (q.trim()) {
        const words = strip(q).split(/\s+/).filter(Boolean);
        v = v.filter((r) => {
          const hay = strip(cols.map((c) => c.value(r)).join(' '));
          return words.every((w) => hay.includes(w));
        });
      }
      if (sortIdx != null) {
        const c = cols[sortIdx], s = dir === 'asc' ? 1 : -1;
        v = [...v].sort((a, b) => {
          const x = c.value(a), y = c.value(b);
          return (typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), 'en', { sensitivity: 'base', numeric: true })) * s;
        });
      }
      return v;
    };

    function draw() {
      view = sortedView();
      count.textContent = fmt(view.length) + (view.length === 1 ? ' row' : ' rows') + (view.length !== rows.length ? ' (of ' + fmt(rows.length) + ')' : '');
      const head = h('tr', {}, rank ? h('th', { scope: 'col', class: 'n' }, '#') : null,
        cols.map((c, i) => h('th', { scope: 'col', class: c.num ? 'n' : '', 'aria-sort': sortIdx === i ? (dir === 'asc' ? 'ascending' : 'descending') : false },
          c.nodl ? c.label : h('button', { type: 'button', onclick: () => { dir = sortIdx === i ? (dir === 'asc' ? 'desc' : 'asc') : (c.num ? 'desc' : 'asc'); sortIdx = i; limit = pageSize; draw(); } }, c.label))));
      const body = view.slice(0, limit).map((r, n) => h('tr', {}, rank ? h('td', { class: 'n rank' }, n + 1) : null,
        cols.map((c) => h('td', { class: c.num ? 'n' : '' }, c.cell ? c.cell(r) : c.num && typeof c.value(r) === 'number' ? fmt(c.value(r)) : c.value(r)))));
      tbl.replaceChildren(...[caption ? h('caption', {}, caption) : null, h('thead', {}, head), h('tbody', {}, body)].filter(Boolean));
      more.replaceChildren(view.length > limit ? h('button', { type: 'button', onclick: () => { limit += 500; draw(); } }, 'Show more (' + fmt(Math.min(500, view.length - limit)) + ')') : null);
    }
    search.addEventListener('input', () => { q = search.value; limit = pageSize; draw(); });
    const dcols = cols.filter((c) => !c.nodl);
    const dl = downloadButtons(filename, () => ({ head: dcols.map((c) => c.label), rows: view.map((r) => dcols.map((c) => c.value(r))) }));
    draw();
    wrap.append(h('div', { class: 'tools' }, search, count, dl), h('div', { class: 'tablewrap' }, tbl), more);
    return wrap;
  }

  /** Column sets used in many places. */
  const trackCols = (offset = 0) => [
    { label: 'Artist', value: (r) => r[0], cell: (r) => artistLink(r[0], r[4], r[3]) },
    { label: 'Track', value: (r) => r[1] },
    { label: 'Plays', value: (r) => r[2], num: true },
    { label: 'Listen', value: () => '', cell: (r) => listenLinks(r[0], r[1]), nodl: true },
  ];
  const artistRowCols = [
    { label: 'Artist', value: (r) => r[0], cell: (r) => artistLink(r[0], r[2], r[1]) },
    { label: 'Plays', value: (r) => r[3], num: true },
  ];

  /* ------------------------------------------------------------ charts */

  /** Vertical bar chart drawn as SVG. data: [{label, value, href?}] */
  function barChart({ data, title, filename, unit = 'plays', labelEvery = 1, onPick }) {
    const W = 760, H = 240, L = 46, B = 28, T = 10;
    const max = Math.max(...data.map((d) => d.value), 1);
    const nice = (m) => { const p = 10 ** Math.floor(Math.log10(m)); return [1, 2, 4, 8, 10].map((x) => x * p).find((v) => v >= m); };
    const top = nice(max);
    const bw = (W - L) / data.length;
    const ns = 'http://www.w3.org/2000/svg';
    const el = (n, a = {}, t) => { const e = document.createElementNS(ns, n); for (const k in a) e.setAttribute(k, a[k]); if (t != null) e.textContent = t; return e; };
    const svg = el('svg', { viewBox: `0 0 ${W} ${H}`, role: 'img', 'aria-label': title });
    for (let i = 0; i <= 4; i++) {
      const y = T + (H - T - B) * (1 - i / 4);
      svg.append(el('line', { x1: L, x2: W, y1: y, y2: y }), el('text', { x: L - 6, y: y + 4, 'text-anchor': 'end' }, fmt(Math.round((top * i) / 4))));
    }
    data.forEach((d, i) => {
      const bh = ((H - T - B) * d.value) / top;
      const x = L + i * bw + bw * 0.12;
      const bar = el('rect', { class: 'bar' + (d.sel ? ' sel' : ''), x, y: H - B - bh, width: bw * 0.76, height: Math.max(bh, 0) });
      const tip = `${d.tip || d.label}: ${fmt(d.value)} ${unit}` + (onPick ? ' (click to select)' : '');
      if (onPick) {
        // The whole column is clickable, so even a tiny bar is easy to hit; it also works from the keyboard.
        const col = el('g', { class: 'col', tabindex: 0, role: 'button', 'aria-label': tip });
        col.append(el('rect', { class: 'hit', x: L + i * bw, y: T, width: bw, height: H - T - B }), bar, el('title', {}, tip));
        col.addEventListener('click', () => onPick(i));
        col.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onPick(i); } });
        svg.append(col);
      } else {
        bar.append(el('title', {}, tip));
        svg.append(bar);
      }
      if (i % labelEvery === 0) svg.append(el('text', { x: x + bw * 0.38, y: H - 9, 'text-anchor': 'middle' }, d.label));
    });
    const dl = downloadButtons(filename, () => ({ head: ['Label', unit], rows: data.map((d) => [d.label, d.value]) }));
    return h('figure', { class: 'chart' }, h('div', { class: 'tools' }, h('span', { class: 'count' }, title), dl), svg);
  }

  /** A list with a proportional bar behind each row (for shows, genres, countries). */
  function barList({ items, filename, head }) {
    const max = Math.max(...items.map((i) => i.value), 1);
    const list = h('ol', { class: 'bars' }, items.map((i) => h('li', {}, h('span', {}, i.href ? h('a', { href: i.href }, i.label) : i.label),
      h('span', { class: 'fill-cell' }, h('div', { class: 'fill', style: `width:${(i.value / max) * 100}%` })), h('span', { class: 'v' }, fmt(i.value)))));
    return h('div', {}, h('div', { class: 'tools' }, h('span', { class: 'count' }), downloadButtons(filename, () => ({ head, rows: items.map((i) => [i.label, i.value]) }))), list);
  }

  /* ------------------------------------------------------------ date explorer: zoom and click-to-filter */

  const GRAINS = [['day', 'Day'], ['week', 'Week'], ['month', 'Month'], ['quarter', 'Quarter'], ['year', 'Year']];
  const WINDOW = { day: 60, week: 52, month: 36, quarter: 24, year: 60 };
  const FINER = { year: 'month', quarter: 'month', month: 'day', week: 'day' };
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const MONTHS_LONG = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  const p2 = (n) => String(n).padStart(2, '0');
  /** Day numbers count days since 1970-01-01 (UTC), so there are no time-zone surprises. */
  const dnum = (y, m, d) => Math.floor(Date.UTC(y, m, d) / 864e5);
  const dymd = (n) => { const d = new Date(n * 864e5); return [d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()]; };
  const dnumIso = (iso) => dnum(+iso.slice(0, 4), +iso.slice(5, 7) - 1, +iso.slice(8, 10));
  const dIso = (n) => { const [y, m, d] = dymd(n); return y + '-' + p2(m + 1) + '-' + p2(d); };
  const dLong = (n) => { const [y, m, d] = dymd(n); return d + ' ' + MONTHS[m] + ' ' + y; };

  /** The day, week (Monday to Sunday), month, quarter or year that contains day number n. */
  function periodOf(n, g) {
    const [y, m, d] = dymd(n);
    let from = n, to = n, label, key;
    if (g === 'day') { label = dLong(n); key = dIso(n); }
    else if (g === 'week') { from = n - ((n + 3) % 7 + 7) % 7; to = from + 6; label = 'Week of ' + dLong(from); key = 'w' + dIso(from); }
    else if (g === 'month') { from = dnum(y, m, 1); to = dnum(y, m + 1, 1) - 1; label = MONTHS_LONG[m] + ' ' + y; key = y + '-' + p2(m + 1); }
    else if (g === 'quarter') { const q = Math.floor(m / 3); from = dnum(y, q * 3, 1); to = dnum(y, q * 3 + 3, 1) - 1; label = 'Q' + (q + 1) + ' ' + y; key = y + '-Q' + (q + 1); }
    else { from = dnum(y, 0, 1); to = dnum(y + 1, 0, 1) - 1; label = String(y); key = String(y); }
    return { grain: g, from, to, label, key };
  }
  /** Read a period back from its short key, e.g. 2024-03, 2024-Q1, 2024, 2024-03-05, w2024-03-04. */
  function periodFromKey(k) {
    let m;
    if ((m = /^w(\d{4})-(\d\d)-(\d\d)$/.exec(k))) return periodOf(dnum(+m[1], +m[2] - 1, +m[3]), 'week');
    if ((m = /^(\d{4})-(\d\d)-(\d\d)$/.exec(k))) return periodOf(dnum(+m[1], +m[2] - 1, +m[3]), 'day');
    if ((m = /^(\d{4})-Q([1-4])$/.exec(k))) return periodOf(dnum(+m[1], (+m[2] - 1) * 3, 1), 'quarter');
    if ((m = /^(\d{4})-(\d\d)$/.exec(k))) return periodOf(dnum(+m[1], +m[2] - 1, 1), 'month');
    if ((m = /^(\d{4})$/.exec(k))) return periodOf(dnum(+m[1], 0, 1), 'year');
    return null;
  }
  const nextStart = (p) => p.to + 1;
  const barLabel = (p) => {
    const [y, m, d] = dymd(p.from);
    return p.grain === 'day' || p.grain === 'week' ? d + ' ' + MONTHS[m] : p.grain === 'month' ? MONTHS[m] + ' ' + String(y).slice(2) : p.grain === 'quarter' ? p.label.replace(/ 20/, ' ') : String(y);
  };

  /** Plays per day for the whole archive (or one show) as { start: day number, d: [counts] }. */
  const dailyAll = async () => { const j = await getJSON('daily.json'); return { start: dnumIso(j.start), d: j.d }; };
  async function dailyShow(slug) {
    const [j, all] = await Promise.all([getJSON('show_daily.json'), getJSON('daily.json')]);
    const out = new Array(all.d.length).fill(0), flat = j.shows[slug] || [];
    for (let i = 0; i < flat.length; i += 2) out[flat[i]] = flat[i + 1];
    return { start: dnumIso(j.start), d: out };
  }

  /** Add up every play in [from, to] by fetching only the month files that overlap it. */
  async function totals(from, to, daily, slug) {
    const t = new Map(), a = new Map(), s = new Map(), weekday = new Array(7).fill(0), hour = new Array(24).fill(0);
    let plays = 0;
    const last = daily.start + daily.d.length - 1;
    const lo = Math.max(from, daily.start), hi = Math.min(to, last);
    const files = [];
    for (let n = lo; n <= hi;) { const [y, m] = dymd(n); files.push([y, m]); n = dnum(y, m + 1, 1); }
    const shards = await Promise.all(files.map(([y, m]) => getJSON('month/' + y + '-' + p2(m + 1) + '.json').then((j) => [y, m, j], () => null)));
    for (const sh of shards) {
      if (!sh) continue;
      const [y, m, j] = sh, p = j.p;
      for (let i = 0; i < p.length; i += 4) {
        const n = dnum(y, m, p[i]);
        if (n < from || n > to) continue;
        const sv = j.s[p[i + 2]];
        if (slug && sv[1] !== slug) continue;
        const tv = j.t[p[i + 1]], av = tv[0] >= 0 ? j.a[tv[0]] : null;
        const tk = (av ? av[0] : '') + '\u0001' + tv[1];
        const te = t.get(tk) || t.set(tk, { r: [av ? av[0] : '', tv[1], 0, av ? av[1] : '', av ? av[2] : ''] }).get(tk);
        te.r[2]++;
        if (av) { const ae = a.get(av[0]) || a.set(av[0], { r: [av[0], av[1], av[2], 0] }).get(av[0]); ae.r[3]++; }
        const se = s.get(sv[1]) || s.set(sv[1], { r: [sv[0], sv[1], 0] }).get(sv[1]);
        se.r[2]++;
        weekday[((n + 3) % 7 + 7) % 7]++;
        hour[p[i + 3]]++;
        plays++;
      }
    }
    const rank = (m, k) => [...m.values()].map((x) => x.r).sort((x, y) => y[k] - x[k] || String(x[0]).localeCompare(String(y[0])));
    return { plays, tracks: rank(t, 2), artists: rank(a, 3), shows: rank(s, 2), weekday, hour };
  }

  /** Buckets of one grain that end at day `end` (at most `n` of them), with the plays in each. */
  function buckets(daily, grain, end, n) {
    const last = daily.start + daily.d.length - 1;
    let p = periodOf(Math.min(end, last), grain);
    const list = [];
    while (list.length < n && p.to >= daily.start) { list.unshift(p); p = periodOf(p.from - 1, grain); }
    return list.map((b) => {
      let v = 0;
      for (let d = Math.max(b.from, daily.start); d <= Math.min(b.to, last); d++) v += daily.d[d - daily.start];
      return { ...b, value: v, tip: b.label, label: barLabel(b) };
    });
  }

  /**
   * The zoom control, the chart and the panel that follows the clicked bar.
   *   opts.daily: { start, d }       plays per day
   *   opts.slug: limit everything to one show
   *   opts.onChange(sel|null): the page can show or hide its all-time sections
   * Returns { el: controls and chart, panel: the selected period's dashboard, select(key) }.
   */
  function explorer(opts) {
    const daily = opts.daily, last = daily.start + daily.d.length - 1;
    const st = { grain: opts.grain || 'month', end: opts.end || last, sel: null };
    const el = h('div', { class: 'explorer' }), panel = h('div', { class: 'selpanel', 'aria-live': 'polite' });
    const keepUrl = () => {
      const u = new URL(location.href);
      st.sel ? u.searchParams.set('period', st.sel.key) : u.searchParams.delete('period');
      history.replaceState(history.state, '', u);
    };
    const half = () => Math.floor(WINDOW[st.grain] / 2);
    /** The end of the period k periods after (or before, if k is negative) the one containing day n. */
    const step = (n, k) => { let p = periodOf(n, st.grain); for (let i = 0; i < Math.abs(k); i++) p = periodOf(k > 0 ? p.to + 1 : p.from - 1, st.grain); return p.to; };
    const reveal = (p) => { st.end = Math.min(last, step(p.to, half())); };

    function pick(p) {
      st.sel = p; st.grain = p.grain;
      keepUrl(); drawChart(); drawPanel(); opts.onChange && opts.onChange(p);
    }
    function clear() { st.sel = null; keepUrl(); drawChart(); drawPanel(); opts.onChange && opts.onChange(null); }

    function drawChart() {
      const list = buckets(daily, st.grain, st.end, WINDOW[st.grain]);
      const first = list[0] ? list[0].from : daily.start, lastB = list.length ? list[list.length - 1].to : last;
      const inSel = (b) => st.sel && ((b.from >= st.sel.from && b.to <= st.sel.to) || (st.sel.from >= b.from && st.sel.to <= b.to));
      const chart = barChart({
        title: 'Tracks played per ' + st.grain + (opts.slug ? ' on this show' : '') + ' (' + dLong(first) + ' to ' + dLong(Math.min(lastB, last)) + ')',
        filename: (opts.slug || 'plays') + '-per-' + st.grain, unit: 'plays', labelEvery: Math.ceil(list.length / 12),
        data: list.map((b) => ({ label: b.label, tip: b.tip, value: b.value, sel: inSel(b) })),
        onPick: (i) => pick(periodOf(list[i].from, st.grain)),
      });
      const seg = h('div', { class: 'seg', role: 'group', 'aria-label': 'Zoom: day, week, month, quarter or year' }, GRAINS.map(([g, name]) => h('button', {
        type: 'button', 'aria-pressed': String(st.grain === g),
        onclick: () => { st.grain = g; if (st.sel) reveal(st.sel); drawChart(); },
      }, name)));
      const nav = h('div', { class: 'seg' },
        h('button', { type: 'button', disabled: first <= daily.start, onclick: () => { st.end = Math.max(daily.start, step(lastB, -half())); drawChart(); } }, '‹ Earlier'),
        h('button', { type: 'button', disabled: lastB >= last, onclick: () => { st.end = Math.min(last, step(lastB, half())); drawChart(); } }, 'Later ›'));
      el.replaceChildren(h('div', { class: 'tools' }, seg, nav, st.sel ? h('button', { type: 'button', onclick: clear }, 'Clear: ' + st.sel.label + ' ×') : h('span', { class: 'meta' }, 'Click a bar to see just that ' + st.grain + '.')), chart);
    }

    let token = 0;
    async function drawPanel() {
      const sel = st.sel, me = ++token;
      if (!sel) return panel.replaceChildren();
      panel.replaceChildren(h('p', { class: 'meta' }, 'Loading ' + sel.label + ' ...'));
      let tot;
      try { tot = await totals(sel.from, sel.to, daily, opts.slug); } catch (e) { return panel.replaceChildren(h('p', { class: 'note' }, 'That period could not be loaded.')); }
      if (me !== token) return;
      const key = (opts.slug || 'all') + '-' + sel.key.replace(/[^0-9A-Za-z-]/g, '');
      const fin = FINER[sel.grain];
      const kids = [h('h2', {}, (opts.slug ? '' : 'Everything played in ') + sel.label), h('p', { class: 'meta' }, dLong(sel.from) + (sel.from === sel.to ? '' : ' to ' + dLong(Math.min(sel.to, last))))];
      kids.push(statStrip([[fmt(tot.plays), 'plays'], [fmt(tot.tracks.length), 'distinct tracks'], [fmt(tot.artists.length), 'artists'], opts.slug ? null : [fmt(tot.shows.length), 'shows']].filter(Boolean)));
      if (!tot.plays) { kids.push(h('p', { class: 'note' }, 'Nothing was played in this period.')); return panel.replaceChildren(...kids); }
      if (fin) {
        const sub = buckets(daily, fin, Math.min(sel.to, last), 400).filter((b) => b.from >= sel.from);
        kids.push(section('Zoom in: by ' + fin, barChart({
          title: 'Tracks played per ' + fin + ' in ' + sel.label, filename: key + '-per-' + fin, labelEvery: Math.ceil(sub.length / 14),
          data: sub.map((b) => ({ label: b.label, tip: b.tip, value: b.value })),
          onPick: (i) => { st.grain = fin; const p = periodOf(sub[i].from, fin); reveal(p); pick(p); },
        })));
      }
      kids.push(h('div', { class: 'two' },
        section('Most played tracks', dataTable({ cols: trackCols(), rows: tot.tracks.slice(0, 100), filename: key + '-tracks', sortCol: 2 })),
        section('Most played artists', dataTable({ cols: artistRowCols, rows: tot.artists.slice(0, 100), filename: key + '-artists', sortCol: 1 }))));
      if (!opts.slug) kids.push(section('Busiest shows', barList({ filename: key + '-shows', head: ['Show', 'Plays'], items: tot.shows.slice(0, 15).map((s) => ({ label: s[0], value: s[2], href: '#/show/' + s[1] })) })));
      const wk = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
      kids.push(h('div', { class: 'two' },
        sel.to - sel.from >= 6 ? section('Day of the week', barChart({ title: 'Tracks played by weekday in ' + sel.label, filename: key + '-weekday', data: wk.map((d, i) => ({ label: d, value: tot.weekday[i] })) })) : null,
        section('Time of day', barChart({ title: 'Tracks played by the hour the programme starts, in ' + sel.label, filename: key + '-hour', data: tot.hour.map((v, i) => ({ label: p2(i), value: v })) }))));
      panel.replaceChildren(...kids);
    }

    const fromUrl = opts.useUrl === false ? null : periodFromKey(new URLSearchParams(location.search).get('period') || '');
    if (fromUrl && fromUrl.to >= daily.start && fromUrl.from <= last) { st.sel = fromUrl; st.grain = fromUrl.grain; reveal(fromUrl); drawChart(); drawPanel(); opts.onChange && opts.onChange(fromUrl); }
    else drawChart();
    return { el, panel, select: (key) => { const p = periodFromKey(key); if (p) { st.grain = p.grain; reveal(p); pick(p); } }, clear };
  }

  /* ------------------------------------------------------------ small page parts */

  const section = (title, ...kids) => h('section', {}, h('h2', {}, title), ...kids);
  const statStrip = (items) => h('div', { class: 'stats' }, items.map(([v, l]) => h('div', { class: 'stat' }, h('b', {}, v), h('span', {}, l))));
  const tabs = (items, current) => h('nav', { class: 'tabs', 'aria-label': 'Sections' }, items.map(([href, label, key]) => h('a', { href, 'aria-current': key === current ? 'page' : false }, label)));

  /** An archive-style card: tag, title, place line, credit line. */
  function card({ tag, title, href, lines = [], list, more }) {
    return h('article', { class: 'card' }, h('div', { class: 'tag' }, tag),
      h('h3', {}, href ? h('a', { href }, title) : title),
      lines.map((l) => h('p', { class: 'meta' }, l)),
      list ? h('ol', {}, list) : null,
      more ? h('a', { class: 'more', href: more[0] }, more[1]) : null);
  }
  const trackItem = (r) => h('li', {}, artistLink(r[0], r[4], r[3]), ' - ', r[1], h('span', { class: 'meta' }, ' ×' + r[2]));

  function setPage(title, ...nodes) {
    document.title = (title ? title + ' - ' : '') + 'BBC Radio 6 Music Collection';
    main.replaceChildren(...nodes.flat().filter((n) => n != null && n !== false));
    window.scrollTo(0, 0);
  }

  /* ------------------------------------------------------------ pages */

  async function home() {
    const [meta, win, summary, latest, daily] = await Promise.all([getJSON('meta.json'), getJSON('windows.json'), getJSON('summary.json'), getJSON('latest.json'), dailyAll()]);
    let allTime = null;
    const ex = explorer({ daily, grain: 'month', onChange: (sel) => { if (allTime) allTime.hidden = !!sel; } });
    ex.picked = !!new URLSearchParams(location.search).get('period');
    const lastDay = latest.find((d) => d.episodes.length) || latest[0];
    const wcard = (k, tag) => card({
      tag, title: fmt(win[k].plays) + ' plays', href: '#/most-played/' + k,
      lines: [fmtDate(win[k].from) + (win[k].from === win[k].to ? '' : ' to ' + fmtDate(win[k].to)), fmt(win[k].episodes) + ' episodes · ' + fmt(win[k].distinct_artists) + ' artists'],
      list: win[k].tracks.slice(0, 5).map(trackItem), more: ['#/most-played/' + k, 'See the full chart'],
    });
    setPage('',
      h('h1', {}, 'Every track played on BBC Radio 6 Music.'),
      h('p', { class: 'lede' }, 'An archive of ' + fmt(meta.plays) + ' plays across ' + fmt(meta.shows) + ' shows, from ' + fmtDate(meta.first_day) + ' to ' + fmtDate(meta.as_of) + '. Updated every day.'),
      statStrip([[fmt(meta.plays), 'plays'], [fmt(meta.tracks), 'distinct tracks'], [fmt(meta.artists), 'artists'], [fmt(meta.shows), 'shows'], [fmt(meta.episodes), 'episodes']]),
      section('Explore by date', h('p', { class: 'meta' }, 'Zoom by day, week, month, quarter or year, then click a bar: everything below changes to show just that period.'), ex.el),
      ex.panel,
      h('div', { class: 'alltime', hidden: !!ex.picked }, ...[
      section('Played recently', h('p', { class: 'meta' }, 'The windows below end on the last day in the data (' + fmtDate(meta.as_of) + '), and are refreshed every day.'),
        h('div', { class: 'grid' }, wcard('today', 'Played on the last day'), wcard('week', 'Played this week'), wcard('month', 'Played this month'), wcard('year', 'Played this year'))),
      section('On the last day', h('p', { class: 'meta' }, lastDay ? fmtDate(lastDay.date) + ' · ' + lastDay.episodes.length + ' episodes with tracklists' : ''),
        h('div', { class: 'grid' }, (lastDay ? lastDay.episodes : []).slice(0, 8).map((e) => card({
          tag: e.time, title: e.show, href: '#/show/' + e.slug, lines: [e.title, fmt(e.tracks.length) + ' tracks'],
        }))), h('p', {}, h('a', { class: 'more', href: '#/latest' }, 'All episodes and tracklists from the last 7 days'))),
      section('Plays per year', barChart({ title: 'Tracks played per year', filename: 'plays-per-year', data: summary.by_year.map((y) => ({ label: String(y[0]), value: y[1] })), labelEvery: 2, onPick: (i) => ex.select(String(summary.by_year[i][0])) })),
      h('div', { class: 'two' },
        section('Busiest shows', barList({ filename: 'top-shows', head: ['Show', 'Plays'], items: summary.top_shows.slice(0, 15).map((s) => ({ label: s[0], value: s[2], href: '#/show/' + s[1] })) })),
        section('Day of the week', barChart({ title: 'Tracks played by weekday', filename: 'plays-by-weekday', data: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((d, i) => ({ label: d, value: summary.weekday[i] })) }))),
      section('Time of day', barChart({ title: 'Tracks played by the hour the programme starts', filename: 'plays-by-hour', data: summary.hour.map((v, i) => ({ label: String(i).padStart(2, '0'), value: v })) })),
      NOGENRES ? null : section('Genres (from MusicBrainz, use with care)',
        h('p', { class: 'note' }, 'Genre labels come from MusicBrainz user tags, and only about ' + summary.genre_coverage_pct + '% of plays belong to an artist that has one. Treat these as a rough guide. Every artist page links to its MusicBrainz entry, so you can check or improve it.'),
        barList({ filename: 'plays-by-genre', head: ['Genre group', 'Plays'], items: summary.genre_buckets.slice(0, 18).map((g) => ({ label: g[0], value: g[1] })) })),
    ]));
    allTime = main.querySelector('.alltime');
  }

  const playedTabs = [['#/most-played/today', 'Last day', 'today'], ['#/most-played/week', 'This week', 'week'], ['#/most-played/month', 'This month', 'month'], ['#/most-played/year', 'This year', 'year'], ['#/most-played/all', 'All time', 'all'], ['#/crate-dive', 'Least played', 'least']];

  /** Tracks and artists played exactly once, counted for every year. Also the "Least played" tab. */
  async function crateDive(y) {
    const list = await getJSON('crate.json');
    if (y) {
      const e = list.find((x) => String(x.year) === String(y));
      if (!e) return notFound();
      const d = await getJSON('crate/' + y + '.json');
      const sample = (n, shown) => (shown < n ? ' (' + fmt(shown) + ' shown, spread evenly through the year)' : '');
      return setPage('Crate dive ' + y, h('p', { class: 'tag' }, h('a', { href: '#/crate-dive' }, 'Crate dive')), h('h1', {}, String(y)),
        h('p', { class: 'lede' }, fmt(e.once_tracks) + ' tracks and ' + fmt(e.once_artists) + ' artists played only once, ever · ' + fmt(e.tracks) + ' different tracks were played in ' + y),
        h('div', { class: 'two' },
          section('Tracks' + sample(e.once_tracks, d.track_rows.length), dataTable({
            cols: [{ label: 'Artist', value: (r) => r[0], cell: (r) => artistLink(r[0], r[4], r[3]) }, { label: 'Track', value: (r) => r[1] }, { label: 'Played on', value: (r) => r[5] }, { label: 'Listen', value: () => '', cell: (r) => listenLinks(r[0], r[1]), nodl: true }],
            rows: d.track_rows, filename: 'played-once-tracks-' + y, rank: false, sortCol: 2, sortDir: 'asc',
          })),
          section('Artists' + sample(e.once_artists, d.artist_rows.length), dataTable({
            cols: [{ label: 'Artist', value: (r) => r[0], cell: (r) => artistLink(r[0], r[2], r[1]) }, { label: 'Track', value: (r) => r[3] }, { label: 'Played on', value: (r) => r[4] }],
            rows: d.artist_rows, filename: 'played-once-artists-' + y, rank: false, sortCol: 2, sortDir: 'asc',
          }))));
    }
    const total = list.reduce((n, r) => n + r.once_tracks, 0);
    const pct = (a, b) => (b ? Math.round((a * 100) / b) + '%' : '0%');
    setPage('Crate dive: tracks and artists played only once', h('h1', {}, 'Crate dive'), tabs(playedTabs, 'least'),
      h('p', { class: 'lede' }, fmt(total) + ' tracks in the archive have been played exactly once. Here they are, year by year: the deepest cuts to reach 6 Music.'),
      h('p', { class: 'note' }, 'A one-off is a track, or an artist, with a single play in the whole archive. Recent years score higher because new tracks have had less time to be played again, and tracklists before about 2010 are incomplete.'),
      barChart({ title: 'Tracks played only once, by year', filename: 'played-once-tracks-by-year', unit: 'tracks', data: list.map((r) => ({ label: String(r.year), value: r.once_tracks })), labelEvery: 1, onPick: (i) => go(toPath('#/crate-dive/' + list[i].year)) }),
      barChart({ title: 'Artists played only once, by year', filename: 'played-once-artists-by-year', unit: 'artists', data: list.map((r) => ({ label: String(r.year), value: r.once_artists })), labelEvery: 1, onPick: (i) => go(toPath('#/crate-dive/' + list[i].year)) }),
      section('Choose a year', h('div', { class: 'grid' }, [...list].reverse().map((r) => card({ tag: String(r.year), title: fmt(r.once_tracks) + ' one-off tracks', href: '#/crate-dive/' + r.year, lines: [fmt(r.once_artists) + ' one-off artists', pct(r.once_tracks, r.tracks) + ' of the tracks played that year'], more: ['#/crate-dive/' + r.year, 'Dive into ' + r.year] })))),
      section('Table', dataTable({
        cols: [{ label: 'Year', value: (r) => r.year, cell: (r) => h('a', { href: '#/crate-dive/' + r.year }, String(r.year)) }, { label: 'One-off tracks', value: (r) => r.once_tracks, num: true }, { label: 'Of all tracks', value: (r) => (r.tracks ? r.once_tracks / r.tracks : 0), cell: (r) => pct(r.once_tracks, r.tracks), num: true }, { label: 'One-off artists', value: (r) => r.once_artists, num: true }, { label: 'Of all artists', value: (r) => (r.artists ? r.once_artists / r.artists : 0), cell: (r) => pct(r.once_artists, r.artists), num: true }],
        rows: list, filename: 'crate-dive-by-year', rank: false, sortCol: 0, sortDir: 'desc',
      })));
  }

  async function mostPlayed(which = 'week') {
    const items = playedTabs;
    if (which === 'least') return crateDive();
    if (which === 'all') {
      const [t, a, meta] = await Promise.all([getJSON('tracks_top.json'), getJSON('artists_top.json'), getJSON('meta.json')]);
      return setPage('Most played of all time', h('h1', {}, 'Most played, all time'), tabs(items, which),
        h('p', { class: 'lede' }, fmtDate(meta.first_day) + ' to ' + fmtDate(meta.as_of) + ' · ' + fmt(meta.plays) + ' plays · ' + fmt(meta.tracks) + ' different tracks · ' + fmt(meta.one_play_tracks) + ' of them played only once'),
        h('p', { class: 'note' }, 'Some of the most-played entries are programme theme tunes and jingles, which the BBC logs as tracks.'),
        h('div', { class: 'two' },
          section('Tracks', dataTable({ cols: trackCols(), rows: t, filename: 'most-played-tracks-all-time', sortCol: 2 })),
          section('Artists', dataTable({ cols: artistRowCols, rows: a, filename: 'most-played-artists-all-time', sortCol: 1 }))));
    }
    const w = (await getJSON('windows.json'))[which];
    const names = { today: 'the last day in the data', week: 'the last 7 days', month: 'the last 30 days', year: 'this year so far' };
    setPage('Most played', h('h1', {}, 'Most played, ' + names[which]), tabs(items, which),
      h('p', { class: 'lede' }, fmtDate(w.from) + (w.from === w.to ? '' : ' to ' + fmtDate(w.to)) + ' · ' + fmt(w.plays) + ' plays · ' + fmt(w.distinct_tracks) + ' different tracks · ' + fmt(w.played_once) + ' of them played only once'),
      h('div', { class: 'two' },
        section('Tracks', dataTable({ cols: trackCols(), rows: w.tracks, filename: 'most-played-tracks-' + which, sortCol: 2 })),
        section('Artists', dataTable({ cols: artistRowCols, rows: w.artists, filename: 'most-played-artists-' + which, sortCol: 1 }))));
  }

  /** An episode's tracklist, each track with its listen links. Tracks are [artist, title, artist id, letter]. */
  const epTracks = (e) => h('ol', {}, e.tracks.map((t) => h('li', {}, artistLink(t[0], t[3], t[2]), ' - ', t[1], ' ', listenLinks(t[0], t[1]))));

  async function latest() {
    const days = await getJSON('latest.json');
    setPage('Latest', h('h1', {}, 'Played in the last 7 days'), h('p', { class: 'lede' }, 'Every episode with a tracklist, newest first. Open an episode to see what was played.'),
      days.map((d) => section(fmtDate(d.date), d.episodes.length ? d.episodes.map((e) => h('details', {},
        h('summary', {}, e.time + '  ', h('b', {}, e.show), '  ', h('span', { class: 'meta' }, e.title + ' · ' + e.tracks.length + ' tracks')),
        epTracks(e),
        h('p', {}, h('a', { href: '#/show/' + e.slug }, 'All songs played on ' + e.show), e.pid ? ' · ' : null, bbcEpisode(e.pid)),
        playlistTools(e.slug + '-' + e.date, e.tracks.map((t) => [t[0], t[1]])))) : h('p', { class: 'meta' }, 'No tracklists found for this day.'))));
  }

  async function shows() {
    const list = await getJSON('shows.json');
    setPage('Shows', h('h1', {}, 'Shows'), h('p', { class: 'lede' }, fmt(list.length) + ' shows. Open one to see every song it has played.'),
      dataTable({
        cols: [{ label: 'Show', value: (r) => r[0], cell: (r) => h('a', { href: '#/show/' + r[1] }, r[0]) }, { label: 'Plays', value: (r) => r[2], num: true }, { label: 'Episodes', value: (r) => r[3], num: true },
          { label: 'Different tracks', value: (r) => r[4], num: true }, { label: 'First', value: (r) => r[5] }, { label: 'Last', value: (r) => r[6] }, { label: 'Most played artist', value: (r) => r[7] }],
        rows: list, filename: 'shows', rank: false, sortCol: 1, pageSize: 200,
      }));
  }

  async function show(slug) {
    const [s, daily] = await Promise.all([getJSON('shows/' + encodeURIComponent(slug) + '.json'), dailyShow(slug)]);
    let allShow = null;
    const ex = explorer({ daily, slug, grain: 'month', onChange: (sel) => { if (allShow) allShow.hidden = !!sel; } });
    setPage(s.name, h('p', { class: 'tag' }, h('a', { href: '#/shows' }, 'Shows')), h('h1', {}, s.name),
      h('p', { class: 'lede' }, fmt(s.plays) + ' plays · ' + fmt(s.episodes) + ' episodes with tracklists · ' + fmtDate(s.first) + ' to ' + fmtDate(s.last)),
      section('Zoom in on this show', h('p', { class: 'meta' }, 'Choose day, week, month, quarter or year, then click a bar to see what this show played in that period.'), ex.el), ex.panel,
      h('div', { class: 'alltime', hidden: !!new URLSearchParams(location.search).get('period') }, ...[
      section('Most played artists', dataTable({ cols: artistRowCols, rows: s.top_artists, filename: slug + '-top-artists', sortCol: 1, rank: true })),
      section('Every song played on this show', playlistTools(slug, s.tracks.map((r) => [r[0], r[1]])), dataTable({ cols: trackCols(), rows: s.tracks, filename: slug + '-all-songs', sortCol: 2 })),
      section('Latest episodes', s.recent.map((e) => h('details', {}, h('summary', {}, fmtDate(e.date) + ' ' + e.time + '  ', h('span', { class: 'meta' }, e.title + ' · ' + e.tracks.length + ' tracks')),
        epTracks(e), h('p', {}, bbcEpisode(e.pid)), playlistTools(slug + '-' + e.date, e.tracks.map((t) => [t[0], t[1]])))))]));
    allShow = main.querySelector('.alltime');
  }

  const LETTERS = '0abcdefghijklmnopqrstuvwxyz'.split('');
  let pendingQuery = '';
  const shardOf = (name) => {
    const n = strip(name).replace(/^the\s+/, '').trim();
    const c = n.match(/[a-z0-9]/);
    return !c ? 'other' : /\d/.test(c[0]) ? '0' : c[0];
  };

  async function artists(letter) {
    const az = h('nav', { class: 'az', 'aria-label': 'Artists A to Z' }, [h('a', { href: '#/artists', 'aria-current': !letter ? 'page' : false }, 'Top'), ...LETTERS.map((l) => h('a', { href: '#/artists/' + l, 'aria-current': letter === l ? 'page' : false }, l === '0' ? '0-9' : l)), h('a', { href: '#/artists/other', 'aria-current': letter === 'other' ? 'page' : false }, 'other')]);
    const head = [h('h1', {}, 'Artists'), h('p', { class: 'lede' }, 'Each artist has a MusicBrainz ID where the BBC supplied one. Choose a letter to browse everyone, or see the most played below.'), az];
    if (!letter) {
      const a = await getJSON('artists_top.json');
      return setPage('Artists', ...head, dataTable({
        cols: [{ label: 'Artist', value: (r) => r[0], cell: (r) => artistLink(r[0], r[2], r[1]) }, { label: 'Plays', value: (r) => r[3], num: true }, { label: 'Tracks', value: (r) => r[4], num: true }, ...(NOGENRES ? [] : [{ label: 'Genre (MusicBrainz)', value: (r) => r[5] }, { label: 'Country', value: (r) => r[6] }])],
        rows: a, filename: 'top-artists', sortCol: 1,
      }));
    }
    const rows = await getJSON('artists/' + letter + '.json');
    const q = pendingQuery; pendingQuery = '';
    setPage('Artists ' + letter.toUpperCase(), ...head, dataTable({
      cols: [{ label: 'Artist', value: (r) => r.name, cell: (r) => artistLink(r.name, letter, r.id) }, { label: 'MusicBrainz', value: (r) => r.mbid, cell: (r) => (r.mbid ? mbLink(r.mbid) : '') },
        { label: 'Plays', value: (r) => r.plays, num: true }, { label: 'Tracks', value: (r) => r.tracks, num: true }, { label: 'First played', value: (r) => r.first }, { label: 'Last played', value: (r) => r.last },
        ...(NOGENRES ? [] : [{ label: 'Genre (MusicBrainz)', value: (r) => r.genre }, { label: 'Country', value: (r) => r.country }])],
      rows, filename: 'artists-' + letter, sortCol: 2, filter: q, rank: false,
    }));
  }

  async function artist(letter, id) {
    const rows = await getJSON('artists/' + letter + '.json');
    const a = rows.find((r) => r.id === id);
    if (!a) return notFound();
    const genres = a.genres ? a.genres.split(';').map((g) => g.trim()).filter(Boolean) : [];
    setPage(a.name, h('p', { class: 'tag' }, h('a', { href: '#/artists/' + letter }, 'Artists'), ' / ', letter.toUpperCase()), h('h1', {}, a.name),
      h('dl', { class: 'facts' },
        h('dt', {}, 'Plays'), h('dd', {}, fmt(a.plays) + ' plays of ' + fmt(a.tracks) + ' different tracks'),
        h('dt', {}, 'First / last played'), h('dd', {}, fmtDate(a.first) + ' / ' + fmtDate(a.last)),
        [h('dt', {}, 'MusicBrainz ID'), h('dd', {}, a.mbid ? [h('code', {}, a.mbid), '  ', h('a', { href: 'https://musicbrainz.org/artist/' + a.mbid, rel: 'noopener' }, 'View on MusicBrainz')] : (NOGENRES ? 'Not matched on MusicBrainz yet' : 'Not supplied by the BBC for this artist'))],
        a.type ? [h('dt', {}, 'Type'), h('dd', {}, a.type)] : null,
        a.country ? [h('dt', {}, 'Country'), h('dd', {}, a.country)] : null,
        a.begin ? [h('dt', {}, 'Began'), h('dd', {}, a.begin)] : null,
        genres.length ? [h('dt', {}, 'Genres (MusicBrainz tags)'), h('dd', {}, genres.join(', '))] : null),
      h('p', {}, a.mbid ? [h('a', { href: 'https://musicbrainz.org/artist/' + a.mbid, rel: 'noopener' }, 'Official website and profiles on MusicBrainz'), ' · '] : null,
        h('a', { href: 'https://en.wikipedia.org/w/index.php?search=' + encodeURIComponent(a.name + ' musician'), rel: 'noopener noreferrer nofollow' }, 'Wikipedia search'), '  ', listenLinks(a.name, '')),
      genres.length || NOGENRES ? null : h('p', { class: 'note' }, 'MusicBrainz has no genre for this artist yet. If you know them, adding one there will improve every site that uses it.'),
      section('Most played tracks', dataTable({ cols: [{ label: 'Track', value: (r) => r[0] }, { label: 'Plays', value: (r) => r[1], num: true }, { label: 'Listen', value: () => '', cell: (r) => listenLinks(a.name, r[0]), nodl: true }], rows: a.top, filename: 'artist-' + id.slice(0, 40), sortCol: 1 })));
  }

  async function years(y) {
    const [list, summary] = await Promise.all([getJSON('years.json'), getJSON('summary.json')]);
    if (y) {
      const e = list.find((x) => String(x.year) === String(y));
      if (!e) return notFound();
      const daily = await dailyAll();
      let allYear = null;
      const ex = explorer({ daily, grain: 'month', end: Math.min(daily.start + daily.d.length - 1, dnum(+y, 11, 31)), onChange: (sel) => { if (allYear) allYear.hidden = !!sel; } });
      setPage(String(y), h('p', { class: 'tag' }, h('a', { href: '#/years' }, 'Years')), h('h1', {}, String(y)),
        h('p', { class: 'lede' }, fmt(e.plays) + ' plays · ' + fmt(e.episodes) + ' episodes · ' + fmt(e.artists) + ' different artists'),
        section('Zoom in on ' + y, h('p', { class: 'meta' }, 'Choose day, week, month, quarter or year, then click a bar to see just that period.'), ex.el), ex.panel,
        h('div', { class: 'two alltime', hidden: !!new URLSearchParams(location.search).get('period') },
          section('Most played tracks in ' + y, dataTable({ cols: trackCols(), rows: e.tracks, filename: 'tracks-' + y, sortCol: 2 })),
          section('Most played artists in ' + y, dataTable({ cols: artistRowCols, rows: e.top_artists, filename: 'artists-' + y, sortCol: 1 }))));
      allYear = main.querySelector('.alltime');
      return;
    }
    setPage('Years', h('h1', {}, 'Years'), h('p', { class: 'lede' }, 'Tracklists begin in mid-2008 and are complete from about 2010.'),
      barChart({ title: 'Tracks played per year', filename: 'plays-per-year', data: summary.by_year.map((r) => ({ label: String(r[0]), value: r[1] })), labelEvery: 1, onPick: (i) => go(toPath('#/year/' + summary.by_year[i][0])) }),
      section('Choose a year', h('div', { class: 'grid' }, [...list].reverse().map((e) => card({ tag: String(e.year), title: fmt(e.plays) + ' plays', href: '#/year/' + e.year, lines: [fmt(e.episodes) + ' episodes · ' + fmt(e.artists) + ' artists'], list: e.tracks.slice(0, 3).map(trackItem), more: ['#/year/' + e.year, 'Top 100 of ' + e.year] })))),
      section('Table', dataTable({ cols: [{ label: 'Year', value: (r) => r[0] }, { label: 'Plays', value: (r) => r[1], num: true }, { label: 'Episodes', value: (r) => r[2], num: true }, { label: 'Different artists', value: (r) => r[3], num: true }], rows: summary.by_year, filename: 'years', rank: false, sortCol: 0, sortDir: 'desc' })));
  }

  async function search(query) {
    const q = strip(query).trim();
    if (!q) return home();
    const words = q.split(/\s+/);
    const [list, top] = await Promise.all([getJSON('shows.json'), getJSON('artists_top.json')]);
    const matchShows = list.filter((s) => words.every((w) => strip(s[0]).includes(w)));
    let shard = [];
    try { shard = await getJSON('artists/' + shardOf(query) + '.json'); } catch (e) { /* no such shard */ }
    const found = shard.filter((a) => words.every((w) => strip(a.name).includes(w))).slice(0, 200);
    setPage('Search', h('h1', {}, 'Search: ' + query),
      section('Shows (' + matchShows.length + ')', matchShows.length ? h('ul', {}, matchShows.slice(0, 50).map((s) => h('li', {}, h('a', { href: '#/show/' + s[1] }, s[0]), h('span', { class: 'meta' }, ' · ' + fmt(s[2]) + ' plays')))) : h('p', { class: 'meta' }, 'No shows match.')),
      section('Artists (' + found.length + (found.length === 200 ? '+' : '') + ')', found.length ? dataTable({ cols: [{ label: 'Artist', value: (r) => r.name, cell: (r) => artistLink(r.name, shardOf(r.name), r.id) }, { label: 'Plays', value: (r) => r.plays, num: true }, { label: 'Tracks', value: (r) => r.tracks, num: true }], rows: found, filename: 'search-artists', rank: false, sortCol: 1 })
        : h('p', { class: 'meta' }, 'No artists match. Artists are found by the first letter of their name, so check the spelling of the start of it.')),
      h('p', { class: 'meta' }, 'Searching tracks: use the filter box on any table, for example on the all-time most played page.'));
  }

  /** Split CSV text into rows of fields (handles quotes, doubled quotes and line breaks inside quotes). */
  function parseCSV(text) {
    const rows = [];
    let row = [], f = '', q = false;
    for (let i = 0; i < text.length; i++) {
      const c = text[i];
      if (q) { if (c === '"') { if (text[i + 1] === '"') { f += '"'; i++; } else q = false; } else f += c; }
      else if (c === '"') q = true;
      else if (c === ',') { row.push(f); f = ''; }
      else if (c === '\n') { row.push(f); rows.push(row); row = []; f = ''; }
      else if (c !== '\r') f += c;
    }
    if (f || row.length) { row.push(f); rows.push(row); }
    return rows;
  }
  /** Tab-separated plain text, one line per row: opens in any text editor and pastes into a spreadsheet. */
  const csvToTXT = (text) => parseCSV(text).map((r) => r.map((c) => c.replace(/[\t\r\n]+/g, ' ')).join('\t')).join('\n') + '\n';

  /** A standalone web page holding the whole file as a table, built in pieces so a big file does not need one giant string. */
  function csvToHTMLParts(text, title) {
    const esc = (v) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    const rows = parseCSV(text), parts = [];
    parts.push('<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>' + esc(title) + '</title><style>body{font:14px/1.4 system-ui,sans-serif;margin:16px}table{border-collapse:collapse}th,td{border:1px solid #bbb;padding:3px 8px;text-align:left;vertical-align:top}th{background:#eee;position:sticky;top:0}</style></head><body><h1>' + esc(title) + '</h1><table>');
    for (let i = 0; i < rows.length; i += 5000) {
      parts.push(rows.slice(i, i + 5000).map((r, j) => '<tr>' + r.map((c) => (i + j === 0 ? '<th>' : '<td>') + esc(c) + (i + j === 0 ? '</th>' : '</td>')).join('') + '</tr>').join('\n') + '\n');
    }
    parts.push('</table></body></html>');
    return parts;
  }

  /** A button that fetches a data file and saves it to the computer (as .csv, or converted to .txt) instead of opening it in a tab. */
  function fileButton(url, name, kind = 'csv') {
    const label = kind === 'txt' ? 'TXT' : kind === 'html' ? 'HTML' : 'CSV';
    const btn = h('a', { class: 'btn', href: url, download: name, rel: 'noopener', 'aria-label': 'Download ' + name + ' as ' + label }, label);
    btn.addEventListener('click', async (e) => {
      e.preventDefault();
      if (btn.dataset.busy) return;
      btn.dataset.busy = '1';
      try {
        const res = await fetch(url);
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const total = +res.headers.get('content-length') || 0, parts = [];
        let got = 0;
        const rd = res.body && res.body.getReader();
        if (rd) for (;;) {
          const { done, value } = await rd.read();
          if (done) break;
          parts.push(value); got += value.length;
          btn.textContent = total ? Math.round((got / total) * 100) + '%' : Math.round(got / 1048576) + ' MB';
        } else parts.push(await res.blob());
        if (kind === 'html') {
          btn.textContent = 'Converting';
          await new Promise((r) => setTimeout(r, 30));
          const text = await new Blob(parts).text();
          saveBlob(name.replace(/\.csv$/i, '') + '.html', new Blob(csvToHTMLParts(text, name.replace(/\.csv$/i, '')), { type: 'text/html;charset=utf-8' }));
        } else if (kind === 'txt') {
          btn.textContent = 'Converting';
          await new Promise((r) => setTimeout(r, 30));
          const text = await new Blob(parts).text();
          saveBlob(name.replace(/\.csv$/i, '') + '.txt', new Blob([csvToTXT(text)], { type: 'text/plain;charset=utf-8' }));
        } else saveBlob(name, new Blob(parts, { type: 'text/csv' }));
        btn.textContent = 'Saved ✓';
      } catch (err) {
        // If the browser will not let the page fetch the file, open it directly instead.
        window.location.href = url;
      }
      delete btn.dataset.busy;
      setTimeout(() => { btn.textContent = label; }, 4000);
    });
    return btn;
  }
  /** Most played tracks as a playlist file in Tune My Music's own format (Track name, Artist name, Album, Playlist name). */
  function playlistFileButton(url, count) {
    const label = count ? 'Top ' + fmt(count) : 'All, in lists of 10,000';
    const btn = h('button', { type: 'button' }, label);
    btn.addEventListener('click', async () => {
      if (btn.dataset.busy) return;
      btn.dataset.busy = '1';
      try {
        btn.textContent = 'Loading';
        const rows = parseCSV(await (await fetch(url)).text());
        const head = rows.shift(), col = (n) => head.indexOf(n);
        const [ia, it, ib, ip] = [col('Artist'), col('Title'), col('Album'), col('Plays')];
        rows.sort((x, y) => (+y[ip] || 0) - (+x[ip] || 0));
        const list = (count ? rows.slice(0, count) : rows).filter((r) => r[ia] && r[it]);
        const per = count || 10000, name = (i) => count ? 'BBC 6 Music top ' + fmt(count) : 'BBC 6 Music most played ' + String(i + 1).padStart(2, '0');
        const out = list.map((r, i) => [r[it], r[ia], r[ib] || '', name(Math.floor(i / per))]);
        saveBlob(count ? 'bbc6-top-' + count + '-playlist.csv' : 'bbc6-all-playlists.csv', new Blob([toCSV(['Track name', 'Artist name', 'Album', 'Playlist name'], out)], { type: 'text/csv;charset=utf-8' }));
        btn.textContent = 'Saved ✓';
      } catch (err) { btn.textContent = 'Could not load'; }
      delete btn.dataset.busy;
      setTimeout(() => { btn.textContent = label; }, 4000);
    });
    return btn;
  }
  const fileButtons = (url, name) => h('span', { class: 'btnrow' }, fileButton(url, name, 'csv'), ' ', fileButton(url, name, 'txt'), ' ', fileButton(url, name, 'html'));

  async function downloads() {
    const meta = await getJSON('meta.json');
    const ONLYFILE = '{{DOWNLOADS}}';
    if (ONLYFILE) {
      return setPage('Downloads', h('h1', {}, 'Downloads'),
        h('p', { class: 'lede' }, 'Every play is in one plain CSV file, free to use for personal, non-commercial purposes (as of ' + fmtDate(meta.as_of) + ').'),
        section('The dataset', h('p', {}, fileButtons(BASE + ONLYFILE, ONLYFILE))),
        section('Download a chart', h('p', {}, 'Every table and chart on this site has CSV and TXT buttons that download exactly what is on screen. For example:'),
          h('ul', {}, h('li', {}, h('a', { href: '#/most-played/week' }, 'Most played this week')), h('li', {}, h('a', { href: '#/most-played/all' }, 'Most played of all time')), h('li', {}, h('a', { href: '#/years' }, 'Plays per year, and a top 100 for each year')), h('li', {}, h('a', { href: '#/shows' }, 'Every show; open one for a playlist of everything it has played')))));
    }
    const raw = (f) => REPO.replace('https://github.com/', 'https://raw.githubusercontent.com/') + '/main/data/' + f;
    const files = [
      ['tracks_unique.csv', 'Every distinct track with its plays, first and last played, album, label and MusicBrainz artist ID', '46 MB'],
      ['tracks_by_programme.csv', 'What every show has played, with play counts', '43 MB'],
      ['artists_unique.csv', 'Every artist with total plays and distinct tracks', '5 MB'],
      ['artists_enriched.csv', 'Artists with MusicBrainz genre, country, area and era', '8 MB'],
      ['episodes.csv', 'Every episode seen: date, time, show, title, BBC episode ID', '5 MB'],
    ];
    setPage('Downloads', h('h1', {}, 'Downloads'),
      h('p', { class: 'lede' }, 'The whole dataset is plain CSV and free to use for personal, non-commercial purposes. The files below are a snapshot; the newest plays are in the data release and the website tables.'),
      section('The dataset',
        h('div', { class: 'tablewrap' }, h('table', {}, h('thead', {}, h('tr', {}, ['File', 'What it is', 'Size', ''].map((t) => h('th', { scope: 'col' }, t)))),
          h('tbody', {}, files.map((f) => h('tr', {}, h('td', {}, h('code', {}, f[0])), h('td', {}, f[1]), h('td', {}, f[2]), h('td', {}, fileButtons(raw(f[0]), f[0])))),
            h('tr', {}, h('td', {}, h('code', {}, 'plays_YYYY.csv.gz')), h('td', {}, 'The full play-by-play record, one file per year (the source of all the others)'), h('td', {}, '4-6 MB each'), h('td', {}, h('a', { class: 'btn', href: REPO + '/tree/main/data/plays', rel: 'noopener' }, 'Browse'))),
            h('tr', {}, h('td', {}, h('code', {}, 'data-latest')), h('td', {}, 'The same CSV files, gzipped and rebuilt daily with the newest plays included (as of ' + fmtDate(meta.as_of) + ')'), h('td', {}, 'gzipped'), h('td', {}, h('a', { class: 'btn', href: REPO + '/releases/tag/data-latest', rel: 'noopener' }, 'Latest data'))))))),
      section('Playlist for Tune My Music', h('p', {}, 'Tune My Music needs a file with the columns Track name, Artist name and Playlist name, so the dataset files above will not import. These make that file from the most played tracks, ready to upload at ', h('a', { href: 'https://www.tunemymusic.com/', rel: 'noopener noreferrer' }, 'tunemymusic.com'), ' (choose Upload file as the source). Its free plan is small, so start with the top 500.'),
        h('p', { class: 'btnrow' }, ...[500, 1000, 5000, 10000, 0].flatMap((n) => [playlistFileButton(raw('tracks_unique.csv'), n), ' '])),
        h('p', { class: 'meta' }, 'The last one splits every track into playlists of 10,000, which is Spotify\'s limit.')),
      section('Download a chart', h('p', {}, 'Every table and chart on this site has CSV and TXT buttons that download exactly what is on screen, sorted and filtered the way you have it. For example:'),
        h('ul', {}, h('li', {}, h('a', { href: '#/most-played/week' }, 'Most played this week')), h('li', {}, h('a', { href: '#/most-played/all' }, 'Most played of all time')), h('li', {}, h('a', { href: '#/years' }, 'Plays per year, and a top 100 for each year')), h('li', {}, h('a', { href: '#/shows' }, 'Every show; open one for a playlist of everything it has played')))),
      section('Playlists', h('p', {}, 'Each show page has a table of every song it has played with a CSV download, ready to import into a playlist-converter service. Use the Playlist CSV button on a show or episode page for that: it already has the column names Tune My Music expects (Track name, Artist name, Playlist name). The big dataset files above are for analysis; they are far too large for a converter, and the programme file has some rows with no show name. The command-line program in the source code can also write per-show and per-genre playlist files.')),
      section('Licence and credit', h('p', { class: 'prose' }, 'Tracklist facts originate with the BBC. Artist IDs, countries and dates are MusicBrainz core data (CC0). MusicBrainz genre and tag data is CC BY-NC-SA 3.0: please credit MusicBrainz, use it non-commercially and share alike. The code is MIT licensed.')));
  }

  function notFound() {
    setPage('Not found', h('h1', {}, 'Not found'), h('p', { class: 'lede' }, 'That page does not exist.'), h('p', {}, h('a', { href: '#/' }, 'Back to the start')));
  }

  /* ------------------------------------------------------------ router */

  let firstDone = false;
  async function route() {
    const rel = location.pathname.startsWith(BASE) ? location.pathname.slice(BASE.length) : '';
    const parts = rel.split('/').filter(Boolean).map(decodeURIComponent);
    const [page, a, b] = parts;
    if (main.hasAttribute('data-keep') && !firstDone) { firstDone = true; return; }
    firstDone = true;
    document.querySelectorAll('.top nav a').forEach((l) => {
      const on = l.getAttribute('href') === BASE + (page === 'show' ? 'shows' : page === 'artist' ? 'artists' : page === 'year' ? 'years' : page) + '/';
      on ? l.setAttribute('aria-current', 'page') : l.removeAttribute('aria-current');
    });
    main.replaceChildren(h('p', { class: 'loading' }, 'Loading'));
    try {
      switch (page) {
        case '': case undefined: await home(); break;
        case 'most-played': await mostPlayed(a || 'week'); break;
        case 'crate-dive': await crateDive(a); break;
        case 'latest': await latest(); break;
        case 'shows': await shows(); break;
        case 'show': await show(a); break;
        case 'artists': await artists(a); break;
        case 'artist': await artist(a, b); break;
        case 'years': await years(); break;
        case 'year': await years(a); break;
        case 'search': await search(a || ''); break;
        case 'downloads': await downloads(); break;
        case 'about': location.replace(BASE + 'about/'); return;
        default: notFound();
      }
    } catch (e) {
      setPage('Problem', h('h1', {}, 'Something went wrong'), h('p', { class: 'lede' }, 'The data for this page could not be loaded (' + e.message + '). Try again in a minute.'));
    }
    main.focus({ preventScroll: true });
  }

  document.getElementById('find').addEventListener('submit', (e) => {
    e.preventDefault();
    const v = document.getElementById('q').value.trim();
    if (v) go(BASE + 'search/' + encodeURIComponent(v) + '/');
  });
  /** Open a page of this site without reloading. */
  function go(path) {
    history.pushState(null, '', path);
    main.removeAttribute('data-keep');
    route();
  }
  /* Links inside the site open without a reload; the About page is plain HTML, so it loads normally. */
  document.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('a[href]');
    if (!a || e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || a.target || a.hasAttribute('download')) return;
    const u = new URL(a.href, location.href);
    if (u.origin !== location.origin || !u.pathname.startsWith(BASE) || u.hash || u.pathname === BASE + 'about/') return;
    e.preventDefault();
    go(u.pathname + u.search);
  });
  document.querySelector('.skip').addEventListener('click', (e) => { e.preventDefault(); main.focus(); });
  window.addEventListener('popstate', route);
  route();
})();
