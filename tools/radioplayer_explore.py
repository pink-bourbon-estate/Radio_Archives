#!/usr/bin/env python3
"""One-off: open Radioplayer's station search in a headless browser and record the JSON the page loads
(URLs and the start of each body), to see how its station directory is served.

Radioplayer's robots.txt allows the site but disallows /api/ (where the directory comes from). The project owner decided
(13 Nov 2026) to treat this like Jazz FM (docs/JAZZ-FM.md): one small pass to list station names, towns and websites, as an
identified bot; nothing else is taken. Writes independent/data/radioplayer/_explore.json."""
import json, os, sys
from playwright.sync_api import sync_playwright

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; one small pass)")
seen = []


def main():
    with sync_playwright() as p:
        b = p.chromium.launch()
        pg = b.new_context(user_agent=UA).new_page()

        def on(resp):
            try:
                ct = resp.headers.get("content-type", "")
                if "json" in ct and len(seen) < 60:
                    seen.append({"url": resp.url, "status": resp.status, "body": resp.text()[:2500]})
            except Exception as e:  # noqa: BLE001
                seen.append({"url": resp.url, "error": str(e)[:80]})

        pg.on("response", on)
        pg.goto("https://play.radioplayer.org/en/search", wait_until="networkidle", timeout=60000)
        pg.wait_for_timeout(3000)
        info = {"buttons": pg.eval_on_selector_all("button,a,input", "els=>els.map(e=>[e.tagName,(e.innerText||e.placeholder||e.getAttribute('aria-label')||'').slice(0,50),e.getAttribute('href')||e.type||''])")[:60]}
        steps = []
        for label in ("Discover stations", "Live"):
            try:
                pg.get_by_text(label, exact=True).first.click(timeout=5000)
                pg.wait_for_timeout(5000)
                steps.append({"clicked": label, "url": pg.url, "text": pg.inner_text("body")[:1500],
                              "links": pg.eval_on_selector_all("a", "els=>els.map(e=>[e.innerText.slice(0,40),e.href]).slice(0,40)")})
                pg.goto("https://play.radioplayer.org/en/search", wait_until="networkidle", timeout=60000)
            except Exception as e:  # noqa: BLE001
                steps.append({"clicked": label, "error": str(e)[:100]})
        for q in ("Bristol",):
            try:
                inp = pg.locator("input").first
                inp.fill(q)
                inp.press("Enter")
                pg.wait_for_timeout(5000)
                steps.append({"search": q, "url": pg.url, "text": pg.inner_text("body")[:1500]})
            except Exception as e:  # noqa: BLE001
                steps.append({"search": q, "error": str(e)[:100]})
        pg.screenshot(path="independent/data/radioplayer/_screen.png")
        out = {"final_url": pg.url, "info": info, "steps": steps, "responses": seen}
        os.makedirs("independent/data/radioplayer", exist_ok=True)
        json.dump(out, open("independent/data/radioplayer/_explore.json", "w"), indent=1, ensure_ascii=False)
        print(len(seen), "JSON responses;", [s["url"][:90] for s in seen][:20])
        b.close()


if __name__ == "__main__":
    main()
