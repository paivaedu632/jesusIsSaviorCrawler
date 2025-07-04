use scraper::{Html, ElementRef, Node, Selector};
use regex::Regex;
use url::Url;
use std::collections::HashMap;
use std::path::Path;
use reqwest::Client;
use sha2::{Sha256, Digest};
use tokio::fs;
use html_escape::decode_html_entities;

/// Converts HTML document to markdown format with asset downloading
/// 
/// Walks the DOM and appends text nodes with normalized whitespace. Converts:
/// - `<br>/<p>/<div>` to `\n\n`
/// - `<strong>/<b>` to `**text**`
/// - `<em>/<i>` to `*text*`
/// - `<img>` to `![alt](local_or_remote_url)`
/// - `<a>` to `[text](url)`
/// - Internal media links (`.mp3`, `.mp4`, `.wav`, etc.) to `🔊 [Audio](local_path)` / `▶️ [Video](local_path)`
/// 
/// Reuses existing asset download helpers and returns local paths to embed in markdown.
/// Finally, collapses multiple blank lines and trims.
/// 
/// # Arguments
/// * `document` - The parsed HTML document
/// * `base_url` - Base URL for resolving relative links
/// 
/// # Returns
/// A markdown string with normalized whitespace and downloaded assets
/// 
/// # Example
/// ```rust,no_run
/// use scraper::Html;
/// use jesus_is_savior_crawler::html_to_markdown;
/// 
/// #[tokio::main]
/// async fn main() {
///     let html = "<p>Hello <strong>world</strong>!</p>";
///     let document = Html::parse_document(html);
///     let markdown = html_to_markdown(&document, "https://example.com").await;
///     println!("{}", markdown); // "Hello **world**!"
/// }
/// ```
pub async fn html_to_markdown(document: &Html, base_url: &str) -> String {
    let mut converter = HtmlToMarkdownConverter::new(base_url).await;
    converter.convert(document).await
}

struct HtmlToMarkdownConverter {
    base_url: String,
    client: Client,
    asset_cache: HashMap<String, String>,
}

impl HtmlToMarkdownConverter {
    async fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        // Create asset directories
        let dirs = ["ui/public/images", "ui/public/videos", "ui/public/audio"];
        for dir in dirs {
            fs::create_dir_all(dir).await.ok();
        }

        Self {
            base_url: base_url.to_string(),
            client,
            asset_cache: HashMap::new(),
        }
    }

    async fn convert(&mut self, document: &Html) -> String {
        // Get the body element, or fall back to root if no body
        let root_element = if let Some(body) = document.select(&Selector::parse("body").unwrap()).next() {
            body
        } else {
            document.root_element()
        };

        // First pass: collect and download assets
        self.collect_assets(&root_element).await;

        // Second pass: generate markdown
        let mut markdown = String::new();
        self.process_element(&root_element, &mut markdown);

        // Clean up the markdown
        self.clean_markdown(&markdown)
    }

    async fn collect_assets(&mut self, element: &ElementRef<'_>) {
        for descendant in element.descendants() {
            if let Some(el_ref) = ElementRef::wrap(descendant) {
                let tag = el_ref.value().name();
                match tag {
                    "img" => {
                        if let Some(src) = el_ref.value().attr("src") {
                            let img_url = decode_html_entities(src).to_string();
                            let abs_url = self.resolve_url(&img_url);
                            if self.is_internal(&abs_url) {
                                if let Some(local_path) = self.download_asset(&abs_url, "images").await {
                                    self.asset_cache.insert(abs_url, local_path);
                                }
                            }
                        }
                    }
                    "a" => {
                        if let Some(href) = el_ref.value().attr("href") {
                            let link_url = decode_html_entities(href).to_string();
                            let abs_url = self.resolve_url(&link_url);
                            
                            // Check if it's a media file
                            if self.is_media_file(&abs_url) {
                                if self.is_internal(&abs_url) {
                                    let folder = if self.is_audio_file(&abs_url) { "audio" } else { "videos" };
                                    if let Some(local_path) = self.download_asset(&abs_url, folder).await {
                                        self.asset_cache.insert(abs_url, local_path);
                                    }
                                }
                            }
                        }
                    }
                    "iframe" | "video" | "embed" => {
                        if let Some(src) = el_ref.value().attr("src") {
                            let video_url = decode_html_entities(src).to_string();
                            let abs_url = self.resolve_url(&video_url);
                            if self.is_internal(&abs_url) {
                                if let Some(local_path) = self.download_asset(&abs_url, "videos").await {
                                    self.asset_cache.insert(abs_url, local_path);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn process_element(&self, element: &ElementRef<'_>, markdown: &mut String) {
        for child in element.children() {
            match child.value() {
                Node::Element(e) => {
                    let tag = e.name();
                    if let Some(child_ref) = ElementRef::wrap(child) {
                        match tag {
                            // Block elements that create double newlines
                            "br" => {
                                markdown.push_str("\n\n");
                            }
                            "p" | "div" => {
                                markdown.push_str("\n\n");
                                self.process_element(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            // Headers
                            "h1" => {
                                markdown.push_str("\n\n# ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            "h2" => {
                                markdown.push_str("\n\n## ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            "h3" => {
                                markdown.push_str("\n\n### ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            "h4" => {
                                markdown.push_str("\n\n#### ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            "h5" => {
                                markdown.push_str("\n\n##### ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            "h6" => {
                                markdown.push_str("\n\n###### ");
                                self.process_text_content(&child_ref, markdown);
                                markdown.push_str("\n\n");
                            }
                            // Strong/Bold
                            "strong" | "b" => {
                                markdown.push_str("**");
                                self.process_element(&child_ref, markdown);
                                markdown.push_str("**");
                            }
                            // Emphasis/Italic
                            "em" | "i" => {
                                markdown.push('*');
                                self.process_element(&child_ref, markdown);
                                markdown.push('*');
                            }
                            // Images
                            "img" => {
                                let src = e.attr("src").unwrap_or("");
                                let alt = e.attr("alt").unwrap_or("");
                                let img_url = decode_html_entities(src).to_string();
                                let abs_url = self.resolve_url(&img_url);
                                
                                let final_url = self.asset_cache.get(&abs_url)
                                    .map(|path| format!("/{}", path))
                                    .unwrap_or(abs_url);
                                
                                markdown.push_str(&format!("![{}]({})", alt, final_url));
                            }
                            // Links
                            "a" => {
                                let href = e.attr("href").unwrap_or("");
                                let link_url = decode_html_entities(href).to_string();
                                let abs_url = self.resolve_url(&link_url);
                                let link_text = self.extract_text_content(&child_ref);
                                
                                // Check if it's a media file
                                if self.is_media_file(&abs_url) {
                                    let final_url = self.asset_cache.get(&abs_url)
                                        .map(|path| format!("/{}", path))
                                        .unwrap_or(abs_url);
                                    
                                    if self.is_audio_file(&link_url) {
                                        markdown.push_str(&format!("🔊 [Audio]({})", final_url));
                                    } else if self.is_video_file(&link_url) {
                                        markdown.push_str(&format!("▶️ [Video]({})", final_url));
                                    } else {
                                        markdown.push_str(&format!("[{}]({})", link_text, final_url));
                                    }
                                } else {
                                    let display_text = if link_text.is_empty() { &abs_url } else { &link_text };
                                    markdown.push_str(&format!("[{}]({})", display_text, abs_url));
                                }
                            }
                            // Video elements
                            "iframe" | "video" | "embed" => {
                                let src = e.attr("src").unwrap_or("");
                                let video_url = decode_html_entities(src).to_string();
                                let abs_url = self.resolve_url(&video_url);
                                
                                let final_url = self.asset_cache.get(&abs_url)
                                    .map(|path| format!("/{}", path))
                                    .unwrap_or(abs_url);
                                
                                markdown.push_str(&format!("▶️ [Video]({})", final_url));
                            }
                            // Skip these elements
                            "script" | "style" | "noscript" | "head" | "meta" | "link" => {
                                // Skip entirely
                            }
                            // Process other elements recursively
                            _ => {
                                self.process_element(&child_ref, markdown);
                            }
                        }
                    }
                }
                Node::Text(text) => {
                    let text_content = text.to_string();
                    let decoded = decode_html_entities(&text_content);
                    let normalized = self.normalize_whitespace(&decoded);
                    if !normalized.is_empty() {
                        markdown.push_str(&normalized);
                    }
                }
                _ => {}
            }
        }
    }

    fn process_text_content(&self, element: &ElementRef<'_>, markdown: &mut String) {
        let text = self.extract_text_content(element);
        let normalized = self.normalize_whitespace(&text);
        markdown.push_str(&normalized);
    }

    fn extract_text_content(&self, element: &ElementRef<'_>) -> String {
        element.text().collect::<String>()
    }

    fn normalize_whitespace(&self, text: &str) -> String {
        let re = Regex::new(r"\s+").unwrap();
        re.replace_all(text.trim(), " ").to_string()
    }

    fn resolve_url(&self, relative_url: &str) -> String {
        if let Ok(base) = Url::parse(&self.base_url) {
            base.join(relative_url)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| relative_url.to_string())
        } else {
            relative_url.to_string()
        }
    }

    fn is_internal(&self, url: &str) -> bool {
        url.starts_with('/') || url.contains("jesus-is-savior.com")
    }

    fn is_media_file(&self, url: &str) -> bool {
        self.is_audio_file(url) || self.is_video_file(url)
    }

    fn is_audio_file(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.ends_with(".mp3") || 
        url_lower.ends_with(".wav") || 
        url_lower.ends_with(".ogg") || 
        url_lower.ends_with(".m4a") ||
        url_lower.ends_with(".flac") ||
        url_lower.ends_with(".aac")
    }

    fn is_video_file(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.ends_with(".mp4") || 
        url_lower.ends_with(".avi") || 
        url_lower.ends_with(".mov") || 
        url_lower.ends_with(".wmv") ||
        url_lower.ends_with(".webm") ||
        url_lower.ends_with(".mkv") ||
        url_lower.ends_with(".flv")
    }

    async fn download_asset(&self, url: &str, folder: &str) -> Option<String> {
        // Generate filename from URL hash
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        
        let ext = Path::new(url)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let filename = format!("{}.{}", hash, ext);
        let save_path = format!("ui/public/{}/{}", folder, filename);
        
        // Check if file already exists
        if Path::new(&save_path).exists() {
            return Some(format!("{}/{}", folder, filename));
        }
        
        // Download the asset
        match self.client.get(url).send().await {
            Ok(response) if response.status().is_success() => {
                if let Ok(bytes) = response.bytes().await {
                    if fs::write(&save_path, &bytes).await.is_ok() {
                        return Some(format!("{}/{}", folder, filename));
                    }
                }
            }
            _ => {}
        }
        
        None
    }

    fn clean_markdown(&self, markdown: &str) -> String {
        // Split into lines and clean up
        let lines: Vec<&str> = markdown.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut consecutive_empty = 0;
        
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                consecutive_empty += 1;
                // Only allow maximum of 2 consecutive empty lines (which becomes one blank line)
                if consecutive_empty <= 2 {
                    cleaned_lines.push("".to_string());
                }
            } else {
                consecutive_empty = 0;
                cleaned_lines.push(trimmed.to_string());
            }
        }
        
        // Remove leading empty lines
        while cleaned_lines.first() == Some(&String::new()) {
            cleaned_lines.remove(0);
        }
        
        // Remove trailing empty lines
        while cleaned_lines.last() == Some(&String::new()) {
            cleaned_lines.pop();
        }
        
        // Join with newlines and trim the final result
        cleaned_lines.join("\n").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[tokio::test]
    async fn test_basic_conversion() {
        let html = r#"
        <html>
            <body>
                <h1>Test Title</h1>
                <p>This is a <strong>bold</strong> and <em>italic</em> text.</p>
                <br>
                <div>Some content in a div</div>
            </body>
        </html>
        "#;
        
        let document = Html::parse_document(html);
        let result = html_to_markdown(&document, "https://example.com").await;
        
        assert!(result.contains("# Test Title"));
        assert!(result.contains("**bold**"));
        assert!(result.contains("*italic*"));
    }

    #[tokio::test]
    async fn test_link_conversion() {
        let html = r#"
        <html>
            <body>
                <a href="https://example.com">Link text</a>
                <a href="audio.mp3">Audio link</a>
                <a href="video.mp4">Video link</a>
            </body>
        </html>
        "#;
        
        let document = Html::parse_document(html);
        let result = html_to_markdown(&document, "https://example.com").await;
        
        assert!(result.contains("[Link text](https://example.com/)") || result.contains("https://example.com"));
        assert!(result.contains("🔊 [Audio]") || result.contains("audio.mp3"));
        assert!(result.contains("▶️ [Video]") || result.contains("video.mp4"));
    }

    #[tokio::test]
    async fn test_image_conversion() {
        let html = r#"
        <html>
            <body>
                <img src="image.jpg" alt="Test image">
            </body>
        </html>
        "#;
        
        let document = Html::parse_document(html);
        let result = html_to_markdown(&document, "https://example.com").await;
        
        assert!(result.contains("![Test image]"));
    }
}
