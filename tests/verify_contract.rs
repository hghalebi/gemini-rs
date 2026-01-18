use futures_util::StreamExt;
use gemini_oxide::{Gemini, StreamEvent};
use std::env;
use std::path::PathBuf;

fn get_mock_path() -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    current_dir.join("tests").join("mock_gemini")
}

#[tokio::test]
async fn test_json_contract_deserialization() {
    let mock_path = get_mock_path();

    let gemini = Gemini::new("test prompt").bin_path(mock_path).yolo();

    let result = gemini.json().await.expect("Failed to execute json command");

    assert_eq!(result.response, "Mock response");
    assert!(result.stats.is_some());
    let stats = result.stats.unwrap();
    assert_eq!(stats.tools.total_calls, 1);
}

#[tokio::test]
async fn test_stream_contract() {
    let mock_path = get_mock_path();

    let gemini = Gemini::new("test prompt").bin_path(mock_path);

    let stream = gemini.stream().expect("Failed to start stream");

    // Pin the stream so we can iterate over it
    let mut stream = Box::pin(stream);

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.expect("Failed to parse event"));
    }

    assert!(!events.is_empty());
    assert!(matches!(events[0], StreamEvent::Init { .. }));
    assert!(matches!(events[1], StreamEvent::Message { .. }));
    // The mock might output empty lines or slightly different order, but checking first two is good.
    // The last one should be Result.
    if let Some(last) = events.last() {
        assert!(matches!(last, StreamEvent::Result { .. }));
    }
}
