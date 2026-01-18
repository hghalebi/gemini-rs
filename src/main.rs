use futures::stream::{FuturesUnordered, StreamExt};
use gemini_oxide::Gemini;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // --- Pattern 1: tokio::join! (Static Concurrency) ---
    // Perfect for running a fixed set of heterogeneous tasks.
    println!("--- Pattern 1: Static Concurrency (join!) ---");
    let task_a = Gemini::new("Explain quantum entanglement in one sentence").text();
    let task_b = Gemini::new("Explain general relativity in one sentence").text();

    let (res_a, res_b) = tokio::join!(task_a, task_b);
    println!("A: {}", res_a?);
    println!("B: {}", res_b?);

    // --- Pattern 2: FuturesUnordered (Dynamic Concurrency) ---
    // Perfect for processing a list of items where order doesn't matter
    // and you want to maximize throughput.
    println!("\n--- Pattern 2: Dynamic Concurrency (FuturesUnordered) ---");
    let prompts = vec![
        "What is 2+2?",
        "What is the color of the sky?",
        "Who wrote Rust?",
        "What is the speed of sound?",
    ];

    let mut futures = FuturesUnordered::new();

    for prompt in prompts {
        futures.push(async move {
            let res = Gemini::new(prompt).text().await;
            (prompt, res)
        });
    }

    while let Some((prompt, result)) = futures.next().await {
        match result {
            Ok(text) => println!("Prompt: '{}' -> Answer: {}", prompt, text),
            Err(e) => eprintln!("Error for '{}': {}", prompt, e),
        }
    }

    println!("\nTotal time elapsed: {:?}", start.elapsed());
    Ok(())
}
