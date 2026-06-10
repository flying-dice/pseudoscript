//! Wraps an SSR result into a complete HTML document.
//!
//! The Svelte renderer returns only the body markup and any `<svelte:head>`
//! content; Rust owns the document shell — doctype, `<html data-theme>`, the
//! font + stylesheet links, the theme pre-paint script, and the page's classic
//! deferred scripts. Scripts are never `type="module"` and shared data ships as
//! JS-global files, so the site works opened from disk (`file://`, where Chrome
//! blocks module scripts and `fetch`). Only the universe page embeds its props
//! (`window.__DATA__`) — its 3D island reads them; every other page ships no
//! page data and `client.js` only progressively enhances the SSR markup.

use crate::escape::escape;
use crate::props::{PageProps, RenderedPage};
use crate::render::UNIVERSE_PATH;

/// Applies the visitor's theme before first paint: the stored preference wins,
/// else the OS scheme when the site default is `system`, else the configured
/// default. A static string — deterministic output.
const THEME_PREPAINT: &str = "<script>(function(){var d=document.documentElement;\
var t=null;try{t=localStorage.getItem(\"pds-doc-theme\")}catch(e){}\
if(t!==\"light\"&&t!==\"dark\")t=null;\
if(!t){var c=d.getAttribute(\"data-theme\");\
t=c===\"system\"?(window.matchMedia&&matchMedia(\"(prefers-color-scheme: dark)\").matches?\"dark\":\"light\"):c}\
d.setAttribute(\"data-theme\",t);})();</script>";

/// Builds the full HTML page. `path` is the site-relative output path (used to
/// compute the `../` prefix to the root assets and to gate the universe-only
/// assets); `props_json` is the exact serialised props string, embedded only on
/// the universe page; `rendered` is the SSR output.
pub(crate) fn wrap(
    path: &str,
    props: &PageProps,
    props_json: &str,
    rendered: &RenderedPage,
) -> String {
    let prefix = "../".repeat(path.matches('/').count());
    let title = page_title(path, props);
    let theme = escape(&props.site.theme);
    let head = &rendered.head;
    let body = &rendered.body;
    let page_scripts = if path == UNIVERSE_PATH {
        format!(
            "<script>window.__DATA__ = {};</script>\n\
             <script defer src=\"{prefix}universe.js\"></script>\n",
            script_safe_json(props_json),
        )
    } else {
        String::new()
    };
    format!(
        "<!doctype html>\n\
<html lang=\"en\" data-theme=\"{theme}\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n\
{THEME_PREPAINT}\n\
<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n\
<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n\
<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?\
family=Bricolage+Grotesque:opsz,wght@12..96,600;12..96,700&\
family=Hanken+Grotesk:wght@400;500;600&\
family=JetBrains+Mono:wght@400;500;600&display=swap\">\n\
<link rel=\"stylesheet\" href=\"{prefix}style.css\">\n\
{head}\
</head>\n\
<body>\n\
<div id=\"app\">{body}</div>\n\
{page_scripts}\
<script defer src=\"{prefix}client.js\"></script>\n\
</body>\n\
</html>\n",
    )
}

/// The `<title>`: the page's breadcrumb leaf before the site name, so tabs and
/// history read which page is which.
fn page_title(path: &str, props: &PageProps) -> String {
    let leaf = props
        .crumbs
        .last()
        .map(|crumb| crumb.label.as_str())
        .filter(|_| path != "index.html");
    match leaf {
        Some(leaf) => format!("{} — {}", escape(leaf), escape(&props.site.name)),
        None => escape(&props.site.name).into_owned(),
    }
}

/// Makes a JSON string safe to embed inside a `<script>` element: `</script>`
/// cannot appear in `serde_json` output for ordinary text, but a model could
/// carry the literal `<` sequence in prose. Escaping `<` as its JSON unicode
/// escape keeps the element from closing early while staying byte-identical
/// JSON for the client to parse.
fn script_safe_json(props_json: &str) -> String {
    props_json.replace('<', "\\u003c")
}
