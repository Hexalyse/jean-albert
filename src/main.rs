use anyhow::Result;
use arboard::Clipboard;
use enigo;
use enigo::{Enigo, KeyboardControllable};
use rdev::{Event, EventType, Key, listen};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use tokio;
use tray_icon::{TrayIconBuilder, TrayIconEvent, menu::Menu, menu::MenuItem};

#[derive(Debug, Deserialize, Clone)]
struct Config {
    gemini_api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenerationConfig {
    temperature: f32,
    top_p: f32,
    top_k: i32,
    max_output_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: GeminiContent,
}

struct KeyState {
    ctrl: bool,
    shift: bool,
}

fn get_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap())
}

fn read_prompt(exe_dir: &PathBuf) -> Result<String> {
    let prompt_path = exe_dir.join("prompt.txt");
    match fs::read_to_string(&prompt_path) {
        Ok(content) => {
            println!("✅ Prompt loaded from: {}", prompt_path.display());
            Ok(content)
        }
        Err(e) => {
            eprintln!("❌ Failed to read prompt.txt: {}", e);
            Ok("Please process the following text:".to_string())
        }
    }
}

fn read_config(exe_dir: &PathBuf) -> Result<Config> {
    let config_path = exe_dir.join("config.yaml");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config.yaml: {}", e))?;

    let config: Config = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid config.yaml format: {}", e))?;

    println!("✅ Config loaded from: {}", config_path.display());
    Ok(config)
}

async fn call_gemini_api(api_key: &str, prompt: &str, selected_text: &str) -> Result<String> {
    let full_prompt = format!("{}\n\nSelected text: {}", prompt, selected_text);

    let request = GeminiRequest {
        contents: vec![GeminiContent {
            parts: vec![Part { text: full_prompt }],
        }],
        generation_config: GenerationConfig {
            temperature: 0.7,
            top_p: 0.8,
            top_k: 40,
            max_output_tokens: 2048,
        },
    };

    println!("🤖 Sending request to Gemini API...");

    let client = reqwest::Client::new();
    let response = client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash-latest:generateContent")
        .query(&[("key", api_key)])
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!("Gemini API error: {}", error_text));
    }

    let gemini_response: GeminiResponse = response.json().await?;

    if let Some(candidate) = gemini_response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            println!("✅ Received response from Gemini");
            return Ok(part.text.clone());
        }
    }

    Ok("No response from Gemini".to_string())
}

fn setup_tray() {
    let menu = Menu::new();
    let exit_item = MenuItem::new("Exit", true, None);
    menu.append(&exit_item);
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Gemini Text Processor\nPress Ctrl+Shift+P to process selected text")
        .build()
        .unwrap();
}

fn handle_hotkey<F: Fn(String) + Send + 'static>(callback: F) {
    let key_state = Arc::new(Mutex::new(KeyState {
        ctrl: false,
        shift: false,
    }));

    std::thread::spawn(move || {
        listen(move |event| {
            let mut state = key_state.lock().unwrap();

            match event.event_type {
                EventType::KeyPress(key) => {
                    match key {
                        Key::ControlLeft | Key::ControlRight => {
                            state.ctrl = true;
                        }
                        Key::ShiftLeft | Key::ShiftRight => {
                            state.shift = true;
                        }
                        Key::KeyP => {
                            if state.ctrl && state.shift {
                                println!("🔥 Hotkey pressed! Processing selected text...");
                                // Get selected text from clipboard
                                if let Ok(mut clipboard) = Clipboard::new() {
                                    if let Ok(selected_text) = clipboard.get_text() {
                                        if !selected_text.trim().is_empty() {
                                            callback(selected_text);
                                        } else {
                                            println!("⚠️  No text selected or clipboard is empty");
                                        }
                                    } else {
                                        println!("❌ Failed to read clipboard");
                                    }
                                } else {
                                    println!("❌ Failed to access clipboard");
                                }
                            }
                        }
                        _ => {}
                    }
                }
                EventType::KeyRelease(key) => match key {
                    Key::ControlLeft | Key::ControlRight => {
                        state.ctrl = false;
                    }
                    Key::ShiftLeft | Key::ShiftRight => {
                        state.shift = false;
                    }
                    _ => {}
                },
                _ => {}
            }
        })
        .unwrap();
    });
}

async fn process_text(prompt: String, config: Config, selected_text: String) {
    println!("📝 Processing text: {}", selected_text);

    match call_gemini_api(&config.gemini_api_key, &prompt, &selected_text).await {
        Ok(response) => {
            println!("📋 Gemini response: {}", response);

            // Copy response to clipboard
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Err(e) = clipboard.set_text(response.clone()) {
                    eprintln!("❌ Failed to set clipboard: {}", e);
                    return;
                }
                println!("✅ Response copied to clipboard");
            }

            // Simulate Ctrl+V to paste the response
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut enigo = enigo::Enigo::new();
            enigo.key_down(enigo::Key::Control);
            enigo.key_click(enigo::Key::Layout('v'));
            enigo.key_up(enigo::Key::Control);
            println!("✅ Response pasted");
        }
        Err(e) => {
            eprintln!("❌ Error calling Gemini API: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Gemini Text Processor...");

    let exe_dir = get_exe_dir();
    let prompt = read_prompt(&exe_dir)?;
    let config = read_config(&exe_dir)?;

    println!("📄 Base prompt: {}", prompt);

    setup_tray();

    let prompt_clone = prompt.clone();
    let config_clone = config.clone();

    handle_hotkey(move |selected_text| {
        let prompt = prompt_clone.clone();
        let config = config_clone.clone();

        tokio::spawn(async move {
            process_text(prompt, config, selected_text).await;
        });
    });

    println!("✅ Application started successfully!");
    println!("📌 Press Ctrl+Shift+P to process selected text");
    println!("🖥️  Check the system tray for the application icon");

    // Keep running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
