#!/usr/bin/env python3
"""Save public pages (after checking robots.txt) into independent/sources/raw/ so station lists can be read and imported.

    python3 tools/fetch_sources.py URL [URL ...]

Used by the 'Independent fetch sources' workflow, because the project's sandbox cannot reach most sites.
A page whose robots.txt disallows us is skipped and reported, never fetched. One request every 2 seconds.
"""
import os, re, sys, time, urllib.error, urllib.parse, urllib.request, urllib.robotparser

UA = ("RadioArchivesBot/0.2 (+https://github.com/pink-bourbon-estate/Radio_Archives; "
      "unofficial non-commercial archive; a few requests per site)")


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read(3_000_000)


def main(urls):
    os.makedirs("independent/sources/raw", exist_ok=True)
    for url in urls:
        u = urllib.parse.urlparse(url)
        rp = urllib.robotparser.RobotFileParser()
        try:
            rp.parse(get(f"{u.scheme}://{u.netloc}/robots.txt").decode("utf-8", "replace").splitlines())
            ok = rp.can_fetch(UA, url)
        except urllib.error.HTTPError:
            ok = True  # no robots.txt
        except Exception as e:
            print("robots.txt unreadable for", u.netloc, e, "- skipping")
            continue
        if not ok:
            print("robots.txt disallows", url, "- skipped")
            continue
        time.sleep(2)
        try:
            body = get(url)
        except Exception as e:
            print("failed", url, e)
            continue
        name = re.sub(r"[^A-Za-z0-9]+", "-", u.netloc + u.path + ("?" + u.query if u.query else "")).strip("-")[:100]
        open(f"independent/sources/raw/{name}.html", "wb").write(body)
        print("saved", url, len(body), "bytes ->", name)


if __name__ == "__main__":
    main(sys.argv[1:])
