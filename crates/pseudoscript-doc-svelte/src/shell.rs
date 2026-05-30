//! Wraps an SSR result into a complete HTML document.
//!
//! The Svelte renderer returns only the body markup and any `<svelte:head>`
//! content; Rust owns the document shell — doctype, `<html data-theme>`, the
//! font + stylesheet links, the hydration data, and the client script. The
//! rendered `body` is mounted into `#app`, which the client hydrates.

use pseudoscript_doc::escape;

use crate::props::{PageProps, RenderedPage};

/// Builds the full HTML page. `path` is the site-relative output path (used to
/// compute the `../` prefix to the root assets); `props_json` is the exact
/// serialised props string, embedded for hydration; `rendered` is the SSR
/// output.
pub(crate) fn wrap(
    path: &str,
    props: &PageProps,
    props_json: &str,
    rendered: &RenderedPage,
) -> String {
    let prefix = "../".repeat(path.matches('/').count());
    let title = escape(&props.site.name);
    let theme = escape(&props.site.theme);
    let data = script_safe_json(props_json);
    let head = &rendered.head;
    let body = &rendered.body;
    format!(
        "<!doctype html>\n\
<html lang=\"en\" data-theme=\"{theme}\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n\
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
<script>window.__DATA__ = {data};</script>\n\
<script type=\"module\" src=\"{prefix}client.js\"></script>\n\
</body>\n\
</html>\n",
    )
}

/// Makes a JSON string safe to embed inside a `<script>` element: `</script>`
/// cannot appear in `serde_json` output for ordinary text, but a model could
/// carry the literal `<` sequence in prose. Escaping `<` as its JSON unicode
/// escape keeps the element from closing early while staying byte-identical
/// JSON for the client to parse.
fn script_safe_json(props_json: &str) -> String {
    props_json.replace('<', "\\u003c")
}
