use gemini_oxide::Gemini;
use std::env;
use std::path::PathBuf;

fn get_mock_path() -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    current_dir.join("tests").join("mock_gemini")
}

#[tokio::test]
async fn test_cli_crash_handling() {
    let mock_path = get_mock_path();
    
    // "crash_it" trigger in the mock causes exit 1
    let gemini = Gemini::new("crash_it")
        .bin_path(mock_path);
        
    let result = gemini.text().await;
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    println!("DEBUG: Received error message: '{}'", msg);
    assert!(msg.contains("Runtime Error"));
    // The mock writes "Critical Failure" to stderr
    assert!(msg.contains("Critical Failure"));
}

#[tokio::test]
async fn test_malformed_json_handling() {
    let mock_path = get_mock_path();
    
    // "bad_json" trigger causes the mock to print raw text
    let gemini = Gemini::new("bad_json")
        .bin_path(mock_path);
        
    let result = gemini.json().await;
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Should be a parsing error
    assert!(err.to_string().contains("Failed to parse JSON"));
}
