import { useEffect, useState, useRef } from "react";
import "./App.css";
import { Card } from "./components/ui/card";
import ReactMarkdown from 'react-markdown';

interface NarrativeElement {
  type: string;
  content?: string;
  url?: string;
}

interface Post {
  url: string;
  title?: string;
  main_content?: string;
  image_urls: string[];
  video_urls: string[];
  audio_urls: string[];
  external_links: string[];
  narrative: NarrativeElement[][]; // paragraphs
  avatar: string;
  username: string;
  date?: string;
}

const PAGE_SIZE = 20;

function App() {
  const [posts, setPosts] = useState<any[]>([]);
  const [search, setSearch] = useState("");
  const [shown, setShown] = useState(PAGE_SIZE); // AJAX-style load more
  const [loading, setLoading] = useState(true);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    fetch("/posts.json")
      .then((res) => res.json())
      .then((data) => {
        setPosts(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const filtered = posts.filter(
    (post) => {
      const meta = post.post.metadata;
      return (
        (meta.title && meta.title.toLowerCase().includes(search.toLowerCase())) ||
        (meta.content && meta.content.toLowerCase().includes(search.toLowerCase()))
      );
    }
  );
  const paged = filtered.slice(0, shown);

  return (
    <div className="container x-bg">
      <input
        className="search x-search"
        type="text"
        placeholder="Search posts..."
        value={search}
        onChange={(e) => {
          setSearch(e.target.value);
          setShown(PAGE_SIZE);
        }}
      />
      {loading ? (
        <p>Loading posts...</p>
      ) : filtered.length === 0 ? (
        <p>No posts found.</p>
      ) : (
        <>
          <div className="posts">
            {paged.map((post, postIdx) => {
              const meta = post.post.metadata;
              return (
                <Card
                  key={postIdx}
                  className="mb-6 max-w-xl mx-auto border border-gray-700 bg-[#16181c] text-white rounded-2xl shadow-md transition-transform hover:scale-[1.01] hover:shadow-lg p-5"
                  style={{ boxShadow: '0 2px 8px rgba(0,0,0,0.12)' }}
                >
                  <div className="flex items-center mb-2">
                    <img
                      src={meta.avatar}
                      alt={meta.username}
                      className="rounded-full w-11 h-11 mr-3 border border-gray-600"
                      style={{ objectFit: 'cover' }}
                    />
                    <div className="flex flex-col justify-center">
                      <span className="font-bold text-base leading-tight">{meta.username}</span>
                      {meta.date && (
                        <span className="text-xs text-gray-400 leading-tight">{meta.date}</span>
                      )}
                    </div>
                  </div>
                  {meta.title && (
                    <div className="font-bold text-lg mb-2 text-left leading-snug">{meta.title}</div>
                  )}
                  <div className="text-left text-[15px] leading-relaxed whitespace-pre-line">
                    <ReactMarkdown
                      components={{
                        img: (props: any) => (
                          <img {...props} className="my-3 rounded-xl max-w-full" style={{ maxHeight: 400, marginLeft: 0, ...props.style }} />
                        ),
                        a: (props: any) => (
                          <a {...props} className="text-blue-400 underline hover:text-blue-300" target="_blank" rel="noopener noreferrer" />
                        ),
                        audio: (props: any) => (
                          <audio {...props} controls className="my-2 w-full" />
                        ),
                        video: (props: any) => (
                          <video {...props} controls className="my-2 w-full rounded-xl" style={{ maxHeight: 400, ...props.style }} />
                        ),
                      }}
                    >
                      {meta.content || ""}
                    </ReactMarkdown>
                  </div>
                </Card>
              );
            })}
            {/* Hidden audio element for playback */}
            <audio ref={audioRef} style={{ display: "none" }} />
          </div>
          {shown < filtered.length && (
            <div style={{ textAlign: "center", margin: "2rem 0" }}>
              <button className="x-btn" onClick={() => setShown(shown + PAGE_SIZE)}>
                Load more
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default App;
