//! Static HTML pages for search engines (and for anyone without JavaScript).
//!
//! The interactive site is a single-page app, but a search engine cannot index what a script
//! draws later. So `bbc6 site` also writes a real HTML page at a real address for every show,
//! the top artists, every year, every chart and so on, each with its own title, description,
//! canonical link, Open Graph tags and schema.org data, plus `sitemap.xml`, `robots.txt` and a
//! `404.html` that lets the app open any address that has no pre-built page.
//!
//! The template is `site/index.html`; the `{{MARKERS}}` in it are filled in here. When the
//! script loads it replaces the plain page with the interactive one.

use crate::util::*;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const SITE_NAME: &str = "BBC Radio 6 Music Collection";

/// HTML-escape text.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// Percent-encode for a URL query or path part.
pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Plain search links to places where a track can be heard. Nothing is embedded or tracked:
/// they are ordinary links that open the other site's own search.
pub fn listen_links(artist: &str, title: &str) -> String {
    let q = enc(&format!("{artist} {title}").trim().to_string());
    let l = |name: &str, short: &str, url: String| {
        format!("<a class=\"lk\" href=\"{url}\" rel=\"noopener noreferrer nofollow\" title=\"Search {name}\" aria-label=\"Search {name} for {} - {}\">{short}</a>", esc(artist), esc(title))
    };
    [
        l("YouTube", "YT", format!("https://www.youtube.com/results?search_query={q}")),
        l("Spotify", "SP", format!("https://open.spotify.com/search/{q}")),
        l("SoundCloud", "SC", format!("https://soundcloud.com/search?q={q}")),
        l("Bandcamp", "BC", format!("https://bandcamp.com/search?q={q}")),
    ]
    .join(" ")
}

pub struct Emitter {
    tpl: String,
    out: PathBuf,
    origin: String,
    base: String,
    asof: String,
    verify: String,
    urls: Vec<String>,
    /// Set SITE_NOINDEX=1 for a copy of a site that must not compete with the original in search.
    noindex: bool,
}

impl Emitter {
    pub fn new(src: &Path, out: &Path, origin: &str, base: &str, asof: &str) -> Res<Self> {
        let tpl = fs::read_to_string(src.join("index.html")).map_err(err("cannot read the site template index.html"))?;
        let mut base = base.to_string();
        if !base.starts_with('/') {
            base.insert(0, '/');
        }
        if !base.ends_with('/') {
            base.push('/');
        }
        // Optional search-engine ownership tags, set as repository variables.
        let mut verify = String::new();
        for (var, name) in [("GOOGLE_SITE_VERIFICATION", "google-site-verification"), ("BING_SITE_VERIFICATION", "msvalidate.01")] {
            if let Ok(v) = std::env::var(var) {
                let v = v.trim();
                if !v.is_empty() && v.len() < 200 {
                    verify.push_str(&format!("<meta name=\"{name}\" content=\"{}\">\n", esc(v)));
                }
            }
        }
        // copy assets
        copy_dir(&src.join("assets"), &out.join("assets"))?;
        Ok(Emitter { tpl, out: out.to_path_buf(), origin: origin.trim_end_matches('/').to_string(), base, asof: asof.to_string(), verify, urls: Vec::new(), noindex: std::env::var("SITE_NOINDEX").map(|v| v == "1").unwrap_or(false) })
    }

    /// Site-relative address, e.g. `u("show/x/")` is `/Repo/show/x/`.
    pub fn u(&self, rel: &str) -> String {
        format!("{}{}", self.base, rel)
    }

    pub fn count(&self) -> usize {
        self.urls.len()
    }

    fn fill(&self, path: &str, title: &str, desc: &str, main_attr: &str, body: &str, ld: Option<&Value>, noindex: bool) -> String {
        let full = if title.is_empty() { "BBC Radio 6 Music Collection - every track played on BBC Radio 6 Music".to_string() } else { format!("{title} - {SITE_NAME}") };
        let canon = format!("{}{}{}", self.origin, self.base, path);
        let ld = ld
            .map(|v| format!("<script type=\"application/ld+json\">{}</script>\n", serde_json::to_string(v).unwrap_or_default().replace("</", "<\\/")))
            .unwrap_or_default();
        self.tpl
            .replace("{{TITLE}}", &esc(&full))
            .replace("{{DESC}}", &esc(desc))
            .replace("{{CANON}}", &esc(&canon))
            .replace("{{ROBOTS}}", if noindex { "<meta name=\"robots\" content=\"noindex\">\n" } else { "" })
            .replace("{{VERIFY}}", &self.verify)
            .replace("{{JSONLD}}", &ld)
            .replace("{{MAINATTR}}", main_attr)
            .replace("{{MAIN}}", body)
            .replace("{{BASE}}", &self.base)
    }

    fn write(&self, rel_dir: &str, html: &str) -> Res<()> {
        let dir = self.out.join(rel_dir);
        fs::create_dir_all(&dir).map_err(err("cannot create folder"))?;
        fs::write(dir.join("index.html"), html).map_err(err("cannot write page"))
    }

    /// Write one page. `path` is like `show/x/` (empty for the home page). `keep` pages
    /// (About) are not replaced by the app.
    pub fn page(&mut self, path: &str, title: &str, desc: &str, body: &str, ld: Option<Value>, keep: bool) -> Res<()> {
        let attr = if keep { "data-keep" } else { "data-static" };
        let html = self.fill(path, title, desc, attr, body, ld.as_ref(), self.noindex);
        self.write(path.trim_end_matches('/'), &html)?;
        self.urls.push(path.to_string());
        Ok(())
    }

    /// Sitemap, robots.txt and 404.html.
    pub fn finish(self) -> Res<()> {
        let mut sm = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for u in &self.urls {
            sm.push_str(&format!("<url><loc>{}{}{}</loc><lastmod>{}</lastmod></url>\n", self.origin, self.base, esc(u), self.asof));
        }
        sm.push_str("</urlset>\n");
        if self.urls.len() > 50_000 {
            return Err("sitemap would exceed 50,000 URLs".into());
        }
        fs::write(self.out.join("sitemap.xml"), sm).map_err(err("cannot write sitemap.xml"))?;
        fs::write(self.out.join("robots.txt"), format!("User-agent: *\nAllow: /\n\nSitemap: {}{}sitemap.xml\n", self.origin, self.base)).map_err(err("cannot write robots.txt"))?;
        let nf = self.fill("", "Page not found", "This page does not exist.", "", "<p class=\"loading\">Loading</p>", None, true);
        fs::write(self.out.join("404.html"), nf).map_err(err("cannot write 404.html"))
    }

    // ------------------------------------------------------------ body pieces

    pub fn artist_link(&self, name: &str, id: &str, letter: &str, pages: &HashSet<String>) -> String {
        if !id.is_empty() && pages.contains(id) {
            format!("<a href=\"{}\">{}</a>", self.u(&format!("artist/{letter}/{id}/")), esc(name))
        } else {
            esc(name)
        }
    }

    /// Rows `[artist, title, plays, artist id, letter]`.
    pub fn track_table(&self, rows: &[Value], pages: &HashSet<String>, n: usize) -> String {
        let mut s = String::from("<div class=\"tablewrap\"><table><thead><tr><th class=\"n\">#</th><th>Artist</th><th>Track</th><th class=\"n\">Plays</th></tr></thead><tbody>");
        for (i, r) in rows.iter().take(n).enumerate() {
            s.push_str(&format!(
                "<tr><td class=\"n rank\">{}</td><td>{}</td><td>{}</td><td class=\"n\">{}</td></tr>",
                i + 1,
                self.artist_link(r[0].as_str().unwrap_or(""), r[3].as_str().unwrap_or(""), r[4].as_str().unwrap_or(""), pages),
                esc(r[1].as_str().unwrap_or("")),
                r[2]
            ));
        }
        s.push_str("</tbody></table></div>");
        s
    }

    /// Rows `[name, id, letter, plays]`.
    pub fn artist_table(&self, rows: &[Value], pages: &HashSet<String>, n: usize) -> String {
        let mut s = String::from("<div class=\"tablewrap\"><table><thead><tr><th class=\"n\">#</th><th>Artist</th><th class=\"n\">Plays</th></tr></thead><tbody>");
        for (i, r) in rows.iter().take(n).enumerate() {
            s.push_str(&format!(
                "<tr><td class=\"n rank\">{}</td><td>{}</td><td class=\"n\">{}</td></tr>",
                i + 1,
                self.artist_link(r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or(""), r[2].as_str().unwrap_or(""), pages),
                r[3]
            ));
        }
        s.push_str("</tbody></table></div>");
        s
    }

    /// One episode: heading line, BBC link and tracklist. Tracks are `[artist, title, id, letter]`.
    pub fn episode(&self, e: &Value, heading: &str, pages: &HashSet<String>) -> String {
        let mut s = format!("<h3>{}</h3>", esc(heading));
        if let Some(pid) = e["pid"].as_str().filter(|p| !p.is_empty()) {
            s.push_str(&format!("<p class=\"meta\"><a href=\"https://www.bbc.co.uk/programmes/{}\" rel=\"noopener\">This episode on the BBC</a></p>", esc(pid)));
        }
        s.push_str("<ol>");
        for t in e["tracks"].as_array().map(|v| v.as_slice()).unwrap_or(&[]) {
            s.push_str(&format!(
                "<li>{} - {}</li>",
                self.artist_link(t[0].as_str().unwrap_or(""), t[2].as_str().unwrap_or(""), t[3].as_str().unwrap_or(""), pages),
                esc(t[1].as_str().unwrap_or(""))
            ));
        }
        s.push_str("</ol>");
        s
    }
}

fn copy_dir(from: &Path, to: &Path) -> Res<()> {
    fs::create_dir_all(to).map_err(err("cannot create folder"))?;
    for entry in fs::read_dir(from).map_err(err(&format!("cannot read {}", from.display())))?.flatten() {
        let (p, t) = (entry.path(), to.join(entry.file_name()));
        if p.is_dir() {
            copy_dir(&p, &t)?;
        } else {
            fs::copy(&p, &t).map_err(err("cannot copy asset"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_and_encoding() {
        assert_eq!(esc("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#39;");
        assert_eq!(enc("Björk & Co"), "Bj%C3%B6rk%20%26%20Co");
    }

    #[test]
    fn listen_links_are_plain_search_links() {
        let l = listen_links("The Fall", "Hit the North");
        assert!(l.contains("youtube.com/results?search_query=The%20Fall%20Hit%20the%20North"));
        assert!(l.contains("noopener noreferrer nofollow"));
        assert!(!l.contains("<script") && !l.contains("<iframe"));
    }
}
