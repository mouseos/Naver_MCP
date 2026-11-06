//! Naver Search MCP server — JSON-RPC 2.0 over stdio.

#![windows_subsystem = "windows"]

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use naver_search::NaverSearchClient;

mod tools;

const SERVER_NAME: &str = "Naver_MCP";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

const INSTRUCTIONS: &str = "\
Search Naver — integrated search, web documents, images, news, videos and blog posts, plus \
autocomplete suggestions. No API key, login or configuration is required.

Reach for this server whenever a question is about Korea or is best answered by Korean-language \
sources: Korean web pages, blogs, news, shops, government and corporate sites, or what Korean \
users are saying right now. Naver indexes this material far more deeply than general-purpose \
engines.

Queries in Korean return substantially better results than the same query in English. \
Call naver_list_verticals if you are unsure which search tool fits the question.";

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("setup") {
        print_setup();
        return;
    }

    let client = NaverSearchClient::new();

    // Orphan watchdog (same scheme as Shopping_MCP / Baidu_MCP): a stdio
    // MCP server whose client died must not linger. When the parent pid
    // changes, exit.
    #[cfg(unix)]
    {
        let parent_pid = unsafe { libc::getppid() };
        if parent_pid > 1 {
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if unsafe { libc::getppid() } != parent_pid {
                    std::process::exit(0);
                }
            });
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(request) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(&id),
            "ping" => json_rpc_response(&id, json!({})),
            "tools/list" => handle_tools_list(&id),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                handle_tools_call(&id, &params, &client).await
            }
            "resources/list" => json_rpc_response(&id, json!({"resources": []})),
            "prompts/list" => json_rpc_response(&id, json!({"prompts": []})),
            _ => {
                // Notifications carry no id and expect no reply.
                if method.starts_with("notifications/") || id.is_none() {
                    continue;
                }
                json_rpc_error(&id, -32601, &format!("Method not found: {method}"))
            }
        };

        let Ok(out) = serde_json::to_string(&response) else {
            continue;
        };
        let _ = writeln!(stdout, "{out}");
        let _ = stdout.flush();
    }
}

fn handle_initialize(id: &Option<Value>) -> Value {
    json_rpc_response(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
            "instructions": INSTRUCTIONS,
        }),
    )
}

fn handle_tools_list(id: &Option<Value>) -> Value {
    json_rpc_response(id, json!({"tools": tools::tool_definitions()}))
}

async fn handle_tools_call(
    id: &Option<Value>,
    params: &Value,
    client: &NaverSearchClient,
) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match tools::dispatch(tool_name, &args, client).await {
        Ok(content) => {
            let text = serde_json::to_string(&content).unwrap_or_default();
            json_rpc_response(id, json!({"content": [{"type": "text", "text": text}]}))
        }
        Err(e) => json_rpc_response(
            id,
            json!({
                "content": [{"type": "text", "text": format!("Error: {e}")}],
                "isError": true,
            }),
        ),
    }
}

fn json_rpc_response(id: &Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn json_rpc_error(id: &Option<Value>, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

// ── `setup` subcommand ──

fn attach_parent_console() {
    #[cfg(windows)]
    {
        extern "system" {
            fn AttachConsole(pid: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

fn is_japanese_locale() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        extern "system" {
            fn GetUserDefaultLocaleName(p: *mut u16, n: i32) -> i32;
        }
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), 85) };
        len > 0
            && OsString::from_wide(&buf[..len as usize])
                .to_string_lossy()
                .to_lowercase()
                .starts_with("ja")
    }
    #[cfg(not(windows))]
    {
        std::env::var("LANG")
            .unwrap_or_default()
            .to_lowercase()
            .starts_with("ja")
    }
}

fn print_setup() {
    attach_parent_console();
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_str = exe.to_string_lossy();
    let config = serde_json::to_string_pretty(&json!({
        "mcpServers": {
            SERVER_NAME: { "command": &*exe_str, "args": [] }
        }
    }))
    .unwrap_or_default();
    let cc_cmd = format!("claude mcp add {SERVER_NAME} -- {exe_str}");
    let sep = "=".repeat(60);

    if is_japanese_locale() {
        eprintln!(
            "\n{sep}\n  {SERVER_NAME} サーバー起動準備完了\n{sep}\n\n\
             APIキー・設定は不要です。以下のJSONをAIエージェントのMCP設定に追加してください。\n\n\
             ■ Claude Desktop:\n  設定 → Developer → Edit Config で claude_desktop_config.json を開き、以下を追加:\n\n\
             {config}\n\n\
             ■ Claude Code:\n  以下のコマンドを実行:\n\n  {cc_cmd}\n\n\
             ■ Google Antigravity:\n  ~/.gemini/config/mcp_config.json を開き、以下を追加:\n\n\
             {config}\n\n\
             ■ その他のMCPクライアント:\n  MCP設定に上記のJSONを追加してください。\n\n\
             {sep}\n  設定完了後、エージェントを再起動してください。\n\
               (Claude Desktopはアプリ再起動、Claude Codeは新しいセッション開始)\n{sep}\n"
        );
    } else {
        eprintln!(
            "\n{sep}\n  {SERVER_NAME} server ready\n{sep}\n\n\
             No API key or configuration needed. Add the following JSON to your agent's MCP config.\n\n\
             ■ Claude Desktop:\n  Settings -> Developer -> Edit Config, then add:\n\n\
             {config}\n\n\
             ■ Claude Code:\n  Run this command:\n\n  {cc_cmd}\n\n\
             ■ Google Antigravity:\n  Open ~/.gemini/config/mcp_config.json and add:\n\n\
             {config}\n\n\
             ■ Other MCP clients:\n  Add the JSON above to your MCP settings.\n\n\
             {sep}\n  After setup, restart your agent.\n\
               (Claude Desktop: restart the app, Claude Code: start a new session)\n{sep}\n"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn call_tool(client: &NaverSearchClient, name: &str, args: Value) -> Value {
        let params = json!({"name": name, "arguments": args});
        let resp = handle_tools_call(&Some(json!(1)), &params, client).await;
        assert!(
            resp["result"]["isError"] != json!(true),
            "{name} returned an error: {}",
            resp["result"]["content"][0]["text"]
        );
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("{}");
        serde_json::from_str(text).unwrap_or(json!({"_raw": text}))
    }

    // ── Protocol-level tests (offline) ──

    #[test]
    fn initialize_reports_name_and_protocol() {
        let resp = handle_initialize(&Some(json!(1)));
        assert_eq!(resp["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(resp["result"]["instructions"].as_str().unwrap().len() > 100);
    }

    #[test]
    fn tools_list_exposes_every_tool() {
        let resp = handle_tools_list(&Some(json!(1)));
        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in [
            "naver_search",
            "naver_web_search",
            "naver_image_search",
            "naver_news_search",
            "naver_video_search",
            "naver_blog_search",
            "naver_suggest",
            "naver_build_search_url",
            "naver_list_verticals",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn unknown_method_is_a_json_rpc_error() {
        let resp = json_rpc_error(&Some(json!(1)), -32601, "Method not found: foo/bar");
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn unknown_tool_is_reported_as_tool_error() {
        let client = NaverSearchClient::new();
        let params = json!({"name": "nope", "arguments": {}});
        let resp = handle_tools_call(&Some(json!(1)), &params, &client).await;
        assert_eq!(resp["result"]["isError"], true);
    }

    #[tokio::test]
    async fn missing_query_is_reported_as_tool_error() {
        let client = NaverSearchClient::new();
        let params = json!({"name": "naver_web_search", "arguments": {}});
        let resp = handle_tools_call(&Some(json!(1)), &params, &client).await;
        assert_eq!(resp["result"]["isError"], true);
    }

    #[tokio::test]
    async fn list_verticals_is_offline_and_complete() {
        let client = NaverSearchClient::new();
        let result = call_tool(&client, "naver_list_verticals", json!({})).await;
        assert_eq!(result["config_required"], false);
        assert_eq!(result["verticals"].as_array().unwrap().len(), 7);
        assert_eq!(result["unsupported"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn build_search_url_is_offline() {
        let client = NaverSearchClient::new();
        let result = call_tool(
            &client,
            "naver_build_search_url",
            json!({"query": "테스트", "vertical": "image"}),
        )
        .await;
        assert_eq!(result["ok"], true);
        assert_eq!(result["vertical"], "image");
        assert!(result["url"]
            .as_str()
            .unwrap()
            .contains("ssc=tab.m_image.all"));
    }

    #[tokio::test]
    async fn unservable_pages_are_structured_errors() {
        let client = NaverSearchClient::new();
        // No network involved in either case: the web tab has no page 2,
        // and chained paging is capped at MAX_PAGE — both refuse before
        // any fetch.
        let web = call_tool(
            &client,
            "naver_web_search",
            json!({"query": "테스트", "page": 2}),
        )
        .await;
        assert_eq!(web["ok"], false);
        assert_eq!(web["error"]["kind"], "unsupported_page");

        let deep = call_tool(
            &client,
            "naver_blog_search",
            json!({"query": "테스트", "page": 11}),
        )
        .await;
        assert_eq!(deep["ok"], false);
        assert_eq!(deep["error"]["kind"], "unsupported_page");
    }

    // ── Live end-to-end tests (hit Naver) ──

    #[tokio::test]
    async fn e2e_integrated_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(&client, "naver_search", json!({"query": "테스트"})).await;
        assert_eq!(r["ok"], true, "integrated search failed: {r}");
        let sections = r["sections"].as_array().unwrap();
        assert!(!sections.is_empty(), "no sections returned: {r}");
        assert!(r["count"].as_u64().unwrap_or(0) > 0);
        for section in sections {
            assert!(!section["items"].as_array().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn e2e_web_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(
            &client,
            "naver_web_search",
            json!({"query": "테스트", "limit": 5}),
        )
        .await;
        assert_eq!(r["ok"], true, "web search failed: {r}");
        let items = r["items"].as_array().unwrap();
        assert!(!items.is_empty(), "web search returned 0 items: {r}");
        assert!(items.len() <= 5);
        for item in items {
            assert!(item["url"].as_str().unwrap().starts_with("http"));
            assert!(!item["title"].as_str().unwrap().is_empty());
            // Highlight markup must never survive into tool output.
            assert!(!item["title"].as_str().unwrap().contains("<mark>"));
        }
    }

    /// Fetch pages 1 and 2 of a chained vertical and assert page 2
    /// brings results page 1 did not have.
    async fn assert_paging(tool: &str, query: &str, limit: i64) {
        let client = NaverSearchClient::new();
        let p1 = call_tool(
            &client,
            tool,
            json!({"query": query, "page": 1, "limit": limit}),
        )
        .await;
        let p2 = call_tool(
            &client,
            tool,
            json!({"query": query, "page": 2, "limit": limit}),
        )
        .await;
        assert_eq!(p2["ok"], true, "{tool} page 2 failed: {p2}");
        let urls = |r: &Value| -> Vec<String> {
            r["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["url"].as_str().unwrap().to_string())
                .collect()
        };
        let (urls1, urls2) = (urls(&p1), urls(&p2));
        assert!(!urls2.is_empty(), "{tool} page 2 was empty");
        assert!(
            urls2.iter().any(|u| !urls1.contains(u)),
            "{tool} page 2 duplicated page 1"
        );
    }

    #[tokio::test]
    async fn e2e_news_search_paging_returns_new_results() {
        assert_paging("naver_news_search", "경제", 15).await;
    }

    #[tokio::test]
    async fn e2e_video_search_paging_returns_new_results() {
        assert_paging("naver_video_search", "테스트", 48).await;
    }

    #[tokio::test]
    async fn e2e_blog_search_paging_returns_new_results() {
        assert_paging("naver_blog_search", "테스트", 30).await;
    }

    #[tokio::test]
    async fn e2e_image_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(
            &client,
            "naver_image_search",
            json!({"query": "고양이", "limit": 5}),
        )
        .await;
        assert_eq!(r["ok"], true, "image search failed: {r}");
        let items = r["items"].as_array().unwrap();
        assert!(!items.is_empty(), "image search returned 0 items: {r}");
        for item in items {
            let url = item["url"].as_str().unwrap();
            assert!(url.starts_with("http"));
            assert!(
                !url.contains("search.pstatic.net"),
                "original URL must not be the proxy: {url}"
            );
            assert!(item["thumbnail"].as_str().unwrap_or("").starts_with("http"));
        }
    }

    #[tokio::test]
    async fn e2e_image_search_paging_returns_new_results() {
        let client = NaverSearchClient::new();
        let p1 = call_tool(
            &client,
            "naver_image_search",
            json!({"query": "고양이", "page": 1, "limit": 50}),
        )
        .await;
        let p2 = call_tool(
            &client,
            "naver_image_search",
            json!({"query": "고양이", "page": 2, "limit": 50}),
        )
        .await;
        assert_eq!(p2["ok"], true, "image page 2 failed: {p2}");
        let urls1: Vec<&str> = p1["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["url"].as_str().unwrap())
            .collect();
        let urls2: Vec<&str> = p2["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["url"].as_str().unwrap())
            .collect();
        assert!(!urls2.is_empty(), "image page 2 was empty");
        assert!(
            urls2.iter().any(|u| !urls1.contains(u)),
            "image page 2 duplicated page 1"
        );
    }

    #[tokio::test]
    async fn e2e_news_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(
            &client,
            "naver_news_search",
            json!({"query": "경제", "limit": 5}),
        )
        .await;
        assert_eq!(r["ok"], true, "news search failed: {r}");
        let items = r["items"].as_array().unwrap();
        assert!(!items.is_empty(), "news search returned 0 items: {r}");
        for item in items {
            assert!(item["url"].as_str().unwrap().starts_with("http"));
            assert!(!item["title"].as_str().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn e2e_video_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(
            &client,
            "naver_video_search",
            json!({"query": "테스트", "limit": 5}),
        )
        .await;
        assert_eq!(r["ok"], true, "video search failed: {r}");
        let items = r["items"].as_array().unwrap();
        assert!(!items.is_empty(), "video search returned 0 items: {r}");
        for item in items {
            assert!(item["url"].as_str().unwrap().starts_with("http"));
            assert!(!item["title"].as_str().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn e2e_blog_search() {
        let client = NaverSearchClient::new();
        let r = call_tool(
            &client,
            "naver_blog_search",
            json!({"query": "테스트", "limit": 5}),
        )
        .await;
        assert_eq!(r["ok"], true, "blog search failed: {r}");
        let items = r["items"].as_array().unwrap();
        assert!(!items.is_empty(), "blog search returned 0 items: {r}");
        for item in items {
            assert!(item["url"].as_str().unwrap().contains("blog.naver.com"));
            assert!(!item["title"].as_str().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn e2e_suggest() {
        let client = NaverSearchClient::new();
        let r = call_tool(&client, "naver_suggest", json!({"query": "네이"})).await;
        assert_eq!(r["ok"], true, "suggest failed: {r}");
        let suggestions = r["suggestions"].as_array().unwrap();
        assert!(!suggestions.is_empty(), "suggest returned 0 items: {r}");
        assert!(suggestions
            .iter()
            .all(|s| !s["keyword"].as_str().unwrap().is_empty()));
    }
}
