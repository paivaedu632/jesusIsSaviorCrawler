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

/// Simplified Post schema with direct Markdown content
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Post {
    /// Author/site avatar image URL
    pub avatar: String,
    /// Author or site name
    pub username: String,
    /// Original source URL
    pub url: String,
    /// Page title (optional)
    pub title: Option<String>,
    /// Markdown-formatted content
    pub content: String,
    /// Auto-extracted tags from URL segments
    pub tags: Vec<String>,
}
