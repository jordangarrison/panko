//! Embedded static assets using rust-embed.

use rust_embed::Embed;

/// Static assets embedded in the binary.
#[derive(Embed)]
#[folder = "src/assets/"]
pub struct StaticAssets;

/// Get the content type for a file based on its extension.
pub fn content_type(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_assets_contains_styles() {
        let styles = StaticAssets::get("styles.css");
        assert!(styles.is_some(), "styles.css should be embedded");
    }

    #[test]
    fn test_static_assets_contains_htmx() {
        let htmx = StaticAssets::get("htmx.min.js");
        assert!(htmx.is_some(), "htmx.min.js should be embedded");
    }

    #[test]
    fn test_static_assets_contains_keyboard() {
        let keyboard = StaticAssets::get("keyboard.js");
        assert!(keyboard.is_some(), "keyboard.js should be embedded");
    }

    #[test]
    fn test_content_type_css() {
        assert_eq!(content_type("styles.css"), "text/css; charset=utf-8");
    }

    #[test]
    fn test_content_type_js() {
        assert_eq!(
            content_type("htmx.min.js"),
            "application/javascript; charset=utf-8"
        );
    }

    #[test]
    fn test_content_type_html() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
    }

    #[test]
    fn test_content_type_unknown() {
        assert_eq!(content_type("unknown.xyz"), "application/octet-stream");
    }
}
