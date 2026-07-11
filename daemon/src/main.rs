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

fn strip_fences(body: &str) -> &str {
    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() || !lines[0].trim().starts_with("```") {
        return body;
    }
    let mut result = String::new();
    let mut in_fence = false;
    for line in lines {
        let t = line.trim();
        if t.starts_with("```") { in_fence = !in_fence; continue; }
        if in_fence {
            if t.starts_with("fn ") || t.starts_with("pub fn") || t == "}" { continue; }
            result.push_str(line);
            result.push('\n');
        }
    }
    let trimmed = result.trim().trim_end_matches('}').trim();
    if trimmed.is_empty() { body } else { Box::leak(trimmed.to_string().into_boxed_str()) }
}

fn call_llm(prompt: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": "qwen2.5-coder:1.5b",
        "prompt": format!(
            "{}", prompt
        ),
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

fn generate_and_build(prompt: &str) -> Result<Vec<u8>> {
    let system = concat!(
        "You generate Rust code for a Karnelos OS ring-3 userspace app (x86-64, no_std).\n",
        "The app runs in ring 3 with its own page tables.\n",
        "Available API (imported via `use syscall::*`):\n",
        "  print(s: &str)  - write a string to the console\n",
        "  exit(code: u64) - exit the program\n",
        "  write_bytes(s: &[u8]) - write bytes to console\n",
        "RULES:\n",
        "- Output ONLY the Rust statements that go inside main(), one per line\n",
        "- No markdown, no code fences, no fn declaration, no closing brace\n",
        "- Use print(\"...\") for output, with \\r\\n for newlines\n",
        "- You can use alloc::vec::Vec, alloc::format!, loops, etc.\n",
        "EXAMPLE for 'print hello': print(\"Hello\\r\\n\");\n",
        "EXAMPLE for 'count to 5': for i in 1..=5u8 { print(&alloc::format!(\"count: {}\\r\\n\", i)); }\n"
    );

    let full_prompt = format!("{}\n\nRequest: {}", system, prompt);

    eprintln!("[daemon] Requesting LLM for: {}", prompt);

    match call_llm(&full_prompt) {
        Ok(response) => {
            let body = response.trim();
            let body = strip_fences(body);
            if body.len() < 3 {
                return Ok(b"ERROR: Response too short\n".to_vec());
            }

            let daemon_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
            let userspace_main = daemon_dir.parent().unwrap().join("userspace/src/main.rs");
            let existing = std::fs::read_to_string(&userspace_main)
                .context("Failed to read userspace/src/main.rs")?;

            // Replace the body between the markers
            let start_marker = "// >>> KARNELOS_BODY_START";
            let end_marker = "// >>> KARNELOS_BODY_END";
            let start_pos = existing.find(start_marker)
                .ok_or_else(|| anyhow::anyhow!("Missing KARNELOS_BODY_START marker"))?;
            let end_pos = existing.find(end_marker)
                .ok_or_else(|| anyhow::anyhow!("Missing KARNELOS_BODY_END marker"))?;
            let end_of_start = start_pos + start_marker.len();
            let new_main = format!(
                "{}\n    {}\n{}",
                &existing[..end_of_start],
                body,
                &existing[end_pos..]
            );
            std::fs::write(&userspace_main, &new_main)
                .context("Failed to write userspace/src/main.rs")?;
            eprintln!("[daemon] Code written to userspace/src/main.rs");

            eprintln!("[daemon] Building userspace app...");
            let home = std::env::var("HOME").unwrap_or_default();
            let path = format!("{}/.cargo/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", home);
            let status = Command::new("make")
                .args(["-C", "/Users/kartavyadikshit/Projects/Karnelos", "userspace"])
                .env_clear()
                .env("HOME", &home)
                .env("PATH", &path)
                .env("RUSTUP_HOME", format!("{}/.rustup", home))
                .env("CARGO_HOME", format!("{}/.cargo", home))
                .status()
                .context("Failed to run make userspace")?;

            if !status.success() {
                eprintln!("[daemon] Build FAILED");
                return Ok(b"BUILD_FAILED\n".to_vec());
            }

            eprintln!("[daemon] Build OK, reading ELF...");
            let elf_path = daemon_dir.parent().unwrap()
                .join("userspace/target/karnelos-user/debug/karnelos-user");
            let elf_data = std::fs::read(&elf_path)
                .context("Failed to read userspace ELF")?;

            if elf_data.len() > 512 * 1024 {
                eprintln!("[daemon] ELF too large: {} bytes", elf_data.len());
                return Ok(b"ERROR: ELF too large\n".to_vec());
            }

            // Format: "<size>\n<binary data>"
            let size_str = format!("{}\n", elf_data.len());
            let mut result = size_str.into_bytes();
            result.extend_from_slice(&elf_data);
            eprintln!("[daemon] Sending ELF ({} bytes)", elf_data.len());
            Ok(result)
        }
        Err(e) => {
            eprintln!("[daemon] LLM error: {}", e);
            Ok(format!("ERROR: {}\n", e).into_bytes())
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
            stream.write_all(&result)?;
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
