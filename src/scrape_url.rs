use anyhow::{anyhow, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use url::Url;
use rand::{distributions::Alphanumeric, Rng};
use dotenv::dotenv;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::Write;

// Configuration constants
const CONCURRENT_REQUESTS: usize = 1000; // Increased for maximum concurrency (use with caution)
const REQUEST_TIMEOUT_SECS: u64 = 15;

// File names for state persistence
const STATE_FILE: &str = "crawl_state.json";
const PENDING_FILE: &str = "pending_urls.json";

#[derive(Serialize, Deserialize, Debug)]
struct CrawlState {
    discovered: Vec<String>,
    processed: usize,
    pending: Vec<String>,
    start_time: u64,
}

#[derive(Clone)]
struct ProxyConfig {
    user: String,
    pass: String,
    host: String,
    port: String,
}

impl ProxyConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            user: std::env::var("BRD_PROXY_USER")
                .map_err(|_| anyhow!("BRD_PROXY_USER environment variable not set"))?,
            pass: std::env::var("BRD_PROXY_PASS")
                .map_err(|_| anyhow!("BRD_PROXY_PASS environment variable not set"))?,
            host: std::env::var("BRD_PROXY_HOST").unwrap_or_else(|_| "brd.superproxy.io".to_string()),
            port: std::env::var("BRD_PROXY_PORT").unwrap_or_else(|_| "33335".to_string()),
        })
    }

    fn create_client(&self) -> Result<Client> {
        let session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();

        let user_with_session = if self.user.contains("-session-") {
            self.user.clone()
        } else {
            format!("{}-session-{}", self.user, session)
        };

        let proxy_url = format!("http://{}:{}@{}:{}", user_with_session, self.pass, self.host, self.port);
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| anyhow!("Invalid proxy URL: {}", e))?;

        Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .pool_max_idle_per_host(0) // Disable connection pooling for better IP rotation
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))
    }
}

struct Crawler {
    base_domain: String,
    proxy_config: ProxyConfig,
    semaphore: Arc<Semaphore>,
    state: Arc<Mutex<InternalState>>,
}

struct InternalState {
    discovered: HashSet<String>,
    processed: usize,
    start_time: Instant,
    last_progress_update: Instant,
}

impl Crawler {
    fn new(base_url: &str, max_concurrent: usize) -> Result<Self> {
        let parsed_url = Url::parse(base_url)
            .map_err(|_| anyhow!("Invalid base URL: {}", base_url))?;
        let base_domain = parsed_url.host_str()
            .ok_or_else(|| anyhow!("Cannot extract domain from URL: {}", base_url))?
            .to_string();

        let proxy_config = ProxyConfig::from_env()?;
        let mut initial_state = InternalState {
            discovered: HashSet::new(),
            processed: 0,
            start_time: Instant::now(),
            last_progress_update: Instant::now(),
        };
        // Try to load previous state
        if let Ok(saved_state) = Self::load_state() {
            initial_state.discovered = saved_state.discovered.into_iter().collect();
            initial_state.processed = saved_state.processed;
            println!("Resuming crawl: {} discovered, {} processed", 
                     initial_state.discovered.len(), initial_state.processed);
        } else {
            // Initialize with base URL
            let base_url_str = format!("https://{}", base_domain);
            initial_state.discovered.insert(base_url_str);
        }

        Ok(Self {
            base_domain,
            proxy_config,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            state: Arc::new(Mutex::new(initial_state)),
        })
    }

    fn load_state() -> Result<CrawlState> {
        let state_bytes = fs::read(STATE_FILE)?;
        let state = serde_json::from_slice(&state_bytes)?;
        Ok(state)
    }

    fn load_pending_urls() -> Vec<String> {
        fs::read(PENDING_FILE)
            .and_then(|bytes| Ok(serde_json::from_slice(&bytes)?))
            .unwrap_or_else(|_| Vec::new())
    }

    async fn crawl(&self) -> Result<HashSet<String>> {
        // Always-seed URLs
        let always_seed = vec![
            "https://www.jesus-is-savior.com/",
            "https://www.jesus-is-savior.com/sitemap.xml",
            "https://www.jesus-is-savior.com/rss.xml",
            "https://www.jesus-is-savior.com/recent_articles.htm",
            "https://www.jesus-is-savior.com/Basics/basics_of_christianity.htm",
        ];
        let mut discovered_urls: HashSet<String> = HashSet::new();
        let mut urls_to_visit = Self::load_pending_urls();
        // Add always-seed URLs to discovered and to-visit
        for url in always_seed {
            if discovered_urls.insert(url.to_string()) {
                urls_to_visit.push(url.to_string());
            }
        }
        // If no pending URLs, start with base URL
        if urls_to_visit.is_empty() {
            let base_url_str = format!("https://{}", self.base_domain);
            if discovered_urls.insert(base_url_str.clone()) {
                urls_to_visit.push(base_url_str);
            }
        }
        let mut batch_number = 0;
        while !urls_to_visit.is_empty() {
            let batch: Vec<String> = urls_to_visit.drain(..std::cmp::min(50, urls_to_visit.len())).collect();
            let results = self.process_batch(batch.clone()).await;
            let mut new_urls = Vec::new();
            for (url, result) in results {
                match result {
                    Ok(found_urls) => {
                        for found in found_urls {
                            if discovered_urls.insert(found.clone()) {
                                new_urls.push(found);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error processing URL: {} | {}", url, e);
                    }
                }
            }
            let new_count = new_urls.len();
            // Add new URLs to visit
            for url in new_urls {
                urls_to_visit.push(url);
            }
            batch_number += 1;
            println!("--- Batch {} completed: {} new URLs discovered", batch_number, new_count);
            println!("Progress: {} discovered, {} processed, {} pending | Elapsed: ...", discovered_urls.len(), batch_number * 50, urls_to_visit.len());
        }
        Ok(discovered_urls)
    }

    async fn process_batch(&self, urls: Vec<String>) -> Vec<(String, Result<Vec<String>>)> {
        let processing_futures = urls.into_iter().map(|url| {
            let crawler = self.clone();
            let url_clone = url.clone();
            async move {
                let permit_result = crawler.semaphore.acquire().await;
                if let Err(e) = permit_result {
                    return (url_clone, Err(anyhow!("Semaphore error: {}", e)));
                }
                let result = crawler.fetch_and_parse(&url_clone).await;
                (url_clone, result)
            }
        });
        futures::future::join_all(processing_futures).await
    }

    async fn fetch_and_parse(&self, url: &str) -> Result<Vec<String>> {
        // Create a new client for each request to ensure IP rotation
        let client = self.proxy_config.create_client()?;
        let response = client.get(url).send().await
            .map_err(|e| anyhow!("Failed to fetch {}: {}", url, e))?;
        let status = response.status();
        let content_type = response.headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("")
            .to_string();
        let is_xml = content_type.contains("xml") || url.ends_with(".xml");
        if !status.is_success() {
            return Err(anyhow!("HTTP error for {}: {}", url, status));
        }
        let body = response.text().await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;
        if is_xml {
            // Parse XML for URLs
            let mut urls = Vec::new();
            let mut reader = Reader::from_str(&body);
            reader.trim_text(true);
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(ref e)) => {
                        let tag: Vec<u8> = e.name().as_ref().to_vec();
                        if tag == b"loc" || tag == b"link" || tag == b"url" {
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                                let url_str = text.unescape().unwrap_or_default().to_string();
                                if !url_str.is_empty() {
                                    urls.push(url_str);
                                }
                            }
                        }
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => (),
                }
                buf.clear();
            }
            return Ok(urls);
        }
        // HTML: Only process if content-type is HTML
        if !content_type.contains("text/html") {
            return Ok(Vec::new()); // Skip non-HTML, non-XML content
        }
        self.extract_links(&body, url)
    }

    fn extract_links(&self, html: &str, base_url: &str) -> Result<Vec<String>> {
        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a[href]").unwrap();

        let links = document
            .select(&link_selector)
            .filter_map(|element| element.value().attr("href"))
            .filter_map(|href| self.resolve_url(base_url, href).ok())
            .filter(|link| self.is_valid_internal_url(link.as_str()))
            .collect::<Vec<String>>();

        Ok(links)
    }

    fn resolve_url(&self, base: &str, href: &str) -> Result<String, url::ParseError> {
        let base_url = Url::parse(base)?;
        let mut resolved = base_url.join(href)?;
        
        // Normalize URL
        resolved.set_fragment(None);
        
        let mut url_str = resolved.to_string();
        if url_str.ends_with('/') && url_str.matches('/').count() > 2 {
            url_str.pop();
        }
        
        Ok(url_str)
    }

    fn is_valid_internal_url(&self, url_str: &str) -> bool {
        // Check if it's an internal URL
        if !self.is_internal_url(url_str) {
            println!("[SKIP] External URL: {}", url_str);
            return false;
        }

        // Only skip URLs with unwanted file extensions at the end
        let url_lower = url_str.to_lowercase();
        let unwanted_exts = [
            ".pdf", ".doc", ".docx", ".zip", ".exe", ".dmg",
            ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp",
            ".mp3", ".mp4", ".avi", ".mov", ".wmv",
            ".css", ".js", ".xml", ".json",
        ];
        for ext in &unwanted_exts {
            if url_lower.ends_with(ext) {
                println!("[SKIP] Unwanted extension ({}): {}", ext, url_str);
                return false;
            }
        }

        // Skip mailto, tel, ftp, javascript, data links
        let unwanted_schemes = ["mailto:", "tel:", "ftp:", "javascript:", "data:"];
        for scheme in &unwanted_schemes {
            if url_lower.starts_with(scheme) {
                println!("[SKIP] Unwanted scheme ({}): {}", scheme, url_str);
                return false;
            }
        }

        // (Optional) Log allowed URLs for debugging
        // println!("[ALLOW] {}", url_str);
        true
    }

    fn is_internal_url(&self, url_str: &str) -> bool {
        if let Ok(url) = Url::parse(url_str) {
            if let Some(host) = url.host_str() {
                return host == self.base_domain || host.ends_with(&format!(".{}", self.base_domain));
            }
        }
        false
    }
}

impl Clone for Crawler {
    fn clone(&self) -> Self {
        Self {
            base_domain: self.base_domain.clone(),
            proxy_config: self.proxy_config.clone(),
            semaphore: Arc::clone(&self.semaphore),
            state: Arc::clone(&self.state),
        }
    }
}

fn save_urls_to_file(urls: &HashSet<String>, filename: &str) -> std::io::Result<()> {
    let mut file = File::create(filename)?;
    for url in urls {
        writeln!(file, "{}", url)?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let base_url = "https://jesus-is-savior.com";
    println!("Starting high-concurrency crawler for {}", base_url);
    let crawler = Crawler::new(base_url, CONCURRENT_REQUESTS)?;
    let discovered_urls = crawler.crawl().await?;
    save_urls_to_file(&discovered_urls, "urls.txt")?;
    println!("Crawling completed. Found {} unique URLs.", discovered_urls.len());
    println!("URLs saved to urls.txt");
    Ok(())
}