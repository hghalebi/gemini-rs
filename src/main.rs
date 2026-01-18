use gemini_oxide::Gemini;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let answer = Gemini::new("What is the capital of France?").text().await?;

    println!("{answer}");
    Ok(())
}
