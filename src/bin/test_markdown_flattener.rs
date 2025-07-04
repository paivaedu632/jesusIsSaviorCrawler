use scraper::Html;
use jesus_is_savior_crawler::markdown_flattener::html_to_markdown;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test HTML content
    let html_content = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test Page</title>
    </head>
    <body>
        <h1>Main Title</h1>
        <p>This is a paragraph with <strong>bold text</strong> and <em>italic text</em>.</p>
        
        <h2>Section with Media</h2>
        <p>Here's an image: <img src="image.jpg" alt="Test Image"></p>
        <p>And a link to <a href="https://example.com">Example.com</a></p>
        
        <h3>Media Files</h3>
        <p>Audio file: <a href="audio.mp3">Listen to this audio</a></p>
        <p>Video file: <a href="video.mp4">Watch this video</a></p>
        
        <div>
            Some content in a div<br>
            With a line break
        </div>
        
        <blockquote>This would be a quote if we had blockquote support</blockquote>
    </body>
    </html>
    "#;

    // Parse the HTML
    let document = Html::parse_document(html_content);
    
    // Convert to markdown
    let base_url = "https://jesus-is-savior.com";
    let markdown = html_to_markdown(&document, base_url).await;
    
    println!("Generated Markdown:");
    println!("==================");
    println!("{}", markdown);
    
    Ok(())
}
