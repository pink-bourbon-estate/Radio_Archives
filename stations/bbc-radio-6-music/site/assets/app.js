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
    const get = () => ({ head: ['Artist', 'Title'], rows: pairs });
    return h('p', { class: 'playlist' },
      h('button', { type: 'button', onclick: () => saveFile(name + '-playlist.csv', toCSV(get().head, get().rows), 'text/csv'), 'aria-label': 'Download this tracklist as a playlist CSV' }, 'Playlist CSV'),
      ' then import it at ', h('a', { href: 'https://www.tunemymusic.com/', rel: 'noopener noreferrer' }, 'Tune My Music'), ' or ', h('a', { href: 'https://soundiiz.com/', rel: 'noopener noreferrer' }, 'Soundiiz'),
      ' to make a playlist in the service you use.');
  }
  const strip = (s) => String(s || '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');

  /** Save text as a file in the visitor's browser. */
  function saveFile(name, text, type) {
    const url = URL.createObjectURL(new Blob([text], { type: type + ';charset=utf-8' }));
    const a = h('a', { href: url, download: name });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
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
      kind === 'csv' ? saveFile(filename + '.csv', toCSV(head, rows), 'text/csv') : saveFile(filename + '.txt', toTXT(head, rows), 'text/plain');
    };
    return [h('button', { type: 'button', onclick: go('csv'), 'aria-label': 'Download this table as CSV' }, 'CSV'), h('button', { type: 'button', onclick: go('txt'), 'aria-label': 'Download this table as text' }, 'TXT')];
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
  function barChart({ data, title, filename, unit = 'plays', labelEvery = 1 }) {
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
      const bar = el('rect', { class: 'bar', x, y: H - B - bh, width: bw * 0.76, height: Math.max(bh, 0) });
      bar.append(el('title', {}, `${d.label}: ${fmt(d.value)} ${unit}`));
      svg.append(bar);
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
    const [meta, win, summary, latest] = await Promise.all([getJSON('meta.json'), getJSON('windows.json'), getJSON('summary.json'), getJSON('latest.json')]);
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
      section('Played recently', h('p', { class: 'meta' }, 'The windows below end on the last day in the data (' + fmtDate(meta.as_of) + '), and are refreshed every day.'),
        h('div', { class: 'grid' }, wcard('today', 'Played on the last day'), wcard('week', 'Played this week'), wcard('month', 'Played this month'), wcard('year', 'Played this year'))),
      section('On the last day', h('p', { class: 'meta' }, lastDay ? fmtDate(lastDay.date) + ' · ' + lastDay.episodes.length + ' episodes with tracklists' : ''),
        h('div', { class: 'grid' }, (lastDay ? lastDay.episodes : []).slice(0, 8).map((e) => card({
          tag: e.time, title: e.show, href: '#/show/' + e.slug, lines: [e.title, fmt(e.tracks.length) + ' tracks'],
        }))), h('p', {}, h('a', { class: 'more', href: '#/latest' }, 'All episodes and tracklists from the last 7 days'))),
      section('Plays per year', barChart({ title: 'Tracks played per year', filename: 'plays-per-year', data: summary.by_year.map((y) => ({ label: String(y[0]), value: y[1] })), labelEvery: 2 })),
      h('div', { class: 'two' },
        section('Busiest shows', barList({ filename: 'top-shows', head: ['Show', 'Plays'], items: summary.top_shows.slice(0, 15).map((s) => ({ label: s[0], value: s[2], href: '#/show/' + s[1] })) })),
        section('Day of the week', barChart({ title: 'Tracks played by weekday', filename: 'plays-by-weekday', data: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((d, i) => ({ label: d, value: summary.weekday[i] })) }))),
      section('Time of day', barChart({ title: 'Tracks played by the hour the programme starts', filename: 'plays-by-hour', data: summary.hour.map((v, i) => ({ label: String(i).padStart(2, '0'), value: v })) })),
      NOGENRES ? null : section('Genres (from MusicBrainz, use with care)',
        h('p', { class: 'note' }, 'Genre labels come from MusicBrainz user tags, and only about ' + summary.genre_coverage_pct + '% of plays belong to an artist that has one. Treat these as a rough guide. Every artist page links to its MusicBrainz entry, so you can check or improve it.'),
        barList({ filename: 'plays-by-genre', head: ['Genre group', 'Plays'], items: summary.genre_buckets.slice(0, 18).map((g) => ({ label: g[0], value: g[1] })) })),
    );
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
      barChart({ title: 'Tracks played only once, by year', filename: 'played-once-tracks-by-year', unit: 'tracks', data: list.map((r) => ({ label: String(r.year), value: r.once_tracks })), labelEvery: 1 }),
      barChart({ title: 'Artists played only once, by year', filename: 'played-once-artists-by-year', unit: 'artists', data: list.map((r) => ({ label: String(r.year), value: r.once_artists })), labelEvery: 1 }),
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
    const s = await getJSON('shows/' + encodeURIComponent(slug) + '.json');
    setPage(s.name, h('p', { class: 'tag' }, h('a', { href: '#/shows' }, 'Shows')), h('h1', {}, s.name),
      h('p', { class: 'lede' }, fmt(s.plays) + ' plays · ' + fmt(s.episodes) + ' episodes with tracklists · ' + fmtDate(s.first) + ' to ' + fmtDate(s.last)),
      section('Most played artists', dataTable({ cols: artistRowCols, rows: s.top_artists, filename: slug + '-top-artists', sortCol: 1, rank: true })),
      section('Every song played on this show', playlistTools(slug, s.tracks.map((r) => [r[0], r[1]])), dataTable({ cols: trackCols(), rows: s.tracks, filename: slug + '-all-songs', sortCol: 2 })),
      section('Latest episodes', s.recent.map((e) => h('details', {}, h('summary', {}, fmtDate(e.date) + ' ' + e.time + '  ', h('span', { class: 'meta' }, e.title + ' · ' + e.tracks.length + ' tracks')),
        epTracks(e), h('p', {}, bbcEpisode(e.pid)), playlistTools(slug + '-' + e.date, e.tracks.map((t) => [t[0], t[1]]))))));
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
      cols: [{ label: 'Artist', value: (r) => r.name, cell: (r) => artistLink(r.name, letter, r.id) }, ...(NOGENRES ? [] : [{ label: 'MusicBrainz', value: (r) => r.mbid, cell: (r) => (r.mbid ? mbLink(r.mbid) : '') }]),
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
        NOGENRES ? null : [h('dt', {}, 'MusicBrainz ID'), h('dd', {}, a.mbid ? [h('code', {}, a.mbid), '  ', h('a', { href: 'https://musicbrainz.org/artist/' + a.mbid, rel: 'noopener' }, 'View on MusicBrainz')] : 'Not supplied by the BBC for this artist')],
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
      return setPage(String(y), h('p', { class: 'tag' }, h('a', { href: '#/years' }, 'Years')), h('h1', {}, String(y)),
        h('p', { class: 'lede' }, fmt(e.plays) + ' plays · ' + fmt(e.episodes) + ' episodes · ' + fmt(e.artists) + ' different artists'),
        h('div', { class: 'two' },
          section('Most played tracks', dataTable({ cols: trackCols(), rows: e.tracks, filename: 'tracks-' + y, sortCol: 2 })),
          section('Most played artists', dataTable({ cols: artistRowCols, rows: e.top_artists, filename: 'artists-' + y, sortCol: 1 }))));
    }
    setPage('Years', h('h1', {}, 'Years'), h('p', { class: 'lede' }, 'Tracklists begin in mid-2008 and are complete from about 2010.'),
      barChart({ title: 'Tracks played per year', filename: 'plays-per-year', data: summary.by_year.map((r) => ({ label: String(r[0]), value: r[1] })), labelEvery: 1 }),
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

  async function downloads() {
    const meta = await getJSON('meta.json');
    const ONLYFILE = '{{DOWNLOADS}}';
    if (ONLYFILE) {
      return setPage('Downloads', h('h1', {}, 'Downloads'),
        h('p', { class: 'lede' }, 'Every play is in one plain CSV file, free to use for personal, non-commercial purposes (as of ' + fmtDate(meta.as_of) + ').'),
        section('The dataset', h('p', {}, h('a', { class: 'btn', href: BASE + ONLYFILE }, 'Download ' + ONLYFILE))),
        section('Download a chart', h('p', {}, 'Every table and chart on this site has CSV and TXT buttons that download exactly what is on screen. For example:'),
          h('ul', {}, h('li', {}, h('a', { href: '#/most-played/week' }, 'Most played this week')), h('li', {}, h('a', { href: '#/most-played/all' }, 'Most played of all time')), h('li', {}, h('a', { href: '#/years' }, 'Plays per year, and a top 100 for each year')), h('li', {}, h('a', { href: '#/shows' }, 'Every show; open one for a playlist of everything it has played')))));
    }
    const raw = (f) => REPO + '/raw/main/data/' + f;
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
          h('tbody', {}, files.map((f) => h('tr', {}, h('td', {}, h('code', {}, f[0])), h('td', {}, f[1]), h('td', {}, f[2]), h('td', {}, h('a', { class: 'btn', href: raw(f[0]), rel: 'noopener' }, 'Download')))),
            h('tr', {}, h('td', {}, h('code', {}, 'plays_YYYY.csv.gz')), h('td', {}, 'The full play-by-play record, one file per year (the source of all the others)'), h('td', {}, '4-6 MB each'), h('td', {}, h('a', { class: 'btn', href: REPO + '/tree/main/data/plays', rel: 'noopener' }, 'Browse'))),
            h('tr', {}, h('td', {}, h('code', {}, 'data-latest')), h('td', {}, 'The same CSV files, gzipped and rebuilt daily with the newest plays included (as of ' + fmtDate(meta.as_of) + ')'), h('td', {}, 'gzipped'), h('td', {}, h('a', { class: 'btn', href: REPO + '/releases/tag/data-latest', rel: 'noopener' }, 'Latest data'))))))),
      section('Download a chart', h('p', {}, 'Every table and chart on this site has CSV and TXT buttons that download exactly what is on screen, sorted and filtered the way you have it. For example:'),
        h('ul', {}, h('li', {}, h('a', { href: '#/most-played/week' }, 'Most played this week')), h('li', {}, h('a', { href: '#/most-played/all' }, 'Most played of all time')), h('li', {}, h('a', { href: '#/years' }, 'Plays per year, and a top 100 for each year')), h('li', {}, h('a', { href: '#/shows' }, 'Every show; open one for a playlist of everything it has played')))),
      section('Playlists', h('p', {}, 'Each show page has a table of every song it has played with a CSV download, ready to import into a playlist-converter service. The command-line program in the source code can also write per-show and per-genre playlist files.')),
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
