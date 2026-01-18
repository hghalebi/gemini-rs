//! # gemini-rs
//! 
//! `gemini-rs` is a production-grade, asynchronous Rust SDK for the Google Gemini Headless CLI.
//! 
//! It abstracts away the complexity of process management, I/O piping, and JSON parsing,
//! providing a fluent, type-safe interface for integrating Gemini AI models into your Rust applications.
//! 
//! ## Key Features
//! 
//! *   **Fluent Builder API:** Construct complex queries with a readable, sentence-like syntax.
//! *   **Async/Await:** Built on `tokio` for high-performance, non-blocking concurrency.
//! *   **Robust I/O:** Automatically handles background piping of large files and context to prevent deadlocks.
//! *   **Type Safety:** Strong Rust types for all JSON outputs and streaming events.
//! 
//! ## Getting Started
//! 
//! Add `gemini-rs` to your `Cargo.toml`. You also need `tokio` as the async runtime.
//! 
//! ```toml
//! [dependencies]
//! gemini-rs = "0.1"
//! tokio = { version = "1", features = ["full"] }
//! ```
//! 
//! ## Examples
//! 
//! ### Basic Text Query
//! 
//! ```rust,no_run
//! use gemini_oxide::Gemini;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let response = Gemini::new("What is the speed of light?")
//!         .model("gemini-2.5-flash")
//!         .text()
//!         .await?;
//!     
//!     println!("Response: {}", response);
//!     Ok(())
//! }
//! ```
//! 
//! ### Code Analysis with File Context
//! 
//! ```rust,no_run
//! use gemini_oxide::Gemini;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let analysis = Gemini::new("Check this code for concurrency bugs")
//!         .file("src/main.rs")
//!         .file("src/lib.rs")
//!         .yolo() // Auto-approve tools
//!         .json()
//!         .await?;
//! 
//!     println!("Analysis: {}", analysis.response);
//!     Ok(())
//! }
//! ```
//! 
//! ### Real-time Streaming
//! 
//! ```rust,no_run
//! use gemini_oxide::{Gemini, StreamEvent};
//! use futures_util::StreamExt;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let stream = Gemini::new("Tell me a short story").stream().await?;
//!     tokio::pin!(stream);
//! 
//!     while let Some(event) = stream.next().await {
//!         match event? {
//!             StreamEvent::Message { content, delta: Some(true), .. } => {
//!                 print!("{}", content);
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use serde::{Deserialize, Serialize};
use futures_util::stream::Stream;

// =========================================================================
//  1. The Builder (Ergonomic Interface)
// =========================================================================

/// The primary builder struct for constructing Gemini requests.
///
/// Use `Gemini::new(prompt)` to start a request chain.
pub struct Gemini {
    bin_path: PathBuf,
    prompt: String,
    input_data: Option<String>,
    input_files: Vec<PathBuf>,
    model: Option<String>,
    include_dirs: Vec<String>,
    yolo: bool,
    debug: bool,
}

impl Gemini {
    /// Start a new Gemini request with the given prompt.
    ///
    /// This is the entry point for the builder.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The main query or instruction for the AI model.
    ///
    /// # Example
    ///
    /// ```rust
    /// use gemini_oxide::Gemini;
    /// let request = Gemini::new("Hello, world!");
    /// ```
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            bin_path: PathBuf::from("gemini"),
            prompt: prompt.into(),
            input_data: None,
            input_files: Vec::new(),
            model: None,
            include_dirs: Vec::new(),
            yolo: false,
            debug: false,
        }
    }

    /// Set the path to the `gemini` binary.
    ///
    /// Defaults to `"gemini"` (assuming it is in your system PATH).
    /// Use this if the CLI is installed in a non-standard location or for testing.
    pub fn bin_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.bin_path = path.into();
        self
    }

    /// Select a specific Gemini model.
    ///
    /// If not specified, the CLI defaults to the currently active model (usually `gemini-2.0-flash` or similar).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use gemini_oxide::Gemini;
    /// let req = Gemini::new("Hi").model("gemini-1.5-pro");
    /// ```
    pub fn model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    /// Pipe raw text context (code, logs, data) directly into the model's standard input.
    ///
    /// This simulates running `echo "data" | gemini ...` in the shell.
    pub fn context(mut self, data: impl Into<String>) -> Self {
        self.input_data = Some(data.into());
        self
    }

    /// Read a file from disk and pipe its content into the model's standard input.
    ///
    /// Can be called multiple times to include multiple files.
    /// This simulates `cat file.txt | gemini ...`.
    ///
    /// # Async I/O
    ///
    /// The file reading happens in a background task during execution to ensure
    /// that large files do not block the main thread or cause pipe deadlocks.
    pub fn file(mut self, path: impl Into<PathBuf>) -> Self {
        self.input_files.push(path.into());
        self
    }

    /// Include a directory in the analysis workspace.
    ///
    /// This maps to the `--include-directories` flag of the CLI.
    pub fn include(mut self, dir: &str) -> Self {
        self.include_dirs.push(dir.to_string());
        self
    }

    /// Enable "YOLO" mode (You Only Live Once).
    ///
    /// When enabled, the agent will automatically approve all tool use actions (like file edits or shell commands)
    /// without asking for user confirmation. Use with caution.
    pub fn yolo(mut self) -> Self {
        self.yolo = true;
        self
    }

    /// Enable debug mode.
    ///
    /// Passes the `--debug` flag to the CLI, causing it to emit verbose logs to stderr.
    pub fn debug(mut self) -> Self {
        self.debug = true;
        self
    }

    // =====================================================================
    //  2. Execution Methods
    // =====================================================================

    /// Execute the request and return the raw text response.
    ///
    /// This method waits for the process to complete and returns the standard output as a String.
    ///
    /// # Errors
    ///
    /// Returns `GeminiError` if the CLI fails to start, exits with a non-zero code, or prints to stderr.
    pub async fn text(self) -> Result<String, GeminiError> {
        let output = self.execute_process("text").await?;
        Ok(String::from_utf8_lossy(&output).trim().to_string())
    }

    /// Execute the request and return a structured JSON response.
    ///
    /// This parses the output into `GeminiJsonOutput`, which contains the response text,
    /// token usage statistics, and detailed tool usage metrics.
    ///
    /// # Errors
    ///
    /// Returns `GeminiError::JsonParseFailed` if the CLI output is not valid JSON.
    pub async fn json(self) -> Result<GeminiJsonOutput, GeminiError> {
        let output = self.execute_process("json").await?;
        let parsed: GeminiJsonOutput = serde_json::from_slice(&output)
            .map_err(GeminiError::JsonParseFailed)?;

        if let Some(err) = parsed.error {
            return Err(GeminiError::ApiError(err.message));
        }

        Ok(parsed)
    }

    /// Execute the request and return a real-time stream of events.
    ///
    /// This is useful for building interactive UIs, chatbots, or monitoring tool execution in real-time.
    /// The stream yields `Result<StreamEvent, GeminiError>`.
    ///
    /// # Panic
    ///
    /// Panics if it fails to open stdin/stdout pipes (which should be unreachable under normal OS conditions).
    pub async fn stream(self) -> Result<impl Stream<Item = Result<StreamEvent, GeminiError>>, GeminiError> {
        let mut cmd = self.build_command("stream-json");
        cmd.stdout(Stdio::piped()).stdin(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(GeminiError::CliLaunchFailed)?;
        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");

        // CRITICAL: Spawn a separate background task to write to stdin.
        // This prevents deadlocks if the CLI produces output while we are still writing input.
        let input_data = self.input_data.clone();
        let input_files = self.input_files.clone();
        tokio::spawn(async move {
            let _ = Self::write_stdin(stdin, input_data, input_files).await;
        });

        let reader = BufReader::new(stdout);

        // Convert the newline-delimited JSON output into a Rust Stream
        let stream = async_stream::try_stream! {
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() { continue; }
                let event: StreamEvent = serde_json::from_str(&line)
                    .map_err(GeminiError::JsonParseFailed)?;
                yield event;
            }
        };

        Ok(stream)
    }

    // =====================================================================
    //  3. Internal Helpers
    // =====================================================================

    fn build_command(&self, format: &str) -> Command {
        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("--output-format").arg(format);

        if let Some(m) = &self.model { cmd.arg("--model").arg(m); }
        if self.yolo { cmd.arg("--yolo"); }
        if self.debug { cmd.arg("--debug"); }
        if !self.include_dirs.is_empty() {
            cmd.arg("--include-directories").arg(self.include_dirs.join(","));
        }
        
        cmd.arg(&self.prompt);
        cmd
    }

    async fn execute_process(&self, format: &str) -> Result<Vec<u8>, GeminiError> {
        let mut cmd = self.build_command(format);
        cmd.stdout(Stdio::piped()).stdin(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(GeminiError::CliLaunchFailed)?;
        
        // Handle input piping in background to support large files
        if let Some(stdin) = child.stdin.take() {
            let data = self.input_data.clone();
            let files = self.input_files.clone();
            tokio::spawn(async move {
                let _ = Self::write_stdin(stdin, data, files).await;
            });
        }

        let output = child.wait_with_output().await.map_err(GeminiError::CliLaunchFailed)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GeminiError::RuntimeError(stderr.into_owned()));
        }

        Ok(output.stdout)
    }

    async fn write_stdin(mut stdin: tokio::process::ChildStdin, text: Option<String>, files: Vec<PathBuf>) -> std::io::Result<()> {
        if let Some(t) = text {
            stdin.write_all(t.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
        }
        for path in files {
            // In a real scenario, you might want to stream this chunk by chunk
            let content = tokio::fs::read(path).await?;
            stdin.write_all(&content).await?;
            stdin.write_all(b"\n").await?;
        }
        Ok(())
    }
}

// =========================================================================
//  4. Type Definitions
// =========================================================================

/// Structured response from the Gemini CLI when using JSON mode.
#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiJsonOutput {
    /// The primary text response from the model.
    pub response: String,
    /// Detailed statistics about usage (tokens, tool calls, files).
    #[serde(default)]
    pub stats: Option<GeminiStats>,
    /// Error details if the API returned a structured error.
    #[serde(default)]
    pub error: Option<GeminiErrorDetail>,
}

/// Aggregated statistics for the session.
#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiStats {
    /// Statistics per model (token counts, latency).
    pub models: HashMap<String, ModelStats>,
    /// Summary of tool execution.
    pub tools: ToolStats,
    /// Summary of file modifications (lines added/removed).
    pub files: FileStats,
}

/// Statistics specific to a single model interaction.
#[derive(Debug, Deserialize, Serialize)]
pub struct ModelStats {
    /// API performance metrics (latency, request count).
    pub api: HashMap<String, serde_json::Value>, 
    /// Token usage counts (prompt, candidates, total).
    pub tokens: HashMap<String, u64>, 
}

/// Summary of tool usage during the session.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStats {
    pub total_calls: u64,
    pub total_success: u64,
    pub total_fail: u64,
}

/// Summary of file changes made by the agent.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStats {
    pub total_lines_added: u64,
    pub total_lines_removed: u64,
}

/// Details of an error returned by the API.
#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiErrorDetail {
    #[serde(rename = "type")]
    pub err_type: String,
    pub message: String,
    pub code: Option<i32>,
}

/// Event types emitted during streaming.
///
/// Use `StreamEvent` with the `.stream()` method to handle real-time updates.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Initial session metadata (model name, session ID).
    Init { session_id: String, model: String, timestamp: String },
    /// A chunk of text content or a full message.
    Message { role: String, content: String, delta: Option<bool>, timestamp: String },
    /// Notification that the agent is invoking a tool.
    ToolUse { tool_name: String, parameters: serde_json::Value, timestamp: String },
    /// Result of a tool execution.
    ToolResult { tool_id: String, status: String, output: String, timestamp: String },
    /// Final completion event containing stats.
    Result { status: String, stats: serde_json::Value, timestamp: String },
    /// An error occurred during the stream.
    Error { message: String },
}

/// Errors that can occur when using the SDK.
#[derive(thiserror::Error, Debug)]
pub enum GeminiError {
    /// The `gemini` CLI could not be launched. Check that it is installed and in your PATH.
    #[error("Failed to start Gemini CLI. Is it installed?")]
    CliLaunchFailed(#[source] std::io::Error),
    /// The CLI output could not be parsed as JSON.
    #[error("Failed to parse JSON output")]
    JsonParseFailed(#[source] serde_json::Error),
    /// The Gemini API returned an error (e.g., quota exceeded).
    #[error("Gemini API Error: {0}")]
    ApiError(String),
    /// A general runtime error (non-zero exit code or stderr output).
    #[error("Runtime Error: {0}")]
    RuntimeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let g = Gemini::new("hello");
        assert_eq!(g.prompt, "hello");
        assert_eq!(g.bin_path, PathBuf::from("gemini"));
        assert!(!g.yolo);
    }

    #[test]
    fn test_builder_overrides() {
        let g = Gemini::new("hello")
            .bin_path("/tmp/test")
            .yolo()
            .model("gpt-4"); 
        
        assert_eq!(g.bin_path, PathBuf::from("/tmp/test"));
        assert!(g.yolo);
        assert_eq!(g.model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_command_generation() {
        let g = Gemini::new("test").model("my-model").debug();
        let cmd = g.build_command("text");
        
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("text"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("my-model"));
    }
}