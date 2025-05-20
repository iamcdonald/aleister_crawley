# A҉l҉e҉i҉s҉t҉e҉r҉ C҉r҉a҉w҉l҉e҉y҉
---

🕸️ Crawl from a base URL, visiting all other links found within the same domain.

🕸️ Ignore any other domains and subdomains.

🕸️ Does not dedupe based on query params or url hash.

🕸️ Retries with exponential backoff if getting page links fails.

🕸️ Multi-threaded (managed via tokio runtime a.k.a 'green threads')

🕸️ Shows progress indicator.

🕸️ Outputs tree view to stdout as shown below.

e.g

## File Output Structure

Outputs a visual tree structure starting at the initial URL provided when crawling.
```
http://example.com
├──http://example.com/one
│  ├──http://example.com/two 🔗
│  ├──http://example.com/three - 😵 401
│  ├──http://example.com/four
│  └──http://example.com ⟳
└──http://example.com/two
   ├──http://example.com/five - 😵 "problem getting content"
   └──http://example.com/six
```

**Symbol Key:**
- 🔗 ⇒ This URL has been documented elsewhere. When a URL is encountered multiple times it will only document the links form that page once and at the point it occurs closest to the base URL. This was done to avoid duplication and minimise the chance of deeply nested structures.
- ⟳ ⇒ This URL has already appeared as a parent. At any URL's second appearance in a chain this symbol is used to highlight the cyclical nature.
- 😵 => an error occurred fetching the page or page contents.

---

## How to Run

---

**Requires [Rust install](https://www.rust-lang.org/tools/install) to run**

```
> // run unit tests
> cargo test
> // compile & run
> cargo run
> // CLI Args
> cargo run --url ${base_url} --log-level ${trace|debug|info|warn|error}

```
