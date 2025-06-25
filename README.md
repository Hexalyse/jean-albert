# Gemini Text Processor

A cross-platform Rust application that processes selected text using Google's Gemini AI API. The application runs in the system tray and allows you to process any selected text with a global hotkey.

## Features

- **System Tray Icon**: Runs in the background with a tray icon
- **Configurable Global Hotkey**: Customize the shortcut (default: `Ctrl+Shift+P`)
- **Gemini AI Integration**: Uses Google's Gemini 2.0 Flash-Lite model
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Automatic Text Replacement**: Replaces selected text with AI response

## Setup

### 1. Get a Gemini API Key

1. Go to [Google AI Studio](https://makersuite.google.com/app/apikey)
2. Create a new API key
3. Copy the API key

### 2. Configure the Application

1. **Edit `config.yaml`**:

   ```yaml
   # Gemini API Configuration
   gemini_api_key: "YOUR_ACTUAL_API_KEY_HERE"

   # Shortcut Configuration (optional - defaults shown)
   use_ctrl: true # Require Ctrl key (default: true)
   use_shift: true # Require Shift key (default: true)
   use_alt: false # Require Alt key (default: false)
   trigger_key: "P" # Trigger key (default: "P")

   # Examples:
   # Ctrl+Shift+P (default): use_ctrl: true, use_shift: true, use_alt: false, trigger_key: "P"
   # Ctrl+Alt+S: use_ctrl: true, use_shift: false, use_alt: true, trigger_key: "S"
   # Shift+Alt+T: use_ctrl: false, use_shift: true, use_alt: true, trigger_key: "T"
   # Just F1: use_ctrl: false, use_shift: false, use_alt: false, trigger_key: "1"
   ```

2. **Edit `prompt.txt`** (optional):
   - This file contains the base prompt that will be prepended to your selected text
   - You can customize it to give specific instructions to the AI

### 3. Build and Run

```bash
# Build the application
cargo build --release

# Run the application
cargo run --release
```

The executable will be created in `target/release/` directory.

## Usage

1. **Start the application**: Run the executable
2. **Look for the tray icon**: The application will appear in your system tray
3. **Select text**: Select any text in any application
4. **Process text**: Press your configured shortcut (default: `Ctrl+Shift+P`)
5. **Wait for response**: The AI will process your text and replace the selection

## How It Works

1. When you press your configured shortcut, the application:
   - Reads the currently selected text from the clipboard
   - Combines it with the prompt from `prompt.txt`
   - Sends the combined text to the Gemini API
   - Copies the AI response to the clipboard
   - Simulates `Ctrl+V` to paste the response

## File Structure

```
jean-albert/
├── src/
│   └── main.rs          # Main application code
├── config.yaml          # Configuration file with API key and shortcut settings
├── prompt.txt           # Base prompt for AI processing
├── Cargo.toml           # Rust dependencies
└── README.md           # This file
```

## Dependencies

- `tray-icon`: System tray functionality
- `rdev`: Global hotkey detection
- `arboard`: Cross-platform clipboard access
- `reqwest`: HTTP client for API calls
- `serde_yaml`: YAML configuration parsing
- `tokio`: Async runtime
- `enigo`: Keyboard simulation

## Troubleshooting

### Common Issues

1. **API Key Error**: Make sure your `config.yaml` contains a valid Gemini API key
2. **Hotkey Not Working**: Ensure no other application is using your configured shortcut
3. **Clipboard Issues**: Some applications may not allow clipboard access
4. **Text Not Replaced**: The automatic paste may not work in all applications

### Platform-Specific Notes

- **Windows**: May require running as administrator for global hotkeys
- **macOS**: May require accessibility permissions for keyboard simulation
- **Linux**: May require additional permissions for global hotkeys

## Development

To modify the application:

1. Edit the prompt in `prompt.txt`
2. Modify the shortcut configuration in `config.yaml`
3. Adjust AI parameters in the `GenerationConfig` struct
4. Rebuild with `cargo build --release`

## License

This project is open source. Feel free to modify and distribute.
