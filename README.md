# Naver Search MCP

Naver（네이버）の検索をAIエージェントから使えるようにするMCPサーバー。**APIキー不要・アカウント不要・設定不要**。

[English](README.en.md)

[![CI](https://github.com/mouseos/Naver_MCP/actions/workflows/ci.yml/badge.svg)](https://github.com/mouseos/Naver_MCP/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 対応する検索

| ツール | 検索 | 備考 |
|---|---|---|
| `naver_search` | 統合検索（セクション別） | 1セクションあたり最大10件 |
| `naver_web_search` | ウェブ文書 | 上位抜粋の1バッチのみ（ページングなし） |
| `naver_image_search` | 画像 | 元画像URLを復元。ページングあり |
| `naver_news_search` | ニュース | ページングあり |
| `naver_video_search` | 動画 | ページングあり |
| `naver_blog_search` | ブログ | ページングあり |
| `naver_suggest` | サジェスト（検索語の自動補完） | — |
| `naver_build_search_url` | 検索URLの生成 | ネットワーク不要 |
| `naver_list_verticals` | 上記の一覧を返す | — |

カフェ（카페）と 지식iN のタブはアプリ内に存在しますが、通常HTTPでの取得を検証できていないため、対応外として明示しています（`naver_list_verticals` の `unsupported` に理由付きで載せています）。

### ページングについて

Naverのモバイルタブは、1回のページ読み込みで1バッチだけを返し、URLの `page` / `start` パラメータは無視されます（1ページ目と2ページ目のURLで同一の内容が返ることを実測で確認しました）。

- 画像・ニュース・動画・ブログは、各タブページ自身が発行する「もっと見る」APIのURLをたどることでページングします（ページ1からチェーンをたどるため、ページNの取得にはN回のリクエストを使います）。上限は10ページです
- ウェブ文書タブは上位結果の抜粋バッチのみで、このAPIも存在しないため、**1ページ目を黙って繰り返さず、2ページ目以降は `unsupported_page` エラーを明示的に返します**

また、この形式のページには総件数が出ないため、結果に `total_hits` は含みません。

## インストール

### 1. リリースから

1. [Releases](https://github.com/mouseos/Naver_MCP/releases) から自分のOS向けZIPをダウンロードして展開
2. `naver-mcp-launcher` を起動し、使いたいエージェントの **インストール** をクリック
3. エージェントを再起動

ランチャーは Claude Desktop / Claude Code / Google Antigravity / Codex CLI を自動検出して登録します。

### 2. コマンドラインから

```bash
# 設定用のJSONとコマンドを表示
naver-mcp setup

# Claude Code
claude mcp add Naver_MCP -- /path/to/naver-mcp
```

### 3. 手動設定

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

`env` は空のままで構いません。設定する項目はありません。

### 4. ソースからビルド

```bash
git clone https://github.com/mouseos/Naver_MCP.git
cd Naver_MCP
cargo build --release
# → target/release/naver-mcp
```

Rust 1.80以上が必要です（`std::sync::LazyLock` のため）。Linuxではランチャーのビルドに
`libfontconfig1-dev libxkbcommon-dev libgtk-3-dev` が追加で必要です。サーバー本体に
システム依存はありません。

## Rustライブラリとして使う

検索ロジックは provider crate（package名 `naver-mcp-provider`、import名は
lib名の `naver_search`）にあり、MCPサーバーを介さず自分のRustプログラムから
そのまま使えます。APIキー・ログイン・設定は不要で、MCP版と同じく匿名で動きます。

```toml
[dependencies]
# GitHubから
naver_search = { git = "https://github.com/mouseos/Naver_MCP", package = "naver-mcp-provider" }
# ローカルチェックアウトから
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

公開API（すべて crate ルートから import できます）:

| 種類 | 名前 |
|---|---|
| クライアント | `NaverSearchClient`（`search` / `web_search` / `image_search` / `news_search` / `video_search` / `blog_search` / `suggest`、戻り値は `anyhow::Result`） |
| 共通の結果型 | `SearchResult<T>`、`PROVIDER`、`verticals()` |
| 各検索の結果型 | `IntegratedResult` / `IntegratedSection` / `IntegratedItem`、`WebItem`、`ImageItem`、`NewsItem`、`VideoItem`、`BlogItem`、`SuggestResult` / `Suggestion` |
| URL生成 | `build_search_url` / `build_page_url` / `suggest_url`（URLで表せるのは1ページ目のみ） |
| ページング補助 | `extract_more_url` / `parse_more_response` / `MorePayload` / `MAX_PAGE`（タブページが発行する「もっと見る」APIのURL抽出と応答解析） |
| オフライン用パーサー | `parse_integrated_results` / `parse_web_results` / `parse_image_results` / `extract_image_items` / `parse_news_results` / `parse_video_results` / `parse_blog_results` / `parse_suggest_json`（保存したページHTML・JSONから抽出）、`clean_text`、`decode_entities`、`urlencode` |

結果型は serde の `Serialize` / `Deserialize` に対応し、JSONへの保存と読み戻しが
できます。ネットワーク不要の利用例が
`crates/providers/naver/examples/offline_basics.rs` と `build_search_url.rs` に、
外部利用の形そのままのテストが `crates/providers/naver/tests/library_api.rs` にあります。

なお、MCPサーバー経由で検索が失敗した場合は、ツール結果として
`{provider, ok:false, error:{kind, message}}` の形で返ります（Baidu_MCP / Yahoo-Japan-Search-MCP と共通の形式）。
引数の誤り（クエリ未指定など）は従来どおり MCP の `isError` で返ります。

## 使用例

エージェントへの指示例:

- 「제주도의 맛집을 네이버 블로그에서 찾아줘」 → `naver_blog_search`
- 「韓国で今話題のニュースを5件」 → `naver_news_search`
- 「고양이 사진 찾아줘」 → `naver_image_search`
- 「네이버에서 이 주제를 사람들이 어떻게 검색하는지」 → `naver_suggest`

ツール呼び出し:

```json
{
  "name": "naver_web_search",
  "arguments": { "query": "제주도 여행", "limit": 5 }
}
```

レスポンス（抜粋）:

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

韓国語のクエリの方が英語より遥かに良い結果が返ります。

## 設計

```
crates/
├── providers/naver/          # 検索ロジック
│   ├── common.rs             #   HTTP・HTML整形・カード境界の汎用処理
│   ├── urlbuild.rs           #   アプリ方式の検索URL生成
│   ├── search.rs web.rs image.rs news.rs video.rs blog.rs suggest.rs
│   └── lib.rs                #   HTTPクライアントと共通レスポンス型
├── server/                   # MCPサーバー本体（stdio JSON-RPC 2.0）
│   ├── main.rs               #   プロトコル処理
│   └── tools.rs              #   ツール定義とディスパッチ
└── launcher/                 # 登録用GUI（egui）
```

検索仕様は公式Naverアプリ（Android）のAPK解析で確定したものです。アプリは
`https://m.search.naver.com/search.naver?where=m&ie=utf8&query=...&sm=...` を
WebViewで開く方式で、ネイティブな署名付き検索APIはありません。本サーバーも同じ
モバイルページを読み取ります。PC版ホストは通常HTTPではJavaScriptのシェルしか
返さないため、取得はモバイルホスト＋モバイルUAに統一しています。バーティカルの
切替は旧形式の `where=image` 等では効かず、`ssc=tab.m_*.all` と `where=m_*` の組が
実形式です。画像タブは結果をDOMではなく `imageSearchTabData` のJSONで埋め込むため、
そちらを解析します。

## 開発

```bash
cargo test --workspace                  # 全テスト（実際にNaverへ通信するE2E含む）
cargo test --workspace -- --skip e2e_   # ネットワーク不要のテストのみ
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

`e2e_*` テストは実際の Naver に接続し、各検索が正しく解析できているかを検証します。
CIでは手動実行され、Naver 側の変更を検出します。パーサーのオフラインテストは
APK解析時に保存した実ページ（`crates/providers/naver/tests/fixtures/`）で固定しています。

stdio越しの手動確認:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"x","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"naver_web_search","arguments":{"query":"테스트","limit":2}}}' \
  | ./target/debug/naver-mcp
```

## 注意

- 公開されている検索結果ページを読み取ります。常識的な頻度で利用してください。
- Naver がページ構造を変更した場合、該当する検索が一時的に失敗することがあります。エラーメッセージにその旨が示されます。
- 本プロジェクトは NAVER Corporation とは無関係です。

## ライセンス

MIT — [LICENSE](LICENSE) を参照。
