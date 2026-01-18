# gemini-rs

A production-grade, fluent Rust SDK for the **Gemini Headless CLI**.

## Overview
`gemini-rs` provides a high-level, asynchronous interface to Google's Gemini models via the official CLI. It is designed with three core principles:
1.  **Fluency:** A builder pattern that reads like a natural English sentence.
2.  **Robustness:** Uses background tasks for stdin/stdout piping to prevent deadlocks when processing large contexts.
3.  **Type Safety:** Fully deserializes complex JSON outputs and streaming events into strong Rust structs.

## Usage Examples

### Simple Text Query
Perform a one-shot query and receive a raw string response.
```rust
use gemini_oxide::Gemini;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = Gemini::new("What is the capital of France?")
        .text()
        .await?;
    
    println!("{}", response); // "The capital of France is Paris."
    Ok(())
}
```

### Structured Code Review
Analyze local files with automated tool approval (YOLO mode) and structured statistics.
```rust
use gemini_oxide::Gemini;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = Gemini::new("Review this code for security vulnerabilities")
        .file("./src/lib.rs")
        .model("gemini-2.0-flash")
        .yolo()
        .json()
        .await?;

    println!("Response: {}", result.response);
    if let Some(stats) = result.stats {
        println!("Lines Added: {}", stats.files.total_lines_added);
    }
    Ok(())
}
```

### Real-time Event Streaming
Stream tokens and tool execution events in real-time.
```rust
use gemini_oxide::{Gemini, StreamEvent};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = Gemini::new("Write a Fibonacci function in Python")
        .stream()
        .await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::Message { content, delta: Some(true), .. } => {
                print!("{}", content); // Live typing effect
            }
            StreamEvent::ToolUse { tool_name, .. } => {
                println!("\n[System] Using tool: {}", tool_name);
            }
            StreamEvent::Result { .. } => {
                println!("\n[System] Generation complete.");
            }
            _ => {}
        }
    }
    Ok(())
}
```

## Definitions

### The Gemini Builder
The `Gemini` struct uses a builder pattern to configure requests.

| Method | Parameters | Description |
| :--- | :--- | :--- |
| `new(prompt)` | `impl Into<String>` | Initializes a new request with the core prompt. |
| `model(name)` | `&str` | Sets the model version (e.g., `gemini-1.5-pro`). |
| `file(path)` | `impl Into<PathBuf>` | Pipes a file's contents into the context. |
| `context(data)` | `impl Into<String>` | Pipes raw string data into the context. |
| `yolo()` | - | Automatically approves all tool actions. |
| `bin_path(path)` | `impl Into<PathBuf>` | Custom path to the `gemini` binary. |
| `debug()` | - | Enables verbose CLI output. |

### Return Types

*   **`text()`**: `Result<String, GeminiError>`
    *   Returns the trimmed text response.
*   **`json()`**: `Result<GeminiJsonOutput, GeminiError>`
    *   Returns a struct containing `response`, `stats` (model/tool/file usage), and `error` details.
*   **`stream()`**: `Result<impl Stream<Item = Result<StreamEvent, GeminiError>>, GeminiError>`
    *   An async stream of events including `Init`, `Message`, `ToolUse`, `ToolResult`, `Result`, and `Error`.

### Error Handling
The `GeminiError` enum covers:
*   `CliLaunchFailed`: CLI binary not found or failed to start.
*   `JsonParseFailed`: Output did not match expected JSON schema.
*   `ApiError`: Error message returned by the Gemini API.
*   `RuntimeError`: Non-zero exit code or stderr output from the CLI.
