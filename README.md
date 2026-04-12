# LibgenMCP

A Rust MCP server for searching books through Anna's Archive via RapidAPI and downloading selected results safely to your machine.

## How to use

This section is a practical manual for getting LibgenMCP running and using it from an MCP client.

### 1. Build the server

Clone the repository and compile the binary:

```sh
git clone <your-repo-url>
cd LibgenMCP
cargo build --release
```

The compiled binary will be:

```sh
target/release/libgen-mcp
```

### 2. Get a RapidAPI key

LibgenMCP needs a RapidAPI key that has access to the Anna's Archive API.

Set it in your environment before starting the server:

```sh
export RAPIDAPI_KEY=your_api_key_here
```

You can also place it in a local `.env` file if that fits your setup better.

### 3. Configure optional defaults

You can run with only `RAPIDAPI_KEY`, but these optional variables are useful:

- `RAPIDAPI_HOST` — defaults to `annas-archive-api.p.rapidapi.com`
- `LIBGEN_MCP_DOWNLOAD_DIR` — default folder for downloaded files
- `LIBGEN_MCP_OPEN_AFTER_DOWNLOAD` — `true` or `false`
- `LIBGEN_MCP_CREATE_SUBDIRS` — `true` or `false`
- `LIBGEN_MCP_OVERWRITE_DEFAULT` — `true` or `false`
- `LIBGEN_MCP_DEFAULT_LIMIT` — default number of search results
- `LIBGEN_MCP_MAX_LIMIT` — maximum allowed search results
- `RUST_LOG` — log level such as `info` or `debug`

Example:

```env
RAPIDAPI_KEY=your_api_key_here
LIBGEN_MCP_DOWNLOAD_DIR=~/Downloads
LIBGEN_MCP_OPEN_AFTER_DOWNLOAD=false
LIBGEN_MCP_CREATE_SUBDIRS=true
LIBGEN_MCP_OVERWRITE_DEFAULT=false
RUST_LOG=info
```

### 4. Run the server

Run it with Cargo during development:

```sh
cargo run
```

Or run the compiled binary directly:

```sh
./target/release/libgen-mcp
```

The server uses stdio, so it is normally started by your MCP client instead of being used as a standalone interactive CLI.

### 5. Add it to your MCP client

Point your MCP client at the built binary and provide the required environment variables.

Example MCP client configuration:

```json
{
  "mcpServers": {
    "libgen": {
      "command": "/absolute/path/to/LibgenMCP/target/release/libgen-mcp",
      "env": {
        "RAPIDAPI_KEY": "your_api_key_here",
        "RUST_LOG": "info"
      }
    }
  }
}
```

Replace the path with the actual absolute path on your machine.

### 6. Search for books

Use the `search_books` tool first. This is the recommended workflow.

Typical search arguments:

- `query` — the search text, for example a title, author, or both
- `formats` — optional preferred formats such as `pdf` or `epub`
- `languages` — optional language filters
- `yearFrom` / `yearTo` — optional year range
- `limit` — maximum number of results to return
- `sort` — `relevance`, `year`, or `size`

A typical search request would be conceptually like:

- query: `clean code`
- formats: `["pdf", "epub"]`
- limit: `5`

The response includes ranked results with stable `result_id` values. Save the `result_id` for the result you want to download.

### 7. Download a selected book

After searching, use `download_book` with either:

- `resultId` — recommended, because it comes directly from `search_books`
- `providerItemId` — provider-native identifier

Optional download arguments include:

- `downloadDir`
- `filenameTemplate`
- `overwrite`
- `openAfterDownload`
- `createSubdirs`

Typical workflow:

1. Search with `search_books`
2. Review the returned results
3. Copy the desired `result_id`
4. Call `download_book` with that `resultId`

The response includes the final saved path, filename, bytes written, and whether the file was opened automatically.

### 8. Use the compatibility workflow if you want one-call behavior

LibgenMCP also provides `search_and_download_book`.

This workflow:

- searches first
- returns results when manual selection is needed
- or automatically downloads the top match when selection settings allow it

Useful `selectionMode` values:

- `manual` — always return choices for you to inspect
- `auto_if_confident` — auto-download only when the top match is strong enough
- `force_top` — always pick the top-ranked result

This mode is convenient, but the separate `search_books` → `download_book` flow is usually easier to control.

### 9. Recommended everyday usage

For the safest and most predictable experience, use this routine:

1. Start the MCP server through your MCP client
2. Call `search_books` with a clear query
3. Inspect title, author, year, language, format, and relevance
4. Choose the best `result_id`
5. Call `download_book`
6. Confirm the returned `destination_path`

### 10. Example real-world usage pattern

If you want an EPUB copy of a book:

1. Search for the title and author together
2. Pass `formats: ["epub"]`
3. Limit results to a small number such as `5`
4. Inspect the best matches
5. Download the result with the most accurate metadata

If you get too few results, remove format or language filters and try again.

### 11. What to expect in responses

`search_books` returns structured information such as:

- a summary
- the normalized query
- a result count
- ranked results
- warnings when relevant

`download_book` returns structured information such as:

- selected result identifiers
- title and authors
- file format and MIME type
- destination path
- final filename
- bytes written
- warnings

### 12. Troubleshooting quick guide

If the server does not start:

- confirm `RAPIDAPI_KEY` is set
- confirm your key has Anna's Archive API access
- confirm your MCP client points to the correct binary path

If searching returns nothing useful:

- broaden the query
- remove some filters
- verify the upstream API is available

If downloading fails:

- check destination directory permissions
- check available disk space
- verify the selected result is downloadable
- retry later if the upstream service is unavailable or rate-limited

### 13. Security tips

- Do not hardcode API keys in source code
- Prefer environment variables or MCP client secret storage
- Do not commit `.env` files containing real secrets

## Features

- MCP stdio server implemented in Rust
- Normalized search results with stable `result_id` values
- Separate tools for search and download
- Compatibility `search_and_download_book` workflow
- Safer path handling and filename generation
- Ranked result selection with manual and auto-selection modes
- Optional open-after-download behavior
- Structured JSON responses for MCP clients

## Requirements

- Rust 1.78+ recommended
- Cargo
- A RapidAPI key with access to the Anna's Archive API

## Installation

Clone the repository and build it:

```sh
git clone <your-repo-url>
cd LibgenMCP
cargo build --release
```

The server binary will be available at:

```sh
target/release/libgen-mcp
```

## Configuration

LibgenMCP reads configuration from environment variables and optionally from a local `.env` file.

### Required

- `RAPIDAPI_KEY` — your RapidAPI API key

### Optional

- `RAPIDAPI_HOST` — defaults to `annas-archive-api.p.rapidapi.com`
- `LIBGEN_MCP_DEFAULT_PROVIDER` — defaults to `annas_archive`
- `LIBGEN_MCP_DOWNLOAD_DIR` — default download directory
- `LIBGEN_MCP_OPEN_AFTER_DOWNLOAD` — `true` or `false`
- `LIBGEN_MCP_CREATE_SUBDIRS` — `true` or `false`
- `LIBGEN_MCP_OVERWRITE_DEFAULT` — `true` or `false`
- `LIBGEN_MCP_FILENAME_TEMPLATE` — filename template string
- `LIBGEN_MCP_MAX_FILENAME_LENGTH` — max generated filename length
- `LIBGEN_MCP_DEFAULT_LIMIT` — default search result limit
- `LIBGEN_MCP_MAX_LIMIT` — max allowed search result limit
- `LIBGEN_MCP_ENABLE_DEBUG_FIELDS` — include provider debug fields by default
- `LIBGEN_MCP_TIMEOUT_SECS` — HTTP timeout
- `LIBGEN_MCP_MAX_RETRIES` — provider retry count
- `LIBGEN_MCP_USER_AGENT` — custom HTTP user agent
- `RUST_LOG` — log level, for example `info` or `debug`

### Example `.env`

```env
RAPIDAPI_KEY=your_api_key_here
RAPIDAPI_HOST=annas-archive-api.p.rapidapi.com
LIBGEN_MCP_DOWNLOAD_DIR=~/Downloads
LIBGEN_MCP_OPEN_AFTER_DOWNLOAD=false
LIBGEN_MCP_CREATE_SUBDIRS=true
LIBGEN_MCP_OVERWRITE_DEFAULT=false
LIBGEN_MCP_DEFAULT_LIMIT=10
LIBGEN_MCP_MAX_LIMIT=50
RUST_LOG=info
```

## Running the server

Run directly with Cargo:

```sh
cargo run
```

Or run the release binary:

```sh
./target/release/libgen-mcp
```

The server communicates over stdio and is intended to be launched by an MCP client.

## MCP client configuration

Example command-based MCP configuration:

```json
{
  "mcpServers": {
    "libgen": {
      "command": "/absolute/path/to/LibgenMCP/target/release/libgen-mcp",
      "env": {
        "RAPIDAPI_KEY": "your_api_key_here",
        "RUST_LOG": "info"
      }
    }
  }
}
```

Adjust the path for your machine and MCP client.

## Available tools

### `search_books`

Searches for books and returns ranked normalized results.

#### Arguments

- `query` (required): free-text search query
- `formats`: array of preferred formats like `pdf`, `epub`, `mobi`
- `categories`: array like `fiction`, `nonfiction`
- `languages`: array of language codes or labels
- `yearFrom`: lower year bound
- `yearTo`: upper year bound
- `limit`: maximum results to return
- `provider`: provider hint, currently `annas_archive`
- `sort`: `relevance`, `year`, or `size`
- `debug`: include ranking/debug metadata

#### Returns

Structured content including:

- `summary`
- `query`
- `count`
- `results`
- `warnings`

Each result includes fields such as:

- `result_id`
- `provider`
- `provider_item_id`
- `title`
- `subtitle`
- `authors`
- `publisher`
- `year`
- `language`
- `format`
- `file_size_bytes`
- `cover_url`
- `checksum_md5`
- `checksum_sha1`
- `downloadable`
- `relevance_score`

### `download_book`

Downloads a specific result using either `resultId` or `providerItemId`.

#### Arguments

- `resultId`: stable result id returned from `search_books`
- `providerItemId`: provider-native identifier
- `downloadDir`: override destination directory
- `filenameTemplate`: override filename template
- `overwrite`: overwrite existing files
- `openAfterDownload`: open the file after download
- `createSubdirs`: create destination directory if needed

You must provide either `resultId` or `providerItemId`.

#### Returns

Structured content including:

- `summary`
- `result_id`
- `provider`
- `provider_item_id`
- `title`
- `authors`
- `format`
- `mime_type`
- `destination_path`
- `final_file_name`
- `bytes_written`
- `opened`
- `warnings`

### `search_and_download_book`

Compatibility workflow that searches first, then either:

- returns matching results if manual selection is required, or
- auto-selects a result and downloads it

#### Arguments

Includes all major search and download fields plus:

- `selectionMode`: one of
  - `manual`
  - `auto_if_confident`
  - `force_top`

## Typical workflow

### Recommended flow

1. Call `search_books`
2. Inspect results
3. Choose a `result_id`
4. Call `download_book`

### Compatibility flow

1. Call `search_and_download_book`
2. If `selection_required` is `true`, call `download_book` with a chosen `resultId`
3. Otherwise use the returned download result

## Behavior notes

- Automatic file opening is opt-in
- Downloads are written using safer filename/path handling
- Result ranking attempts to prefer better title/author/format matches
- The provider currently supported is Anna's Archive through RapidAPI
- If your API key is missing, startup will fail with a configuration error

## Development

Build:

```sh
cargo build
```

Run tests:

```sh
cargo test
```

Run with debug logging:

```sh
RUST_LOG=debug cargo run
```

Format and lint:

```sh
cargo fmt
cargo clippy --all-targets --all-features
```

## Troubleshooting

### Server fails to start

Check that:

- `RAPIDAPI_KEY` is set
- your key has access to the Anna's Archive API
- the binary path in your MCP client config is correct

### No results returned

Try:

- a broader query
- removing restrictive filters
- verifying the upstream API is reachable

### Download fails

Check:

- destination directory permissions
- available disk space
- whether the provider result is actually downloadable
- whether the upstream API is rate-limiting or unavailable

## Security notes

- Do not hardcode API keys in source files
- Prefer environment variables or MCP client-managed secrets
- Review your MCP client config so secrets are not committed to version control

## Project status

This project is a Rust rewrite focused on a cleaner MCP architecture, safer downloads, and better maintainability than the legacy implementation.