//! # Jesus Is Savior Crawler
//!
//! A high-performance web scraper that converts HTML content to Markdown format
//! with a simplified JSON schema. Features include:
//!
//! - Direct HTML-to-Markdown conversion
//! - Automatic asset downloading (images, videos, audio)
//! - Smart caching and progress tracking
//! - Concurrent processing with rate limiting
//! - Clean, readable output format
//!
//! ## Usage
//!
//! ```rust,no_run
//! use jesus_is_savior_crawler::html_to_markdown;
//! use scraper::Html;
//!
//! #[tokio::main]
//! async fn main() {
//!     let html = "<p>Hello <strong>world</strong>!</p>";
//!     let document = Html::parse_document(html);
//!     let markdown = html_to_markdown(&document, "https://example.com").await;
//!     println!("{}", markdown); // "Hello **world**!"
//! }
//! ```

pub mod markdown_flattener;

// Re-export the main function for convenience
pub use markdown_flattener::html_to_markdown;

// Common types and structures
pub use serde::{Deserialize, Serialize};

/// Post schema matching schema.json format
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Post {
    /// Author avatar URL
    pub avatar: String,
    /// Author name
    pub username: String,
    /// Original source URL
    pub url: String,
    /// Page title (optional)
    pub title: Option<String>,
    /// Date published (YYYY-MM-DD format)
    pub date_published: Option<String>,
    /// Date updated (YYYY-MM-DD format) 
    pub date_updated: Option<String>,
    /// Auto-extracted tags from URL
    pub tags: Vec<String>,
    /// Markdown-formatted content
    pub content: String,
}
