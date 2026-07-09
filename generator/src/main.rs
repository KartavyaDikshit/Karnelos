use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

fn call_llm(prompt: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": "qwen2.5-coder:1.5b",
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.2
        }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .context("Failed to connect to Ollama. Is it running on localhost:11434?")?;

    if !resp.status().is_success() {
        anyhow::bail!("Ollama returned status: {}", resp.status());
    }

    let data = resp.json::<OllamaResponse>()?;
    Ok(data.response)
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: karnelos-gen \"<description of what to generate>\"");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  karnelos-gen \"build a calendar app with reminders\"");
        eprintln!("  karnelos-gen \"write a kernel module that prints the memory map\"");
        std::process::exit(1);
    }
    let prompt = &args[1];

    let system = "You are the Karnelos OS code generator. Generate Rust code for a \
        x86_64 bare-metal kernel (no_std, no_main). \
        The target hardware has: Intel Skylake-Server CPU, AVX2 SIMD, 4 cores, 4GB RAM. \
        Use only core library functions, I/O port instructions (inb/outb), \
        and direct memory access. \
        The entry point is `_start`. \
        Wrap all code in ```rust ... ``` blocks.";

    let full_prompt = format!("{}\n\nUser request: {}", system, prompt);
    println!("[generator] Requesting code from LLM...");

    match call_llm(&full_prompt) {
        Ok(response) => {
            if let Some(code) = extract_rust_code(&response) {
                let kernel_src = Path::new("../kernel/src/generated.rs");
                std::fs::write(kernel_src, &code)
                    .context("Failed to write generated.rs")?;
                println!("[generator] Code written to kernel/src/generated.rs");
                println!("[generator] Building kernel with generated code...");
                let status = Command::new("cargo")
                    .args(["bootimage", "--target", "x86_64-unknown-none"])
                    .current_dir("../kernel")
                    .status()
                    .context("Failed to run cargo bootimage")?;
                if status.success() {
                    println!("[generator] Kernel built successfully!");
                    println!("[generator] Run with: make run");
                } else {
                    eprintln!("[generator] Kernel build failed. Check for errors above.");
                }
            } else {
                println!("[generator] No Rust code block found in response.");
                println!("[generator] Raw response:\n{}", response);
            }
        }
        Err(e) => {
            eprintln!("[generator] Error: {}", e);
            eprintln!("[generator] Make sure Ollama is running: ollama serve");
        }
    }
    Ok(())
}
