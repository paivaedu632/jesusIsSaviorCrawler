import { useEffect, useState, useRef } from "react";
import "./EnhancedApp.css";

interface NarrativeElement {
  type: string;
  content?: string;
  url?: string;
}

interface Post {
  url: string;
  title?: string;
  narrative: NarrativeElement[][]; // paragraphs
}

const PAGE_SIZE = 20;

function EnhancedApp() {
  const [posts, setPosts] = useState<Post[]>([]);
  const [search, setSearch] = useState("");
  const [shown, setShown] = useState(PAGE_SIZE);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<{ [url: string]: boolean }>({});
  const [audioLoading, setAudioLoading] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    fetch("/posts.json")
      .then((res) => res.json())
      .then((data) => {
        setPosts(data);
        setLoading(false);
      })
      .catch((error) => {
        console.error("Failed to load posts:", error);
        setLoading(false);
      });
  }, []);

  const searchInNarrative = (narrative: NarrativeElement[][], searchTerm: string): boolean => {
    const lowerSearch = searchTerm.toLowerCase();
    return narrative.some(paragraph =>
      paragraph.some(element => {
        if (element.type === "text" && element.content) {
          return element.content.toLowerCase().includes(lowerSearch);
        }
        if (element.type === "link" && element.content) {
          return element.content.toLowerCase().includes(lowerSearch);
        }
        return false;
      })
    );
  };

  const filtered = posts.filter((p) => {
    const searchTerm = search.toLowerCase();
    return (
      (p.title?.toLowerCase().includes(searchTerm) || false) ||
      p.url.toLowerCase().includes(searchTerm) ||
      searchInNarrative(p.narrative, searchTerm)
    );
  });

  const paged = filtered.slice(0, shown);

  const toggleExpand = (url: string) => {
    setExpanded((prev) => ({ ...prev, [url]: !prev[url] }));
  };

  const getTextContent = (narrative: NarrativeElement[][]): string => {
    return narrative
      .flat()
      .filter((el) => el.type === "text")
      .map((el) => el.content || "")
      .join(" ");
  };

  const getTotalTextLength = (narrative: NarrativeElement[][]): number => {
    return getTextContent(narrative).length;
  };

  const handleListen = async (post: Post) => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current.currentTime = 0;
      audioRef.current.src = "";
    }
    
    setAudioLoading(post.url);
    const textToSpeak = getTextContent(post.narrative);
    
    try {
      const response = await fetch("/api/tts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: textToSpeak })
      });
      
      if (!response.ok) throw new Error("TTS failed");
      
      const blob = await response.blob();
      const audioUrl = URL.createObjectURL(blob);
      
      if (audioRef.current) {
        audioRef.current.src = audioUrl;
        audioRef.current.play();
      }
    } catch (e) {
      console.error("TTS Error:", e);
      alert("Text-to-speech is not available");
    } finally {
      setAudioLoading(null);
    }
  };

  const renderNarrativeElement = (element: NarrativeElement, idx: number) => {
    switch (element.type) {
      case "text":
        return (
          <span key={idx} className="narrative-text">
            {element.content}
          </span>
        );
      
      case "image":
        return (
          <div key={idx} className="narrative-image">
            <img 
              src={element.url} 
              alt="Content image" 
              loading="lazy"
              onError={(e) => {
                const target = e.target as HTMLImageElement;
                target.style.display = 'none';
              }}
            />
          </div>
        );
      
      case "video":
        return (
          <div key={idx} className="narrative-video">
            {element.url?.startsWith("videos/") ? (
              <video
                src={element.url}
                controls
                preload="metadata"
                onError={(e) => {
                  const target = e.target as HTMLVideoElement;
                  target.style.display = 'none';
                }}
              >
                Your browser does not support the video tag.
              </video>
            ) : (
              <iframe
                src={element.url}
                allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                allowFullScreen
                title={`video-${idx}`}
                onError={(e) => {
                  const target = e.target as HTMLIFrameElement;
                  target.style.display = 'none';
                }}
              />
            )}
          </div>
        );
      
      case "link":
        return (
          <a
            key={idx}
            href={element.url}
            target="_blank"
            rel="noopener noreferrer"
            className="narrative-link"
          >
            {element.content || element.url}
          </a>
        );
      
      default:
        return null;
    }
  };

  const renderPost = (post: Post, index: number) => {
    const isExpanded = expanded[post.url];
    const totalTextLength = getTotalTextLength(post.narrative);
    const showExpandButton = totalTextLength > 500;
    const textToSpeak = getTextContent(post.narrative);
    
    // Limit paragraphs shown when collapsed
    const paragraphsToShow = isExpanded ? post.narrative : post.narrative.slice(0, 3);
    const hasMoreParagraphs = post.narrative.length > 3;

    return (
      <article key={post.url + index} className="post-card">
        <header className="post-header">
          <h2>
            <a
              href={post.url}
              target="_blank"
              rel="noopener noreferrer"
              className="post-title"
            >
              {post.title || "Untitled Post"}
            </a>
          </h2>
          <div className="post-url">
            {new URL(post.url).pathname}
          </div>
        </header>

        <div className="post-content">
          {paragraphsToShow.map((paragraph, pIdx) => (
            <div key={pIdx} className="narrative-paragraph">
              {paragraph.map((element, eIdx) => renderNarrativeElement(element, eIdx))}
            </div>
          ))}
          
          {!isExpanded && hasMoreParagraphs && (
            <div className="content-fade">
              <div className="fade-overlay"></div>
            </div>
          )}
        </div>

        <footer className="post-actions">
          {showExpandButton && (
            <button
              className="action-btn expand-btn"
              onClick={() => toggleExpand(post.url)}
            >
              {isExpanded ? "Show Less" : "Read More"}
            </button>
          )}
          
          <button
            className="action-btn listen-btn"
            onClick={() => handleListen(post)}
            disabled={!textToSpeak || audioLoading === post.url}
          >
            {audioLoading === post.url ? "Loading..." : "🔊 Listen"}
          </button>
          
          <div className="post-stats">
            {post.narrative.length} paragraphs • {Math.ceil(totalTextLength / 1000)}k chars
          </div>
        </footer>
      </article>
    );
  };

  return (
    <div className="enhanced-app">
      <header className="app-header">
        <h1>Jesus Is Savior Content</h1>
        <div className="search-container">
          <input
            className="search-input"
            type="text"
            placeholder="Search posts, content, or links..."
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setShown(PAGE_SIZE);
            }}
          />
          <div className="search-stats">
            {filtered.length} of {posts.length} posts
          </div>
        </div>
      </header>

      <main className="app-main">
        {loading ? (
          <div className="loading-state">
            <div className="loading-spinner"></div>
            <p>Loading posts...</p>
          </div>
        ) : filtered.length === 0 ? (
          <div className="empty-state">
            <p>No posts found matching your search.</p>
            {search && (
              <button 
                className="action-btn"
                onClick={() => setSearch("")}
              >
                Clear Search
              </button>
            )}
          </div>
        ) : (
          <>
            <div className="posts-grid">
              {paged.map((post, index) => renderPost(post, index))}
            </div>
            
            {shown < filtered.length && (
              <div className="load-more-container">
                <button 
                  className="action-btn load-more-btn"
                  onClick={() => setShown(shown + PAGE_SIZE)}
                >
                  Load More Posts ({filtered.length - shown} remaining)
                </button>
              </div>
            )}
          </>
        )}
      </main>

      {/* Hidden audio element for TTS playback */}
      <audio ref={audioRef} style={{ display: "none" }} />
    </div>
  );
}

export default EnhancedApp;
