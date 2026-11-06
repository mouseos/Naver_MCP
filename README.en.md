# Naver Search MCP

An MCP server that lets AI agents search Naver (네이버). **No API key, no account, no configuration.**

[日本語](README.md)

[![CI](https://github.com/mouseos/Naver_MCP/actions/workflows/ci.yml/badge.svg)](https://github.com/mouseos/Naver_MCP/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Supported searches

| Tool | Search | Notes |
|---|---|---|
| `naver_search` | Integrated search (by subject section) | Up to 10 results per section |
| `naver_web_search` | Web documents | One top-selection batch (no paging) |
| `naver_image_search` | Images | Original image URL recovered; paginated |
| `naver_news_search` | News | Paginated |
| `naver_video_search` | Videos | Paginated |
| `naver_blog_search` | Blog posts | Paginated |
| `naver_suggest` | Autocomplete suggestions | — |
| `naver_build_search_url` | Build a search URL | No network access |
| `naver_list_verticals` | List the above | — |

The Cafe (카페) and 지식iN tabs exist in the app but could not be verified over plain HTTP, so they are explicitly listed as unsupported (see `unsupported` in `naver_list_verticals`) rather than advertised.

### About paging

Naver's mobile tabs return exactly one batch per page load and ignore their URL's `page`/`start` parameters (measured: the page 1 and page 2 URLs return identical content).

- Images, news, videos and blog posts page by following the "more" API URL each tab page itself issues (the chain is followed from page 1, so page N costs N requests), up to page 10
- The web-documents tab is a fixed top-selection batch with no such API, so instead of silently repeating page 1, page 2 and beyond return an explicit `unsupported_page` error

These pages do not report a total hit count, so results carry no `total_hits` field.

## Install

### 1. From a release

1. Download the ZIP for your OS from [Releases](https://github.com/mouseos/Naver_MCP/releases) and extract it
2. Run `naver-mcp-launcher` and click **Install** for your agent
3. Restart the agent

The launcher auto-detects Claude Desktop / Claude Code / Google Antigravity / Codex CLI and registers the server for you.

### 2. From the command line

```bash
# Print the config JSON and commands
naver-mcp setup

# Claude Code
claude mcp add Naver_MCP -- /path/to/naver-mcp
```

### 3. Manual configuration

```json
{
  "mcpServers": {
    "Naver_MCP": {
      "command": "/path/to/naver-mcp",
      "args": []
    }
  }
}
```

Leave `env` empty — there is nothing to configure.

### 4. Build from source

```bash
git clone https://github.com/mouseos/Naver_MCP.git
cd Naver_MCP
cargo build --release
# → target/release/naver-mcp
```

Rust 1.80+ is required (for `std::sync::LazyLock`). On Linux the launcher additionally needs `libfontconfig1-dev libxkbcommon-dev libgtk-3-dev`; the server itself has no system dependencies.

## Use as a Rust library

The search logic lives in the provider crate (package `naver-mcp-provider`, imported under the lib name `naver_search`), so you can use it directly from your own Rust program without the MCP server. No API key, login or configuration is needed, and it works anonymously just like the MCP version.

```toml
[dependencies]
# From GitHub
naver_search = { git = "https://github.com/mouseos/Naver_MCP", package = "naver-mcp-provider" }
# From a local checkout
# naver_search = { path = "../Naver_MCP/crates/providers/naver", package = "naver-mcp-provider" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

```rust
use naver_search::NaverSearchClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = NaverSearchClient::new();
    let result = client.web_search("제주도 여행", 1, 5).await?;
    for item in &result.items {
        println!("{} — {}", item.title, item.url);
    }
    let suggest = client.suggest("네이").await?;
    for s in &suggest.suggestions {
        println!("{}", s.keyword);
    }
    Ok(())
}
```

Public API (everything is importable from the crate root):

| Kind | Names |
|---|---|
| Client | `NaverSearchClient` (`search` / `web_search` / `image_search` / `news_search` / `video_search` / `blog_search` / `suggest`, all returning `anyhow::Result`) |
| Shared result types | `SearchResult<T>`, `PROVIDER`, `verticals()` |
| Per-search result types | `IntegratedResult` / `IntegratedSection` / `IntegratedItem`, `WebItem`, `ImageItem`, `NewsItem`, `VideoItem`, `BlogItem`, `SuggestResult` / `Suggestion` |
| URL building | `build_search_url` / `build_page_url` / `suggest_url` (first pages only — deeper pages are not URLs) |
| Paging helpers | `extract_more_url` / `parse_more_response` / `MorePayload` / `MAX_PAGE` (extract the more-API URL a tab page issues, and parse its responses) |
| Offline parsers | `parse_integrated_results` / `parse_web_results` / `parse_image_results` / `extract_image_items` / `parse_news_results` / `parse_video_results` / `parse_blog_results` / `parse_suggest_json` (extract from saved page HTML/JSON), `clean_text`, `decode_entities`, `urlencode` |

Result types derive serde `Serialize` / `Deserialize`, so they can be saved to and loaded back from JSON. Offline examples live in `crates/providers/naver/examples/offline_basics.rs` and `build_search_url.rs`; `crates/providers/naver/tests/library_api.rs` holds tests written exactly the way an external user would write them.

When a search fails through the MCP server, the tool result is `{provider, ok:false, error:{kind, message}}` (the form shared with Baidu_MCP / Yahoo-Japan-Search-MCP). Argument errors (such as a missing query) are returned as MCP `isError` results as usual.

## Usage examples

Instructions to an agent:

- "제주도의 맛집을 네이버 블로그에서 찾아줘" → `naver_blog_search`
- "The latest Korean news on the economy, 5 items" → `naver_news_search`
- "고양이 사진 찾아줘" → `naver_image_search`
- "How do people phrase this topic on Naver?" → `naver_suggest`

Tool call:

```json
{
  "name": "naver_web_search",
  "arguments": { "query": "제주도 여행", "limit": 5 }
}
```

Response (excerpt):

```json
{
  "provider": "naver",
  "vertical": "web",
  "ok": true,
  "query": "제주도 여행",
  "page": 1,
  "count": 5,
  "items": [
    {
      "rank": 1,
      "title": "제주도 여행 코스 총정리",
      "url": "https://example.com/jeju-course",
      "snippet": "2박 3일 일정으로 돌아보는 제주 동쪽 코스…"
    }
  ]
}
```

Korean queries return far better results than the same query in English.

## Design

```
crates/
├── providers/naver/          # Search logic
│   ├── common.rs             #   HTTP, HTML cleanup, card-boundary helpers
│   ├── urlbuild.rs           #   App-style search URL generation
│   ├── search.rs web.rs image.rs news.rs video.rs blog.rs suggest.rs
│   └── lib.rs                #   HTTP client and shared response types
├── server/                   # MCP server proper (stdio JSON-RPC 2.0)
│   ├── main.rs               #   Protocol handling
│   └── tools.rs              #   Tool definitions and dispatch
└── launcher/                 # Registration GUI (egui)
```

The search behaviour was established by analysing the official Naver Android app's APK. The app opens `https://m.search.naver.com/search.naver?where=m&ie=utf8&query=...&sm=...` in a WebView — there is no signed native search API — and this server reads the same mobile pages. The desktop host only ships a JavaScript shell over plain HTTP, so all fetching uses the mobile host with a mobile user agent. Verticals switch with `ssc=tab.m_*.all` plus `where=m_*`; the legacy `where=image`-style values are silently ignored by the new UI. The image tab embeds its results as JSON inside `imageSearchTabData` rather than DOM, and that payload is what gets parsed.

## Development

```bash
cargo test --workspace                  # All tests (including E2E against live Naver)
cargo test --workspace -- --skip e2e_   # Offline tests only
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The `e2e_*` tests connect to the real Naver and verify each search parses correctly. They run manually in CI to catch Naver-side changes. The offline parser tests are pinned to real pages captured during the APK analysis (`crates/providers/naver/tests/fixtures/`).

Manual check over stdio:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"naver_web_search","arguments":{"query":"테스트","limit":2}}}' \
  | ./target/debug/naver-mcp
```

## Notes

- This reads publicly served search result pages. Please use it at a sensible rate.
- If Naver changes its page structure, the affected search may temporarily fail; the error message will say so.
- This project is not affiliated with NAVER Corporation.

## License

MIT — see [LICENSE](LICENSE).
