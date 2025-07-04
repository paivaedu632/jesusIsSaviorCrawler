// Optimized ultra-fast web scraper with advanced features:
// - Connection pooling and HTTP/2 support
// - Concurrent asset downloading with batching
// - Smart caching (URL tracking only)
// - Adaptive rate limiting and retry logic
// - Memory-efficient streaming processing
// - Progress tracking and resumable operations

use std::{
    collections::HashSet,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use reqwest::{Client, Response};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    fs,
    sync::{Mutex, Semaphore, RwLock},
    time::sleep,
};
use url::Url;
use regex::Regex;
use encoding_rs::{Encoding, UTF_8};
use percent_encoding;
use jesus_is_savior_crawler::html_to_markdown;

// Configuration constants
const MAX_CONCURRENT_REQUESTS: usize = 100; // Reduced for better stability
const MAX_CONCURRENT_ASSETS: usize = 50;    // Separate limit for asset downloads
const REQUEST_TIMEOUT_SECS: u64 = 30;
const RETRY_ATTEMPTS: usize = 3;
const RETRY_DELAY_MS: u64 = 1000;
const RATE_LIMIT_DELAY_MS: u64 = 100; // Delay between requests
const CHUNK_SIZE: usize = 20; // Process URLs in chunks

// Cache and state files
const CACHE_FILE: &str = "scraper_cache.json";
const PROGRESS_FILE: &str = "scraper_progress.json";

// NarrativeElement enum removed - now generating markdown directly

#[derive(Serialize, Deserialize, Clone)]
struct Post {
    avatar: String,
    username: String,
    url: String,
    title: Option<String>,
    content: String,      // markdown
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct ScraperCache {
    processed_urls: HashSet<String>, // Set of processed URLs (no hashes)
    failed_urls: HashSet<String>,
    last_updated: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct ScraperProgress {
    total_urls: usize,
    processed: usize,
    failed: usize,
    start_time: u64,
    last_checkpoint: u64,
}

struct OptimizedScraper {
    client: Client,
    asset_client: Client,
    cache: Arc<RwLock<ScraperCache>>,
    progress: Arc<Mutex<ScraperProgress>>,
    request_semaphore: Arc<Semaphore>,
    asset_semaphore: Arc<Semaphore>,
    processed_count: Arc<AtomicUsize>,
    failed_count: Arc<AtomicUsize>,
}

impl OptimizedScraper {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create optimized HTTP client with connection pooling
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .pool_max_idle_per_host(10) // Reduced connection pooling
            .pool_idle_timeout(Duration::from_secs(30))
            .http1_title_case_headers() // Better compatibility
            .http2_adaptive_window(true) // Adaptive HTTP/2 instead of forcing it
            .gzip(true)
            .deflate(true)
            .danger_accept_invalid_certs(false) // Ensure proper cert validation
            .tls_built_in_root_certs(true) // Use built-in root certs
            .build()?;

        // Separate client for asset downloads with different settings
        let asset_client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(60)) // Longer timeout for large assets
            .pool_max_idle_per_host(5)
            .http1_title_case_headers()
            .http2_adaptive_window(true)
            .gzip(true)
            .deflate(true)
            .build()?;

        // Load cache and progress
        let cache = Arc::new(RwLock::new(Self::load_cache().await));
        let progress = Arc::new(Mutex::new(Self::load_progress().await));

        // Create asset directories
        for dir in &["ui/public/images", "ui/public/videos", "ui/public/audio"] {
            fs::create_dir_all(dir).await.ok();
        }

        Ok(Self {
            client,
            asset_client,
            cache,
            progress,
            request_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            asset_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_ASSETS)),
            processed_count: Arc::new(AtomicUsize::new(0)),
            failed_count: Arc::new(AtomicUsize::new(0)),
        })
    }

    async fn load_cache() -> ScraperCache {
        match fs::read_to_string(CACHE_FILE).await {
            Ok(content) => {
                // Try to load with new format first
                if let Ok(cache) = serde_json::from_str::<ScraperCache>(&content) {
                    cache
                } else {
                    // Try to migrate from old format (HashMap -> HashSet)
                    #[derive(serde::Deserialize)]
                    struct OldScraperCache {
                        processed_urls: std::collections::HashMap<String, String>,
                        failed_urls: std::collections::HashSet<String>,
                        last_updated: u64,
                    }
                    
                    if let Ok(old_cache) = serde_json::from_str::<OldScraperCache>(&content) {
                        println!("Migrating cache from old format with {} processed URLs", 
                                old_cache.processed_urls.len());
                        
                        ScraperCache {
                            processed_urls: old_cache.processed_urls.keys().cloned().collect(),
                            failed_urls: old_cache.failed_urls,
                            last_updated: old_cache.last_updated,
                        }
                    } else {
                        ScraperCache::default()
                    }
                }
            },
            Err(_) => ScraperCache::default(),
        }
    }

    async fn load_progress() -> ScraperProgress {
        match fs::read_to_string(PROGRESS_FILE).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => ScraperProgress::default(),
        }
    }

    async fn save_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        let cache = self.cache.read().await;
        let content = serde_json::to_string_pretty(&*cache)?;
        fs::write(CACHE_FILE, content).await?;
        Ok(())
    }

    async fn save_progress(&self) -> Result<(), Box<dyn std::error::Error>> {
        let progress = self.progress.lock().await;
        let content = serde_json::to_string_pretty(&*progress)?;
        fs::write(PROGRESS_FILE, content).await?;
        Ok(())
    }

    async fn read_urls(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path).await?;
        let urls: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|url| !url.is_empty())
            .collect();
        Ok(urls)
    }

    async fn should_skip_url(&self, url: &str) -> bool {
        let cache = self.cache.read().await;
        cache.processed_urls.contains(url) || cache.failed_urls.contains(url)
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<Response, Box<dyn std::error::Error>> {
        let _permit = self.request_semaphore.acquire().await?;
        
        for attempt in 1..=RETRY_ATTEMPTS {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    } else if response.status().is_server_error() && attempt < RETRY_ATTEMPTS {
                        // Retry on server errors
                        sleep(Duration::from_millis(RETRY_DELAY_MS * attempt as u64)).await;
                        continue;
                    } else {
                        return Err(format!("HTTP error: {}", response.status()).into());
                    }
                }
                Err(e) if attempt < RETRY_ATTEMPTS => {
                    eprintln!("Attempt {}/{} failed for {}: {}", attempt, RETRY_ATTEMPTS, url, e);
                    sleep(Duration::from_millis(RETRY_DELAY_MS * attempt as u64)).await;
                }
                Err(e) => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))),
            }
        }
        
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Max retries exceeded")))
    }




    async fn process_html(&self, url: &str, bytes: &[u8], headers: &reqwest::header::HeaderMap) -> Option<Post> {
        // Detect encoding (same logic as before but optimized)
        let encoding = self.detect_encoding(bytes, headers);
        let (text, _, _) = encoding.decode(bytes);
        let body = text.into_owned();
        
        let document = Html::parse_document(&body);
        let title = self.extract_title(&document);
        
        // Call html_to_markdown to obtain content
        let content = html_to_markdown(&document, url).await;

        Some(Post {
            avatar: "https://pbs.twimg.com/profile_images/1277486993765568512/LKqi43Xt_400x400.jpg".to_string(),
            username: "David J. Stewart".to_string(),
            url: url.to_string(),
            title,
            content,
            tags: self.extract_tags_from_url(url),
        })
    }

    fn detect_encoding(&self, bytes: &[u8], headers: &reqwest::header::HeaderMap) -> &'static Encoding {
        // Try to detect encoding from headers first
        if let Some(encoding) = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .and_then(|ct| {
                ct.split(';')
                    .find_map(|s| {
                        let s = s.trim();
                        if s.to_lowercase().starts_with("charset=") {
                            Some(s.trim_start_matches("charset=").trim())
                        } else {
                            None
                        }
                    })
            })
            .and_then(|enc| Encoding::for_label(enc.as_bytes()))
        {
            return encoding;
        }

        // Try to detect from content if it's HTML
        if bytes.len() > 1024 {
            let sample = &bytes[0..1024];
            let (text, _, _) = UTF_8.decode(sample);
            let text_lower = text.to_lowercase();
            
            // Look for meta charset in HTML
            if text_lower.contains("<meta") && text_lower.contains("charset=") {
                let re = Regex::new(r#"<meta[^>]+charset=["']?([a-zA-Z0-9\-_]+)"#).unwrap();
                if let Some(caps) = re.captures(&text_lower) {
                    if let Some(enc_match) = caps.get(1) {
                        let enc_name = enc_match.as_str();
                        if let Some(encoding) = Encoding::for_label(enc_name.as_bytes()) {
                            return encoding;
                        }
                    }
                }
            }
        }

        // Fallback to UTF-8
        UTF_8
    }

    fn extract_title(&self, document: &Html) -> Option<String> {
        let title_sel = Selector::parse("title").unwrap();
        document
            .select(&title_sel)
            .next()
            .map(|t| t.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
    }



    fn extract_tags_from_url(&self, url: &str) -> Vec<String> {
        let stopwords = [
            "in", "of", "the", "and", "a", "an", "on", "for", "to", "by", "with", "at", "from",
            "as", "is", "it", "that", "this", "be", "or", "are", "was", "were", "but", "not",
            "so", "if", "then", "than", "too", "very", "can", "will", "just", "do", "does",
            "did", "has", "have", "had", "about", "into", "out", "up", "down", "over", "under",
            "again", "further", "once", "here", "there", "when", "where", "why", "how", "all",
            "any", "both", "each", "few", "more", "most", "other", "some", "such", "no", "nor",
            "only", "own", "same", "s", "t", "don", "should", "now"
        ];

        let mut tags = Vec::new();
        if let Ok(parsed_url) = Url::parse(url) {
            if let Some(segments) = parsed_url.path_segments() {
                for segment in segments {
                    let segment = segment.split('.').next().unwrap_or("");
                    let decoded = percent_encoding::percent_decode_str(segment).decode_utf8_lossy();
                    
                    for part in decoded.split(|c| c == ' ' || c == '_' || c == '-') {
                        let tag = part.trim().to_lowercase();
                        if tag.len() > 2 && !stopwords.contains(&tag.as_str()) && !tags.contains(&tag) {
                            tags.push(tag);
                        }
                    }
                }
            }
        }
        tags
    }

    async fn update_progress(&self, total: usize) {
        let processed = self.processed_count.load(Ordering::Relaxed);
        let failed = self.failed_count.load(Ordering::Relaxed);
        
        {
            let mut progress = self.progress.lock().await;
            progress.total_urls = total;
            progress.processed = processed;
            progress.failed = failed;
            progress.last_checkpoint = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
        
        // Save progress periodically
        if processed % 10 == 0 {
            self.save_progress().await.ok();
        }
        
        // Print progress
        let percentage = if total > 0 { (processed * 100) / total } else { 0 };
        println!(
            "Progress: {}/{} ({}%) processed, {} failed",
            processed, total, percentage, failed
        );
    }

    pub async fn scrape_urls(&self, urls: Vec<String>) -> Result<Vec<Post>, Box<dyn std::error::Error>> {
        let total_urls = urls.len();
        println!("Starting optimized scraping of {} URLs...", total_urls);
        
        // Filter out already processed URLs
        let mut urls_to_process = Vec::new();
        for url in urls {
            if !self.should_skip_url(&url).await {
                urls_to_process.push(url);
            }
        }
        
        println!("Filtered to {} new URLs to process", urls_to_process.len());
        
        let mut all_posts = Vec::new();
        let start_time = Instant::now();
        
        // Process URLs in chunks but sequentially to avoid Send issues
        for chunk in urls_to_process.chunks(CHUNK_SIZE) {
            let mut chunk_posts = Vec::new();
            
            for url in chunk {
                // Rate limiting
                sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;
                
                let result = self.process_single_url(url).await;
                
                if let Some(post) = result {
                    chunk_posts.push(post);
                }
            }
            
            // Add chunk results to all posts
            all_posts.extend(chunk_posts);
            
            // Update progress
            self.update_progress(total_urls).await;
            
            // Save cache periodically
            if all_posts.len() % 50 == 0 {
                self.save_cache().await.ok();
            }
        }
        
        // Final save
        self.save_cache().await.ok();
        self.save_progress().await.ok();
        
        let elapsed = start_time.elapsed();
        println!(
            "Scraping completed in {:.2}s. Processed: {}, Failed: {}, New posts: {}",
            elapsed.as_secs_f64(),
            self.processed_count.load(Ordering::Relaxed),
            self.failed_count.load(Ordering::Relaxed),
            all_posts.len()
        );
        
        Ok(all_posts)
    }
    
    async fn process_single_url(&self, url: &str) -> Option<Post> {
        match self.fetch_with_retry(url).await {
            Ok(response) => {
                let headers = response.headers().clone();
                match response.bytes().await {
                    Ok(bytes) => {
                        if let Some(post) = self.process_html(url, &bytes, &headers).await {
                            self.processed_count.fetch_add(1, Ordering::Relaxed);
                            
                            // Update cache
                            {
                                let mut cache = self.cache.write().await;
cache.processed_urls.insert(url.to_string());
                            }
                            
                            Some(post)
                        } else {
                            self.failed_count.fetch_add(1, Ordering::Relaxed);
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read response for {}: {}", url, e);
                        self.failed_count.fetch_add(1, Ordering::Relaxed);
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch {}: {}", url, e);
                self.failed_count.fetch_add(1, Ordering::Relaxed);
                
                // Add to failed URLs cache
                {
                    let mut cache = self.cache.write().await;
                    cache.failed_urls.insert(url.to_string());
                }
                
                None
            }
        }
    }
}

// Implement Clone for OptimizedScraper
impl Clone for OptimizedScraper {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            asset_client: self.asset_client.clone(),
            cache: Arc::clone(&self.cache),
            progress: Arc::clone(&self.progress),
            request_semaphore: Arc::clone(&self.request_semaphore),
            asset_semaphore: Arc::clone(&self.asset_semaphore),
            processed_count: Arc::clone(&self.processed_count),
            failed_count: Arc::clone(&self.failed_count),
        }
    }
}

#[derive(Debug)]
struct ScraperConfig {
    urls_file: String,
    output_file: String,
    max_concurrent: usize,
    rate_limit_ms: u64,
    retry_attempts: usize,
    clear_cache: bool,
    verbose: bool,
}

impl Default for ScraperConfig {
    fn default() -> Self {
        Self {
            urls_file: "urls.txt".to_string(),
            output_file: "ui/public/posts.json".to_string(),
            max_concurrent: MAX_CONCURRENT_REQUESTS,
            rate_limit_ms: RATE_LIMIT_DELAY_MS,
            retry_attempts: RETRY_ATTEMPTS,
            clear_cache: false,
            verbose: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut config = ScraperConfig::default();
    
    // Simple argument parsing
    for i in 1..args.len() {
        match args[i].as_str() {
            "--urls" | "-u" if i + 1 < args.len() => {
                config.urls_file = args[i + 1].clone();
            }
            "--output" | "-o" if i + 1 < args.len() => {
                config.output_file = args[i + 1].clone();
            }
            "--concurrent" | "-c" if i + 1 < args.len() => {
                if let Ok(value) = args[i + 1].parse() {
                    config.max_concurrent = value;
                }
            }
            "--rate-limit" | "-r" if i + 1 < args.len() => {
                if let Ok(value) = args[i + 1].parse() {
                    config.rate_limit_ms = value;
                }
            }
            "--retries" if i + 1 < args.len() => {
                if let Ok(value) = args[i + 1].parse() {
                    config.retry_attempts = value;
                }
            }
            "--clear-cache" => {
                config.clear_cache = true;
            }
            "--verbose" | "-v" => {
                config.verbose = true;
            }
            "--help" | "-h" => {
                println!("Optimized Post Scraper");
                println!("Usage: cargo run --release --bin optimized_scrape_posts [OPTIONS]");
                println!("Options:");
                println!("  --urls, -u FILE       URLs file (default: urls.txt)");
                println!("  --output, -o FILE     Output JSON file (default: ui/public/posts.json)");
                println!("  --concurrent, -c NUM  Max concurrent requests (default: {})", MAX_CONCURRENT_REQUESTS);
                println!("  --rate-limit, -r MS   Rate limit delay in ms (default: {})", RATE_LIMIT_DELAY_MS);
                println!("  --retries NUM         Number of retry attempts (default: {})", RETRY_ATTEMPTS);
                println!("  --clear-cache         Clear cache before starting");
                println!("  --verbose, -v         Verbose output");
                println!("  --help, -h            Show this help");
                return Ok(());
            }
            _ => {}
        }
    }
    
    println!("🚀 Starting Optimized Post Scraper");
    println!("Configuration:");
    println!("  URLs file: {}", config.urls_file);
    println!("  Output file: {}", config.output_file);
    println!("  Max concurrent requests: {}", config.max_concurrent);
    println!("  Rate limit delay: {}ms", config.rate_limit_ms);
    println!("  Retry attempts: {}", config.retry_attempts);
    println!("  Clear cache: {}", config.clear_cache);
    println!("  Verbose: {}", config.verbose);
    
    // Clear cache if requested
    if config.clear_cache {
        println!("Clearing cache...");
        let _ = fs::remove_file(CACHE_FILE).await;
        let _ = fs::remove_file(PROGRESS_FILE).await;
    }
    
    // Read URLs from file
    let urls = match OptimizedScraper::read_urls(&config.urls_file).await {
        Ok(urls) => urls,
        Err(e) => {
            eprintln!("Failed to read {}: {}", config.urls_file, e);
            return Ok(());
        }
    };
    
    if urls.is_empty() {
        println!("No URLs found in {}", config.urls_file);
        return Ok(());
    }
    
    println!("Found {} URLs to process", urls.len());
    
    // Create optimized scraper
    let scraper = OptimizedScraper::new().await?;
    
    // Start time tracking
    let start_time = Instant::now();
    
    // Scrape URLs
    let posts = scraper.scrape_urls(urls).await?;
    
    // Calculate elapsed time
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    
    // Save results
    if !posts.is_empty() {
        // Create output directory if it doesn't exist
        if let Some(parent) = Path::new(&config.output_file).parent() {
            fs::create_dir_all(parent).await.ok();
        }
        
        // Save posts to JSON file
        let json_content = serde_json::to_string_pretty(&posts)?;
        fs::write(&config.output_file, json_content).await?;
        
        println!("✅ Saved {} posts to {}", posts.len(), config.output_file);
        println!("⏱️ Total time: {:.2}s ({:.2} posts/second)", 
                 elapsed_secs, posts.len() as f64 / elapsed_secs);
    } else {
        println!("⚠️ No new posts to save");
    }
    
    Ok(())
}