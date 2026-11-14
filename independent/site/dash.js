/* Independent Archive dashboard: one dashboard, filtered by a station (and region) slicer. No libraries, no network
   beyond data/dash.json from this site. Reads ?station=slug,slug from the address so station pages can link in. */
(function () {
  var $ = function (id) { return document.getElementById(id); };
  var base = document.currentScript.getAttribute("data-base") || "";
  var DOW = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
  fetch(base + "data/dash.json").then(function (r) { return r.json(); }).then(start).catch(function () { $("dash").textContent = "The dashboard data could not be loaded."; });

  function start(D) {
    var picked = {}, region = "", q = "";
    (new URLSearchParams(location.search).get("station") || "").split(",").forEach(function (s) { if (s) picked[s] = true; });
    var S = D.st.map(function (s) { return { slug: s[0], name: s[1], region: s[2], city: s[3], eps: 0, tracks: 0 }; });
    var chart = window.Independent.TimeChart($("tc"), { days: [], unit: 1, title: "Shows over time" });

    // slicer
    var list = $("slist");
    S.forEach(function (s, i) {
      var l = document.createElement("label"); l.className = "pick"; l.dataset.i = i;
      var c = document.createElement("input"); c.type = "checkbox"; c.checked = !!picked[s.slug]; c.addEventListener("change", function () { if (c.checked) picked[s.slug] = true; else delete picked[s.slug]; render(); });
      l.appendChild(c); l.appendChild(document.createTextNode(" " + s.name)); list.appendChild(l); s.box = c; s.row = l;
    });
    var regs = {}; S.forEach(function (s) { regs[s.region] = 1; });
    Object.keys(regs).sort().forEach(function (r) { var o = document.createElement("option"); o.textContent = r; $("reg").appendChild(o); });
    $("reg").addEventListener("change", function () { region = $("reg").value; render(); });
    $("sq").addEventListener("input", function () { q = $("sq").value.toLowerCase(); S.forEach(function (s) { s.row.hidden = q && s.name.toLowerCase().indexOf(q) < 0; }); });
    $("clear").addEventListener("click", function () { picked = {}; region = ""; $("reg").value = ""; S.forEach(function (s) { s.box.checked = false; }); render(); });

    function bars(host, rows, fmt) {
      host.innerHTML = "";
      var m = Math.max.apply(null, rows.map(function (r) { return r[1]; })) || 1;
      rows.forEach(function (r) {
        var d = document.createElement("div"); d.className = "hb";
        var n = document.createElement("span"); n.textContent = r[0];
        var b = document.createElement("i"); b.style.width = Math.max(1, 100 * r[1] / m) + "%";
        var v = document.createElement("em"); v.textContent = fmt ? fmt(r[1]) : r[1].toLocaleString();
        d.appendChild(n); d.appendChild(b); d.appendChild(v); host.appendChild(d);
      });
      if (!rows.length) host.textContent = "Nothing for this selection.";
    }

    function render() {
      var any = Object.keys(picked).length > 0;
      var sel = D.ep.filter(function (e) { var s = S[e[0]]; return (!any || picked[s.slug]) && (!region || s.region === region); });
      var days = sel.map(function (e) { return e[1]; });
      var tags = {}, dow = [0, 0, 0, 0, 0, 0, 0], hours = []; for (var h = 0; h < 24; h++) hours.push(0);
      var st = {}, trk = 0, hasHour = 0, gh = {};
      S.forEach(function (s) { s.eps = 0; s.tracks = 0; });
      sel.forEach(function (e) {
        S[e[0]].eps++; S[e[0]].tracks += e[4]; trk += e[4]; st[e[0]] = 1;
        dow[(e[1] + 3) % 7]++;
        e[2].forEach(function (t) { tags[t] = (tags[t] || 0) + 1; });
        if (e[3] >= 0) { hours[e[3]]++; hasHour++; e[2].forEach(function (t) { (gh[t] = gh[t] || new Array(24).fill(0))[e[3]]++; }); }
      });
      var lo = days.length ? Math.min.apply(null, days) : 0, hi = days.length ? Math.max.apply(null, days) : 0;
      function dstr(d) { return new Date(d * 86400000).toISOString().slice(0, 10); }
      $("stats").innerHTML = [[sel.length.toLocaleString(), "shows"], [Object.keys(st).length, "stations with shows"], [trk.toLocaleString(), "tracks listed"],
        [days.length ? dstr(lo).slice(0, 4) + "–" + dstr(hi).slice(0, 4) : "–", "years covered"]].map(function (x) { return '<div class="stat"><b>' + x[0] + "</b><span>" + x[1] + "</span></div>"; }).join("");
      chart.set(days);
      var tl = Object.keys(tags).map(function (k) { return [D.tags[k], tags[k], +k]; }).sort(function (a, b) { return b[1] - a[1]; });
      bars($("genres"), tl.slice(0, 15));
      bars($("dow"), DOW.map(function (n, i) { return [n, dow[i]]; }));
      bars($("hours"), hours.map(function (n, i) { return [(i < 10 ? "0" : "") + i + ":00", n]; }).filter(function (r, i) { return hasHour ? true : false; }));
      $("hournote").textContent = hasHour ? hasHour.toLocaleString() + " of " + sel.length.toLocaleString() + " shows have a known start time (so far Noods Radio's regular slots)." : "No start times known for this selection yet. They come with the weekly timetables.";
      // genre by hour heatmap (top 8 genres with a known time)
      var top = Object.keys(gh).map(function (k) { return [k, gh[k].reduce(function (a, b) { return a + b; }, 0)]; }).sort(function (a, b) { return b[1] - a[1]; }).slice(0, 8);
      var mx = 1; top.forEach(function (t) { gh[t[0]].forEach(function (v) { if (v > mx) mx = v; }); });
      var html = '<table class="hm"><thead><tr><th></th>' + hours.map(function (_, i) { return "<th>" + i + "</th>"; }).join("") + "</tr></thead><tbody>";
      top.forEach(function (t) { html += "<tr><th>" + D.tags[t[0]] + "</th>" + gh[t[0]].map(function (v) { return '<td title="' + v + '" style="background:color-mix(in srgb,var(--ink) ' + Math.round(100 * v / mx) + '%,transparent)"></td>'; }).join("") + "</tr>"; });
      $("heat").innerHTML = top.length ? html + "</tbody></table>" : "";
      // regions
      var rg = {}; sel.forEach(function (e) { var r = S[e[0]].region; rg[r] = (rg[r] || 0) + 1; });
      bars($("regions"), Object.keys(rg).map(function (k) { return [k, rg[k]]; }).sort(function (a, b) { return b[1] - a[1]; }));
      // stations table
      var rows = S.filter(function (s) { return (!any || picked[s.slug]) && (!region || s.region === region); }).sort(function (a, b) { return b.eps - a.eps || a.name.localeCompare(b.name); });
      $("tbody").innerHTML = rows.map(function (s) { return '<tr><td><a href="' + base + "station/" + s.slug + '/">' + s.name + "</a></td><td>" + s.city + "</td><td>" + s.region + "</td><td>" + s.eps.toLocaleString() + "</td><td>" + s.tracks.toLocaleString() + "</td></tr>"; }).join("");
      $("sel").textContent = any ? Object.keys(picked).length + " station" + (Object.keys(picked).length > 1 ? "s" : "") + " selected" : "All stations";
    }
    render();
  }
})();
