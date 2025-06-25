use anyhow::Result;
use arboard::Clipboard;
use enigo;
use enigo::KeyboardControllable;
use rdev::{EventType, Key, listen};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tokio;
use tray_icon::TrayIconBuilder;

#[derive(Debug, Deserialize, Clone)]
struct Config {
    gemini_api_key: String,
    #[serde(default = "default_use_ctrl")]
    use_ctrl: bool,
    #[serde(default = "default_use_shift")]
    use_shift: bool,
    #[serde(default = "default_use_alt")]
    use_alt: bool,
    #[serde(default = "default_trigger_key")]
    trigger_key: String,
    #[serde(default = "default_exit_use_ctrl")]
    exit_use_ctrl: bool,
    #[serde(default = "default_exit_use_shift")]
    exit_use_shift: bool,
    #[serde(default = "default_exit_use_alt")]
    exit_use_alt: bool,
    #[serde(default = "default_exit_key")]
    exit_key: String,
}

fn default_use_ctrl() -> bool {
    true
}

fn default_use_shift() -> bool {
    true
}

fn default_use_alt() -> bool {
    false
}

fn default_trigger_key() -> String {
    "P".to_string()
}

fn default_exit_use_ctrl() -> bool {
    true
}

fn default_exit_use_shift() -> bool {
    true
}

fn default_exit_use_alt() -> bool {
    false
}

fn default_exit_key() -> String {
    "Q".to_string()
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
    alt: bool,
}

fn get_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap())
}

fn read_prompt(exe_dir: &PathBuf) -> Result<String> {
    // Try working directory first
    let working_dir = std::env::current_dir()?;
    let prompt_path_working = working_dir.join("prompt.txt");

    if let Ok(content) = fs::read_to_string(&prompt_path_working) {
        println!("✅ Prompt loaded from: {}", prompt_path_working.display());
        return Ok(content);
    }

    // Fall back to executable directory
    let prompt_path_exe = exe_dir.join("prompt.txt");
    match fs::read_to_string(&prompt_path_exe) {
        Ok(content) => {
            println!("✅ Prompt loaded from: {}", prompt_path_exe.display());
            Ok(content)
        }
        Err(e) => {
            eprintln!(
                "❌ Failed to read prompt.txt from both working directory and executable directory: {}",
                e
            );
            Ok("Please process the following text:".to_string())
        }
    }
}

fn read_config(exe_dir: &PathBuf) -> Result<Config> {
    // Try working directory first
    let working_dir = std::env::current_dir()?;
    let config_path_working = working_dir.join("config.yaml");

    if let Ok(content) = fs::read_to_string(&config_path_working) {
        match serde_yaml::from_str::<Config>(&content) {
            Ok(config) => {
                println!("✅ Config loaded from: {}", config_path_working.display());
                return Ok(config);
            }
            Err(e) => {
                eprintln!("⚠️  Invalid config.yaml format in working directory: {}", e);
            }
        }
    }

    // Fall back to executable directory
    let config_path_exe = exe_dir.join("config.yaml");
    let content = fs::read_to_string(&config_path_exe).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read config.yaml from both working directory and executable directory: {}",
            e
        )
    })?;

    let config: Config = serde_yaml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid config.yaml format: {}", e))?;

    println!("✅ Config loaded from: {}", config_path_exe.display());
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
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-lite-preview-06-17:generateContent")
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

fn setup_tray(config: &Config) -> Result<tray_icon::TrayIcon> {
    let shortcut_text = build_shortcut_text(config);
    let exit_shortcut_text = build_exit_shortcut_text(config);
    
    // Create the tray icon without menu
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip(format!("Press {} to process text\nPress {} to exit", shortcut_text, exit_shortcut_text))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {}", e))?;
    
    Ok(tray_icon)
}

fn build_shortcut_text(config: &Config) -> String {
    let mut parts = Vec::new();
    if config.use_ctrl {
        parts.push("Ctrl".to_string());
    }
    if config.use_shift {
        parts.push("Shift".to_string());
    }
    if config.use_alt {
        parts.push("Alt".to_string());
    }
    parts.push(config.trigger_key.clone());
    parts.join("+")
}

fn build_exit_shortcut_text(config: &Config) -> String {
    let mut parts = Vec::new();
    if config.exit_use_ctrl {
        parts.push("Ctrl".to_string());
    }
    if config.exit_use_shift {
        parts.push("Shift".to_string());
    }
    if config.exit_use_alt {
        parts.push("Alt".to_string());
    }
    parts.push(config.exit_key.clone());
    parts.join("+")
}

fn handle_hotkey(sender: mpsc::Sender<String>, config: Config) {
    let key_state = Arc::new(Mutex::new(KeyState {
        ctrl: false,
        shift: false,
        alt: false,
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
                        Key::Alt | Key::AltGr => {
                            state.alt = true;
                        }
                        _ => {
                            // Check if this is the trigger key
                            if let Some(trigger_key) = parse_trigger_key(&config.trigger_key) {
                                if key == trigger_key {
                                    let ctrl_pressed = !config.use_ctrl || state.ctrl;
                                    let shift_pressed = !config.use_shift || state.shift;
                                    let alt_pressed = !config.use_alt || state.alt;
                                    
                                    if ctrl_pressed && shift_pressed && alt_pressed {
                                        println!("🔥 Hotkey pressed! Processing selected text...");
                                        
                                        // Get selected text from clipboard
                                        if let Ok(mut clipboard) = Clipboard::new() {
                                            if let Ok(selected_text) = clipboard.get_text() {
                                                if !selected_text.trim().is_empty() {
                                                    println!("📝 Processing text: {}", selected_text);
                                                    if let Err(e) = sender.send(selected_text) {
                                                        eprintln!(
                                                            "❌ Failed to send text to main thread: {}",
                                                            e
                                                        );
                                                    }
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
                            }
                            
                            // Check for exit shortcut
                            if let Some(exit_key) = parse_trigger_key(&config.exit_key) {
                                if key == exit_key {
                                    let ctrl_pressed = !config.exit_use_ctrl || state.ctrl;
                                    let shift_pressed = !config.exit_use_shift || state.shift;
                                    let alt_pressed = !config.exit_use_alt || state.alt;
                                    
                                    if ctrl_pressed && shift_pressed && alt_pressed {
                                        println!("👋 Exit shortcut pressed. Shutting down...");
                                        std::process::exit(0);
                                    }
                                }
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => match key {
                    Key::ControlLeft | Key::ControlRight => {
                        state.ctrl = false;
                    }
                    Key::ShiftLeft | Key::ShiftRight => {
                        state.shift = false;
                    }
                    Key::Alt | Key::AltGr => {
                        state.alt = false;
                    }
                    _ => {}
                },
                _ => {}
            }
        })
        .unwrap();
    });
}

fn parse_trigger_key(key_str: &str) -> Option<Key> {
    match key_str.to_uppercase().as_str() {
        "A" => Some(Key::KeyA),
        "B" => Some(Key::KeyB),
        "C" => Some(Key::KeyC),
        "D" => Some(Key::KeyD),
        "E" => Some(Key::KeyE),
        "F" => Some(Key::KeyF),
        "G" => Some(Key::KeyG),
        "H" => Some(Key::KeyH),
        "I" => Some(Key::KeyI),
        "J" => Some(Key::KeyJ),
        "K" => Some(Key::KeyK),
        "L" => Some(Key::KeyL),
        "M" => Some(Key::KeyM),
        "N" => Some(Key::KeyN),
        "O" => Some(Key::KeyO),
        "P" => Some(Key::KeyP),
        "Q" => Some(Key::KeyQ),
        "R" => Some(Key::KeyR),
        "S" => Some(Key::KeyS),
        "T" => Some(Key::KeyT),
        "U" => Some(Key::KeyU),
        "V" => Some(Key::KeyV),
        "W" => Some(Key::KeyW),
        "X" => Some(Key::KeyX),
        "Y" => Some(Key::KeyY),
        "Z" => Some(Key::KeyZ),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        _ => None,
    }
}

async fn process_text(prompt: String, config: Config, selected_text: String) {
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

    let _tray_icon = setup_tray(&config)?;
    println!("✅ Tray icon created successfully");

    let prompt_clone = prompt.clone();

    // Create channel for communication between hotkey thread and main async runtime
    let (sender, receiver) = mpsc::channel();

    handle_hotkey(sender, config.clone());

    println!("✅ Application started successfully!");
    let shortcut_text = build_shortcut_text(&config);
    let exit_shortcut_text = build_exit_shortcut_text(&config);
    println!("📌 Press {} to process selected text", shortcut_text);
    println!("📌 Press {} to exit the application", exit_shortcut_text);
    println!("🖥️  Check the system tray for the application icon");

    // Main loop to handle incoming text from hotkey
    loop {
        // Check for hotkey events
        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(selected_text) => {
                let prompt = prompt_clone.clone();
                let config = config.clone();

                tokio::spawn(async move {
                    process_text(prompt, config, selected_text).await;
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Continue loop
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("❌ Hotkey thread disconnected");
                break;
            }
        }
    }

    Ok(())
}
