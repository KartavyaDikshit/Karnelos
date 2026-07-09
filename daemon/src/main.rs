use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

const PORT: u16 = 12345;

fn call_llm(prompt: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": "qwen2.5-coder:1.5b",
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.2 }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .context("Failed to connect to Ollama. Is it running?")?;

    if !resp.status().is_success() {
        anyhow::bail!("Ollama returned status: {}", resp.status());
    }

    Ok(resp.json::<OllamaResponse>()?.response)
}

fn extract_rust_code(response: &str) -> Option<String> {
    if let Some(start) = response.find("```rust") {
        let from = start + 7;
        if let Some(end) = response[from..].find("```") {
            return Some(response[from..from + end].trim().to_string());
        }
    }
    None
}

fn generate_and_build(prompt: &str) -> Result<String> {
    eprintln!("[daemon] Requesting LLM for: {}", prompt);

    match call_llm(prompt) {
        Ok(response) => {
            if let Some(code) = extract_rust_code(&response) {
                let kernel_src = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("kernel/src/generated.rs");
                std::fs::write(&kernel_src, &code)
                    .context("Failed to write generated.rs")?;
                eprintln!("[daemon] Code written to kernel/src/generated.rs");

                eprintln!("[daemon] Building kernel...");
                let status = Command::new("cargo")
                    .args(["bootimage", "--target", "x86_64-unknown-none"])
                    .env("BOOTLOADER_FEATURES", "map_physical_memory")
                    .current_dir(kernel_src.parent().unwrap().parent().unwrap())
                    .status()
                    .context("Failed to run cargo bootimage")?;

                if status.success() {
                    eprintln!("[daemon] Build OK");
                    Ok("BUILD_OK\n".to_string())
                } else {
                    eprintln!("[daemon] Build FAILED");
                    Ok("BUILD_FAILED\n".to_string())
                }
            } else {
                eprintln!("[daemon] No Rust code block in response");
                Ok("NO_CODE_BLOCK\n".to_string())
            }
        }
        Err(e) => {
            eprintln!("[daemon] LLM error: {}", e);
            Ok(format!("ERROR: {}\n", e))
        }
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let addr = stream.peer_addr()?;
    eprintln!("[daemon] Connection from {}", addr);

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        eprintln!("[daemon] Received: {}", trimmed);

        if let Some(prompt) = trimmed.strip_prefix("KARNELOS_GEN:") {
            let result = generate_and_build(prompt)?;
            stream.write_all(result.as_bytes())?;
            stream.flush()?;
        }
        line.clear();
    }

    eprintln!("[daemon] Connection closed");
    Ok(())
}

fn main() -> Result<()> {
    eprintln!("[daemon] Karnelos daemon starting on port {}", PORT);
    let listener = TcpListener::bind(("127.0.0.1", PORT))
        .context("Failed to bind TCP listener")?;
    eprintln!("[daemon] Listening on 127.0.0.1:{}", PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_connection(stream) {
                    eprintln!("[daemon] Connection handler error: {}", e);
                }
            }
            Err(e) => eprintln!("[daemon] Accept error: {}", e),
        }
    }

    Ok(())
}
