/* Time-series chart module: bars, histogram or box-and-whisker over a chosen period (week, month, quarter, year).
   No libraries, no network. Reusable: Independent.TimeChart(el, {days:[epoch day numbers], title, unit}).
   Draws SVG using the page's CSS colours (--ink, --mute, --line, --accent). */
(function (g) {
  var UNITS = ["Week", "Month", "Quarter", "Year"];
  var MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function ymd(day) { var d = new Date(day * 86400000); return [d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()]; }
  function key(day, u) {
    var t = ymd(day);
    if (u === 0) { var w = Math.floor((day + 3) / 7); return w; }
    if (u === 1) return t[0] * 12 + t[1];
    if (u === 2) return t[0] * 4 + Math.floor(t[1] / 3);
    return t[0];
  }
  function label(k, u) {
    if (u === 0) { var t = ymd(k * 7 - 3); return t[2] + " " + MON[t[1]] + " " + String(t[0]).slice(2); }
    if (u === 1) return MON[k % 12] + " " + String(Math.floor(k / 12)).slice(2);
    if (u === 2) return "Q" + (k % 4 + 1) + " " + String(Math.floor(k / 4)).slice(2);
    return String(k);
  }
  function el(n, a, t) { var e = document.createElementNS("http://www.w3.org/2000/svg", n); for (var k in a) e.setAttribute(k, a[k]); if (t != null) e.textContent = t; return e; }
  function nice(m) { if (m <= 0) return 1; var p = Math.pow(10, Math.floor(Math.log(m) / Math.LN10)), f = m / p; return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * p; }
  function quant(a, q) { var i = (a.length - 1) * q, lo = Math.floor(i), hi = Math.ceil(i); return a[lo] + (a[hi] - a[lo]) * (i - lo); }

  function TimeChart(host, opt) {
    var state = { unit: opt.unit == null ? 1 : opt.unit, type: "bar", days: opt.days || [] };
    host.innerHTML = "";
    var bar = document.createElement("div"); bar.className = "tools";
    var sl = document.createElement("input"); sl.type = "range"; sl.min = 0; sl.max = 3; sl.step = 1; sl.value = state.unit; sl.setAttribute("aria-label", "Period: week, month, quarter or year");
    var out = document.createElement("span"); out.className = "count"; out.style.marginRight = "0";
    var ty = document.createElement("select"); ty.setAttribute("aria-label", "Chart type");
    [["bar", "Bars (total per period)"], ["hist", "Histogram (how often each total happens)"], ["box", "Box and whisker (spread of daily counts)"]].forEach(function (o) { var x = document.createElement("option"); x.value = o[0]; x.textContent = o[1]; ty.appendChild(x); });
    var lab = document.createElement("label"); lab.textContent = "Period "; lab.appendChild(sl); lab.appendChild(document.createTextNode(" ")); lab.appendChild(out);
    bar.appendChild(lab); bar.appendChild(ty);
    var svgWrap = document.createElement("div"); svgWrap.style.overflowX = "auto";
    host.appendChild(bar); host.appendChild(svgWrap);

    function draw() {
      var u = state.unit; out.textContent = UNITS[u];
      svgWrap.innerHTML = "";
      var days = state.days;
      if (!days.length) { svgWrap.textContent = "Nothing to show for this selection."; return; }
      var W = 900, H = 280, L = 44, R = 10, T = 10, B = 44, ink = "var(--ink)", mute = "var(--mute)", line = "var(--line)";
      var s = el("svg", { viewBox: "0 0 " + W + " " + H, width: "100%", role: "img", "aria-label": opt.title || "Chart" });
      s.style.minWidth = "520px";
      var counts = {}, keys = [];
      days.forEach(function (d) { var k = key(d, u); if (!(k in counts)) { counts[k] = 0; keys.push(k); } counts[k]++; });
      var min = Math.min.apply(null, keys), max = Math.max.apply(null, keys), full = [];
      for (var k = min; k <= max; k++) { if (u === 2 && k % 4 > 3) continue; if (u === 1 && k % 12 > 11) continue; full.push(k); }
      var vals, xs, ymax, kind = state.type;
      function axis(maxv, fmt) {
        var step = nice(maxv / 4), n = Math.ceil(maxv / step);
        for (var i = 0; i <= n; i++) {
          var v = i * step, y = T + (H - T - B) * (1 - v / (n * step));
          s.appendChild(el("line", { x1: L, x2: W - R, y1: y, y2: y, stroke: line }));
          s.appendChild(el("text", { x: L - 6, y: y + 4, "text-anchor": "end", "font-size": 11, fill: mute }, fmt ? fmt(v) : v));
        }
        return n * step;
      }
      if (kind === "bar") {
        vals = full.map(function (k) { return counts[k] || 0; });
        ymax = axis(Math.max.apply(null, vals) || 1);
        var bw = (W - L - R) / full.length;
        vals.forEach(function (v, i) {
          var h = (H - T - B) * v / ymax, x = L + i * bw;
          var r = el("rect", { x: x + bw * 0.12, y: H - B - h, width: Math.max(1, bw * 0.76), height: h, fill: ink });
          r.appendChild(el("title", {}, label(full[i], u) + ": " + v)); s.appendChild(r);
        });
        var every = Math.ceil(full.length / 12);
        full.forEach(function (k, i) { if (i % every === 0) s.appendChild(el("text", { x: L + i * bw + bw / 2, y: H - B + 16, "text-anchor": "middle", "font-size": 11, fill: mute }, label(k, u))); });
        s.appendChild(el("text", { x: L, y: H - 6, "font-size": 11, fill: mute }, "Shows per " + UNITS[u].toLowerCase()));
      } else if (kind === "hist") {
        vals = full.map(function (k) { return counts[k] || 0; });
        var mx = Math.max.apply(null, vals) || 1, nb = Math.min(20, Math.max(5, Math.ceil(Math.sqrt(vals.length)))), bs = Math.max(1, Math.ceil((mx + 1) / nb)), bins = [];
        for (var b = 0; b < Math.ceil((mx + 1) / bs); b++) bins.push(0);
        vals.forEach(function (v) { bins[Math.floor(v / bs)]++; });
        ymax = axis(Math.max.apply(null, bins));
        var hw = (W - L - R) / bins.length;
        bins.forEach(function (c, i) {
          var h = (H - T - B) * c / ymax, x = L + i * hw;
          var r = el("rect", { x: x + 1, y: H - B - h, width: hw - 2, height: h, fill: ink });
          r.appendChild(el("title", {}, (i * bs) + "–" + ((i + 1) * bs - 1) + " shows: " + c + " " + UNITS[u].toLowerCase() + "s")); s.appendChild(r);
          if (bins.length <= 24 || i % 2 === 0) s.appendChild(el("text", { x: x + hw / 2, y: H - B + 16, "text-anchor": "middle", "font-size": 11, fill: mute }, i * bs));
        });
        s.appendChild(el("text", { x: L, y: H - 6, "font-size": 11, fill: mute }, "Shows in a " + UNITS[u].toLowerCase() + " (x) and how many " + UNITS[u].toLowerCase() + "s had that many (y)"));
      } else {
        var daily = {}, first = Math.min.apply(null, days), last = Math.max.apply(null, days);
        days.forEach(function (d) { daily[d] = (daily[d] || 0) + 1; });
        var groups = {};
        for (var d = first; d <= last; d++) { var gk = key(d, u); (groups[gk] = groups[gk] || []).push(daily[d] || 0); }
        var gks = full.filter(function (k) { return groups[k]; }), stats = gks.map(function (k) {
          var a = groups[k].slice().sort(function (x, y) { return x - y; }), q1 = quant(a, .25), q3 = quant(a, .75), iq = q3 - q1;
          var lo = a.filter(function (v) { return v >= q1 - 1.5 * iq; })[0], hi = a.filter(function (v) { return v <= q3 + 1.5 * iq; }).pop();
          return { q1: q1, med: quant(a, .5), q3: q3, lo: lo, hi: hi, out: a.filter(function (v) { return v < lo || v > hi; }) };
        });
        var top = Math.max.apply(null, stats.map(function (t) { return Math.max(t.hi, Math.max.apply(null, t.out.concat([0]))); })) || 1;
        ymax = axis(top);
        var xw = (W - L - R) / gks.length;
        function Y(v) { return T + (H - T - B) * (1 - v / ymax); }
        stats.forEach(function (t, i) {
          var cx = L + i * xw + xw / 2, w = Math.max(3, xw * 0.6);
          s.appendChild(el("line", { x1: cx, x2: cx, y1: Y(t.lo), y2: Y(t.hi), stroke: ink }));
          var bx = el("rect", { x: cx - w / 2, y: Y(t.q3), width: w, height: Math.max(1, Y(t.q1) - Y(t.q3)), fill: "none", stroke: ink });
          bx.appendChild(el("title", {}, label(gks[i], u) + ": median " + t.med + ", middle half " + t.q1 + "–" + t.q3 + " a day")); s.appendChild(bx);
          s.appendChild(el("line", { x1: cx - w / 2, x2: cx + w / 2, y1: Y(t.med), y2: Y(t.med), stroke: ink, "stroke-width": 2 }));
          t.out.slice(0, 6).forEach(function (o) { s.appendChild(el("circle", { cx: cx, cy: Y(o), r: 1.8, fill: mute })); });
        });
        var ev = Math.ceil(gks.length / 12);
        gks.forEach(function (k, i) { if (i % ev === 0) s.appendChild(el("text", { x: L + i * xw + xw / 2, y: H - B + 16, "text-anchor": "middle", "font-size": 11, fill: mute }, label(k, u))); });
        s.appendChild(el("text", { x: L, y: H - 6, "font-size": 11, fill: mute }, "Shows a day, within each " + UNITS[u].toLowerCase() + ": box = middle half, line = median, whiskers = typical range"));
      }
      svgWrap.appendChild(s);
    }
    sl.addEventListener("input", function () { state.unit = +sl.value; draw(); });
    ty.addEventListener("change", function () { state.type = ty.value; draw(); });
    draw();
    return { set: function (days) { state.days = days; draw(); } };
  }
  g.Independent = g.Independent || {}; g.Independent.TimeChart = TimeChart;
})(window);
