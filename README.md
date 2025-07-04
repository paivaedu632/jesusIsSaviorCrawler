# Jesus Is Savior Web Scraper

A high-performance, optimized web scraper built in Rust that converts HTML content to **Markdown format** with a **simplified JSON schema**. This scraper is specifically designed to extract posts from websites, outputting clean, readable content that's much easier to work with than complex nested structures.

## Key Features

### 🎯 Simplified Output Schema
- **Direct Markdown content** instead of complex nested HTML/JSON structures
- Clean, readable format that's easy to process and display
- Streamlined JSON with only essential fields: `avatar`, `username`, `url`, `title`, `content`, `tags`
- Field order preserved using `serde_json::to_string_pretty`

### 📝 Advanced Markdown Conversion
- HTML elements converted to proper Markdown syntax
- Headers (`<h1-h6>`) → `# ## ### ####` etc.
- Bold/Strong (`<strong>`, `<b>`) → `**text**`
- Italic/Emphasis (`<em>`, `<i>`) → `*text*`
- Links (`<a>`) → `[text](url)`
- Images (`<img>`) → `![alt](url)`
- Media files → `🔊 [Audio](path)` / `▶️ [Video](path)`
- Paragraph breaks and proper formatting

### 🚀 Performance Optimizations
- **Concurrent processing** with configurable request limits
- **Connection pooling** and HTTP/2 support
- **Smart caching system** to avoid reprocessing URLs
- **Resumable operations** with progress tracking
- **Rate limiting** and adaptive retry logic
- **Memory-efficient streaming** for large datasets

### 💾 Asset Management
- **Automatic downloading** of images, videos, and audio files
- **Local asset storage** in organized `assets/` directory structure
- **SHA256-based filename generation** to avoid conflicts
- **Asset URL mapping** in markdown content
- **Resume capability** for interrupted downloads

### 🛠️ Additional Features
- **Progress tracking** with detailed statistics
- **Error handling** and retry mechanisms
- **Configurable parameters** (concurrency, rate limits, timeouts)
- **Comprehensive logging** and status updates
- **Extensive test coverage** for markdown conversion

## Installation

1. Ensure you have Rust installed (1.70+)
2. Clone this repository
3. Build the project:

```bash
cd jesusIsSaviorCrawler
cargo build --release
```

## Usage

### Basic Usage

```bash
# Run with default settings
cargo run --release --bin scrape_posts

# Specify custom input and output files
cargo run --release --bin scrape_posts --urls my_urls.txt --output results.json

# Adjust performance settings
cargo run --release --bin scrape_posts --concurrent 50 --rate-limit 200
```

### Command Line Options

- `--urls, -u FILE`: URLs file (default: urls.txt)
- `--output, -o FILE`: Output JSON with Markdown content (default: posts.json)
- `--concurrent, -c NUM`: Max concurrent requests (default: 100)
- `--rate-limit, -r MS`: Rate limit delay in ms (default: 100)
- `--retries NUM`: Number of retry attempts (default: 3)
- `--clear-cache`: Clear cache before starting
- `--verbose, -v`: Verbose output
- `--help, -h`: Show help

### Input Format

Create a `urls.txt` file with one URL per line:

```
https://example.com/page1
https://example.com/page2
https://example.com/page3
```

### Output Format

The scraper outputs a JSON array with this simplified schema:

```json
[
  {
    "avatar": "https://example.com/avatar.jpg",
    "username": "Author Name",
    "url": "https://example.com/post",
    "title": "Post Title",
    "content": "# Main Heading\n\nThis is **bold** and *italic* text.\n\n![Image](assets/images/hash.jpg)",
    "tags": ["faith", "salvation", "scripture"]
  }
]
```

## Project Structure

```
jesusIsSaviorCrawler/
├── src/
│   ├── bin/
│   │   ├── scrape_posts.rs          # Main scraper binary
│   │   └── test_markdown_flattener.rs # Test binary for markdown conversion
│   ├── lib.rs                        # Library entry point with Post struct
│   └── markdown_flattener.rs         # HTML-to-Markdown conversion logic
├── assets/                           # Downloaded assets (images, videos, audio)
│   ├── images/
│   ├── videos/
│   └── audio/
├── target/                           # Rust build artifacts
├── Cargo.toml                        # Project dependencies and metadata
├── Cargo.lock                        # Dependency lock file
├── .gitignore                        # Git ignore rules
├── urls.txt                          # Input URLs (create this file)
├── posts.json                        # Output file (generated)
├── proxies.txt                       # Optional proxy list
├── scraper_cache.json               # Cache file (auto-generated)
├── scraper_progress.json            # Progress file (auto-generated)
└── README.md                         # This file
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Test markdown conversion specifically
cargo run --bin test_markdown_flattener
```

### Building for Production

```bash
# Optimized release build
cargo build --release

# The binary will be in target/release/
```

### Using as a Library

You can use this crate as a library in other Rust projects:

```toml
[dependencies]
jesus_is_savior_crawler = { path = "../jesusIsSaviorCrawler" }
```

```rust
use jesus_is_savior_crawler::{html_to_markdown, Post};
use scraper::Html;

#[tokio::main]
async fn main() {
    let html = "<p>Hello <strong>world</strong>!</p>";
    let document = Html::parse_document(html);
    let markdown = html_to_markdown(&document, "https://example.com").await;
    println!("{}", markdown); // "Hello **world**!"
}
```

## Configuration

The scraper uses several configuration files:

- `scraper_cache.json`: Tracks processed URLs to avoid duplicates
- `scraper_progress.json`: Stores progress information for resumable operations
- `proxies.txt`: Optional proxy list for requests
- `.env`: Environment variables

## Performance Tips

1. **Adjust concurrency**: Start with lower values (20-50) and increase based on your system and target server capacity
2. **Use rate limiting**: Respect target servers with appropriate delays (100-500ms)
3. **Enable caching**: Let the scraper skip already processed URLs
4. **Monitor resources**: Watch CPU and memory usage during large scraping operations

## License

MIT OR Apache-2.0
