//! Web server for viewing sessions.

pub mod assets;
pub mod templates;

pub use assets::{content_type, StaticAssets};
pub use templates::{markdown_to_html, BlockView, SessionView, TemplateEngine, Templates};
