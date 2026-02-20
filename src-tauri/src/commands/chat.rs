use crate::state::AppState;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileChange {
    pub component: Option<String>,
    pub filename: String,
    pub description: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "thinking")]
    Thinking { content: String },
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "done")]
    Done {
        changes: Vec<FileChange>,
        explanation: Option<String>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
}

#[derive(Serialize)]
struct ClaudeStreamRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
    stream: bool,
    thinking: ThinkingConfig,
}

const SYSTEM_PROMPT: &str = r#"You are Spicy, an AI assistant for LTspice circuit schematics (.asc files).

The user's currently active .asc file content will be provided at the start of their message with line numbers (e.g. "1| Version 4"). Always use this content as context — never ask the user to paste it.

## MODES

1. **Analysis mode** — When the user asks to explain, analyze, or understand a circuit, respond in plain text. Do NOT output JSON.

2. **Edit mode** — When the user asks to modify, change, add, or remove something, respond with ONLY this JSON (no markdown, no code blocks):
{
  "edits": [
    { "start": 15, "end": 15, "replacement": "SYMATTR Value 24k" }
  ],
  "explanation": "Changed R1 from 10kΩ to 24kΩ",
  "changes": [
    { "component": "R1", "filename": "<filename>", "description": "Value 10kΩ → 24kΩ" }
  ]
}

Edit rules:
- "start"/"end" are 1-based inclusive line numbers
- "replacement" is the new text (multi-line with \n)
- Replace: { "start": 15, "end": 15, "replacement": "new content" }
- Delete: { "start": 15, "end": 17, "replacement": "" }
- Insert after line 15: { "start": 15, "end": 15, "replacement": "<original line 15>\n<new lines>" }
- Multiple edits applied bottom-up so line numbers stay correct
- No overlapping ranges

## .ASC FILE FORMAT

```
Version 4
SHEET 1 <width> <height>
WIRE x1 y1 x2 y2             — connection (always horizontal or vertical)
FLAG x y <label>              — ground (label="0") or net name ("Vcc", "OUT")
SYMBOL <type> x y <Rot>       — component placement
WINDOW <id> dx dy <align> <sz> — optional label position
SYMATTR InstName <name>       — instance name (R1, C1, L1, V1, Q1, U1, D1)
SYMATTR Value <value>         — component value (10k, 100µ, 1m, 5)
TEXT x y <align> <sz> <text>  — comment (;prefix) or SPICE directive (!prefix)
```

Ordering: WIRE lines first, then FLAGs, then SYMBOL+SYMATTR blocks, then TEXT at the end.
All coordinates are integers and multiples of 16. WIREs are always horizontal or vertical.

## ROTATIONS

Rotation codes: R0, R90, R180, R270 (normal), M0, M90, M180, M270 (mirrored).
Given a component pin at R0 offset (dx, dy) from SYMBOL origin (x, y):
- R0:   pin = (x+dx,   y+dy)      — default
- R90:  pin = (x-dy,   y+dx)      — 90° counterclockwise
- R180: pin = (x-dx,   y-dy)      — upside-down
- R270: pin = (x+dy,   y-dx)      — 90° clockwise
Mirror (M prefix) = flip horizontally first: (dx,dy)→(-dx,dy), then apply rotation.

## COMPONENT PIN REFERENCE

For SYMBOL at (x, y), pin offsets in R0 orientation:

### Resistor (res) — 2 pins, 80 units apart
R0 offsets: Pin1(+16, +16), Pin2(+16, +96)
Computed positions by rotation:
- R0:   pins at (x+16, y+16) and (x+16, y+96)    — vertical
- R90:  pins at (x-16, y+16) and (x-96, y+16)     — horizontal
- R180: pins at (x-16, y-16) and (x-16, y-96)     — vertical flipped
- R270: pins at (x+16, y-16) and (x+96, y-16)     — horizontal

### Capacitor (cap) — 2 pins, 64 units apart
R0 offsets: Pin1(+16, 0), Pin2(+16, +64)
- R0:   pins at (x+16, y) and (x+16, y+64)         — vertical
- R90:  pins at (x, y+16) and (x-64, y+16)          — horizontal
- R270: pins at (x, y-16) and (x+64, y-16)          — horizontal

### Inductor (ind) — 2 pins, 80 units apart
Same offsets as resistor: Pin1(+16, +16), Pin2(+16, +96)

### Voltage source (voltage) — 2 pins, 96 units apart
R0 offsets: Plus(0, 0), Minus(0, +96)
- R0:   plus=(x, y), minus=(x, y+96)                — vertical, + on top

### Diode (diode) — 2 pins, 64 units apart
R0 offsets: Cathode(+16, 0), Anode(+16, +64)

### NPN transistor (npn) — 3 pins
R0 offsets: Base(0, +48), Collector(+64, 0), Emitter(+64, +96)

### PNP transistor (pnp) — 3 pins
R0 offsets: Base(0, +48), Collector(+64, +96), Emitter(+64, 0)

### Op-amps — pins vary by model. Read existing WIRE endpoints in the file to find positions.

## STEP-BY-STEP RECIPES

### Change a component value
Find the SYMATTR Value line, replace it.

### Add a resistor IN SERIES (horizontal wire)
Given: WIRE x1 y x2 y (horizontal wire at height y)
1. Pick a midpoint mx between x1 and x2 (multiple of 16)
2. Compute symbol origin: (mx, y-16) for R270, giving pins at (mx+16, y-16) and (mx+96, y-16)
   — this means left pin at (mx+16, y-16)... BETTER: use R90 for left-to-right convention.
   For R90 at origin (ox, oy): left pin = (ox-96, oy+16), right pin = (ox-16, oy+16).
   So set oy = y-16, and pick ox so that left pin = some point between x1 and x2.
   Example: to center the resistor, set ox such that the midpoint of the two pins = mx.
   Midpoint of pins = (ox-96 + ox-16)/2 = ox-56. Set ox-56 = mx → ox = mx+56.
   Then left pin = mx+56-96 = mx-40, right pin = mx+56-16 = mx+40.
3. Delete the original WIRE. Add two new wires + the component:
   WIRE x1 y <left_pin_x> y
   WIRE <right_pin_x> y x2 y
   SYMBOL res <ox> <y-16> R90
   WINDOW 0 0 56 VBottom 2
   WINDOW 3 32 56 VTop 2
   SYMATTR InstName R_new
   SYMATTR Value <value>

CONCRETE EXAMPLE — insert 1k resistor in WIRE 80 96 400 96:
Midpoint mx=240, ox=240+56=296, left pin=(200,96), right pin=(280,96).
Replace the wire with:
  WIRE 80 96 200 96
  WIRE 280 96 400 96
Insert after the WIRE section:
  SYMBOL res 296 80 R90
  WINDOW 0 0 56 VBottom 2
  WINDOW 3 32 56 VTop 2
  SYMATTR InstName R2
  SYMATTR Value 1k

### Add a resistor IN SERIES (vertical wire)
Given: WIRE x y1 x y2 (vertical wire at column x)
1. Pick midpoint my between y1 and y2
2. For R0 at origin (ox, oy): top pin = (ox+16, oy+16), bottom pin = (ox+16, oy+96).
   Set ox = x-16. Set oy such that top pin y = my → oy+16 = my → oy = my-16.
   Then top pin = (x, my), bottom pin = (x, my+80).
3. Replace:
   WIRE x y1 x my
   WIRE x <my+80> x y2
   SYMBOL res <x-16> <my-16> R0
   SYMATTR InstName R_new
   SYMATTR Value <value>

CONCRETE EXAMPLE — insert 1k resistor in WIRE 200 48 200 300:
my=160, ox=184, oy=144. Top pin=(200,160), bottom pin=(200,240).
  WIRE 200 48 200 160
  WIRE 200 240 200 300
  SYMBOL res 184 144 R0
  SYMATTR InstName R2
  SYMATTR Value 1k

### Add a component IN PARALLEL
1. Find the existing component's two pin positions (from SYMBOL + rotation)
2. Place the new component offset by ~128 units in x (or y) with matching orientation
3. Add wires connecting the shared nodes

Example — C1=100n parallel to vertical R1 with pins at (200, 100) and (200, 180):
  WIRE 328 100 200 100
  WIRE 328 180 200 180
  SYMBOL cap 312 100 R0
  SYMATTR InstName C1
  SYMATTR Value 100n
(Cap at (312,100) R0: top pin = (328, 100), bottom pin = (328, 164). Adjust bottom wire to 164.)

### Remove a component
1. Delete the SYMBOL line, all following WINDOW and SYMATTR lines for that component
2. Merge the wires on both sides into one continuous wire

### Add a ground connection
  FLAG x y 0

### Add a voltage source with ground
  SYMBOL voltage x y R0
  WINDOW 123 0 0 Left 2
  WINDOW 39 0 0 Left 2
  SYMATTR InstName V1
  SYMATTR Value 10
  FLAG x <y+96> 0

### Common SPICE directives
  TEXT x y Left 2 !.tran 10m
  TEXT x y Left 2 !.ac dec 1000 10 100k
  TEXT x y Left 2 !.param Fs=16k
  TEXT x y Left 2 !.step param R 1k 10k 1k

### Value suffixes
T=1e12, G=1e9, Meg=1e6, k=1e3, m=1e-3, u=1e-6, n=1e-9, p=1e-12, f=1e-15

## CRITICAL RULES

1. ALL coordinates must be multiples of 16
2. Every component pin MUST connect to a wire endpoint or flag — no floating pins
3. Use unique InstNames: check existing names and increment (R1→R2, C1→C2)
4. WIRE lines go in the WIRE section, SYMBOL blocks go together, TEXT at the end
5. Horizontal resistors/caps/inductors need WINDOW lines:
   WINDOW 0 0 56 VBottom 2
   WINDOW 3 32 56 VTop 2
6. When inserting in series: break the wire at the pin positions, place the component in the gap
7. When space is tight, shift downstream components/wires/flags by a uniform offset
8. To find pin positions of unfamiliar components, trace the existing WIREs in the file
9. Double-check your coordinate math before outputting — wrong coordinates break the circuit
10. Keep edits minimal: only change what's necessary for the requested modification

## EDIT STRATEGY

When the user requests a circuit modification, do ALL reasoning in your thinking block:
1. PARSE — State what the user wants
2. FIND — Identify every component/wire/flag involved with line numbers and pin positions
3. PLAN — List edits needed (deletions, modifications, insertions)
4. CALCULATE — Compute coordinates for new components (all multiples of 16)

Then OUTPUT only the JSON object. Your entire visible response must be the raw JSON — nothing before it, nothing after it. No markdown, no code blocks, no step labels, no explanation text outside the JSON.

RULES: Commit to your first reasonable answer. Do not narrate your thought process in the response. Do not calculate component values (use sensible defaults). The response must start with { and end with }."#;

fn apply_edits(file_path: &std::path::Path, edits: &[serde_json::Value]) -> Result<String, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Collect edits as (start, end, replacement) and sort descending by start line
    let mut edit_ops: Vec<(usize, usize, String)> = edits
        .iter()
        .filter_map(|e| {
            let start = e["start"].as_u64()? as usize;
            let end = e["end"].as_u64()? as usize;
            let replacement = e["replacement"].as_str()?.to_string();
            Some((start, end, replacement))
        })
        .collect();
    edit_ops.sort_by(|a, b| b.0.cmp(&a.0));

    for (start, end, replacement) in edit_ops {
        if start == 0 || end == 0 || start > lines.len() || end > lines.len() || start > end {
            continue;
        }
        let start_idx = start - 1;
        let end_idx = end; // exclusive for drain/splice
        let new_lines: Vec<String> = if replacement.is_empty() {
            vec![]
        } else {
            replacement.lines().map(|l| l.to_string()).collect()
        };
        lines.splice(start_idx..end_idx, new_lines);
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    std::fs::write(file_path, &result)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(result)
}

/// Shared helper: parse JSON edit response, apply edits, send Done event.
/// Returns true if the response was handled as a JSON edit.
fn reload_ltspice() {
    std::thread::spawn(|| {
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(
                r#"tell application "System Events"
    if exists process "LTspice" then
        tell application "LTspice" to activate
        delay 0.3
        tell process "LTspice"
            click menu item "Revert to Saved" of menu "File" of menu bar 1
        end tell
    end if
end tell"#,
            )
            .output();
    });
}

fn handle_edit_response(
    json_val: &serde_json::Value,
    active_file: &Option<String>,
    dir: &str,
    on_event: &Channel<StreamEvent>,
) -> bool {
    let edits = match json_val["edits"].as_array() {
        Some(e) => e,
        None => return false,
    };

    if let Some(ref filename) = active_file {
        let file_path = std::path::Path::new(dir).join(filename);
        if let Err(e) = apply_edits(&file_path, edits) {
            let _ = on_event.send(StreamEvent::Error { message: e });
            return true;
        }
        reload_ltspice();
    }

    let explanation = json_val["explanation"]
        .as_str()
        .unwrap_or("Changes applied.")
        .to_string();

    let changes: Vec<FileChange> = if let Some(changes_arr) = json_val["changes"].as_array() {
        changes_arr
            .iter()
            .filter_map(|c| {
                Some(FileChange {
                    component: c["component"].as_str().map(|s| s.to_string()),
                    filename: c["filename"].as_str()?.to_string(),
                    description: c["description"].as_str()?.to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };

    let _ = on_event.send(StreamEvent::Done {
        changes,
        explanation: Some(explanation),
    });
    true
}

fn read_asc_file_content(dir: &str, filename: &str) -> Result<String, String> {
    let file_path = std::path::Path::new(dir).join(filename);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => Ok(content),
        Err(_utf8_err) => {
            match std::fs::read(&file_path) {
                Ok(bytes) => {
                    let content = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
                        let u16s: Vec<u16> = bytes[2..]
                            .chunks(2)
                            .filter_map(|c| {
                                if c.len() == 2 {
                                    Some(u16::from_le_bytes([c[0], c[1]]))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        String::from_utf16_lossy(&u16s)
                    } else {
                        String::from_utf8_lossy(&bytes).to_string()
                    };
                    Ok(content)
                }
                Err(io_err) => Err(format!("Failed to read file {}: {}", filename, io_err)),
            }
        }
    }
}

#[tauri::command]
pub async fn send_chat_message_stream(
    state: State<'_, AppState>,
    message: String,
    active_file: Option<String>,
    history: Vec<serde_json::Value>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let api_key = {
        let mut key = state.api_key.lock().map_err(|e| e.to_string())?;
        if key.is_empty() {
            if let Ok(env_key) = std::env::var("ANTHROPIC_API_KEY") {
                *key = env_key;
            }
        }
        key.clone()
    };

    if api_key.is_empty() {
        let _ = on_event.send(StreamEvent::Error {
            message: "ANTHROPIC_API_KEY not set. Please set it as an environment variable."
                .to_string(),
        });
        return Ok(());
    }

    let dir = state
        .working_directory
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let dir = match dir {
        Some(d) => d,
        None => {
            let _ = on_event.send(StreamEvent::Error {
                message: "No working directory set".to_string(),
            });
            return Ok(());
        }
    };

    // Build user message with file context
    let mut user_content = String::new();

    if let Some(ref filename) = active_file {
        match read_asc_file_content(&dir, filename) {
            Ok(content) => {
                let numbered: String = content
                    .lines()
                    .enumerate()
                    .map(|(i, line)| format!("{}| {}", i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n");
                user_content.push_str(&format!(
                    "Current file: {}\n\n{}\n\n",
                    filename, numbered
                ));
            }
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error { message: e });
                return Ok(());
            }
        }
    }
    user_content.push_str(&message);

    // Build message history
    let mut messages: Vec<ClaudeMessage> = Vec::new();

    for msg in &history {
        if let (Some(role), Some(content)) = (msg["role"].as_str(), msg["content"].as_str()) {
            messages.push(ClaudeMessage {
                role: role.to_string(),
                content: content.to_string(),
            });
        }
    }

    messages.push(ClaudeMessage {
        role: "user".to_string(),
        content: user_content,
    });

    let request = ClaudeStreamRequest {
        model: "claude-sonnet-4-6".to_string(),
        max_tokens: 16000,
        system: SYSTEM_PROMPT.to_string(),
        messages,
        stream: true,
        thinking: ThinkingConfig {
            thinking_type: "adaptive".to_string(),
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let _ = on_event.send(StreamEvent::Error {
            message: format!("API error ({}): {}", status, body),
        });
        return Ok(());
    }

    // Read SSE stream
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated_text = String::new();
    let mut done_sent = false;
    let mut suppress_text = false; // true when response looks like JSON edit

    // Process a single SSE data line; returns true if we should stop
    let process_line = |line: &str,
                        accumulated_text: &mut String,
                        on_event: &Channel<StreamEvent>,
                        active_file: &Option<String>,
                        dir: &str,
                        done_sent: &mut bool,
                        suppress_text: &mut bool|
     -> bool {
        let data = match line.strip_prefix("data: ") {
            Some(d) => d,
            None => return false,
        };

        if data == "[DONE]" {
            return false;
        }

        let parsed = match serde_json::from_str::<serde_json::Value>(data) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let event_type = parsed["type"].as_str().unwrap_or("");

        match event_type {
            "content_block_delta" => {
                let delta_type = parsed["delta"]["type"].as_str().unwrap_or("");
                match delta_type {
                    "thinking_delta" => {
                        if let Some(thinking) = parsed["delta"]["thinking"].as_str() {
                            let _ = on_event.send(StreamEvent::Thinking {
                                content: thinking.to_string(),
                            });
                        }
                    }
                    "text_delta" => {
                        if let Some(text) = parsed["delta"]["text"].as_str() {
                            // Detect JSON edit on first text chunk
                            if accumulated_text.is_empty() && text.trim_start().starts_with('{') {
                                *suppress_text = true;
                            }
                            accumulated_text.push_str(text);
                            if !*suppress_text {
                                let _ = on_event.send(StreamEvent::Text {
                                    content: text.to_string(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            "message_stop" => {
                // Check if accumulated text is JSON edit response
                if let Ok(json_val) =
                    serde_json::from_str::<serde_json::Value>(accumulated_text)
                {
                    if handle_edit_response(&json_val, active_file, dir, on_event) {
                        *done_sent = true;
                        return true;
                    }
                }

                // Fallback: extract JSON from mixed text (model may prefix with reasoning)
                if let Some(json_start) = accumulated_text.find("{\"edits\"") {
                    let candidate = &accumulated_text[json_start..];
                    if let Ok(json_val) =
                        serde_json::from_str::<serde_json::Value>(candidate)
                    {
                        if handle_edit_response(&json_val, active_file, dir, on_event) {
                            *done_sent = true;
                            return true;
                        }
                    }
                }

                // Analysis mode: plain text
                let _ = on_event.send(StreamEvent::Done {
                    changes: vec![],
                    explanation: None,
                });
                *done_sent = true;
            }
            "error" => {
                let error_msg = parsed["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown API error");
                let _ = on_event.send(StreamEvent::Error {
                    message: error_msg.to_string(),
                });
                *done_sent = true;
                return true;
            }
            _ => {}
        }
        false
    };

    'outer: while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error {
                    message: format!("Stream error: {}", e),
                });
                return Ok(());
            }
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if process_line(
                &line,
                &mut accumulated_text,
                &on_event,
                &active_file,
                &dir,
                &mut done_sent,
                &mut suppress_text,
            ) {
                break 'outer;
            }
        }
    }

    // Flush any remaining data in the buffer (e.g. message_stop without trailing newline)
    if !done_sent && !buffer.trim().is_empty() {
        for line in buffer.lines() {
            let line = line.trim();
            if !line.is_empty() {
                process_line(
                    line,
                    &mut accumulated_text,
                    &on_event,
                    &active_file,
                    &dir,
                    &mut done_sent,
                    &mut suppress_text,
                );
            }
        }
    }

    if !done_sent {
        // Stream ended — do final edit check on accumulated text
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&accumulated_text) {
            if handle_edit_response(&json_val, &active_file, &dir, &on_event) {
                return Ok(());
            }
        }

        // Fallback: extract JSON from mixed text
        if let Some(json_start) = accumulated_text.find("{\"edits\"") {
            let candidate = &accumulated_text[json_start..];
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(candidate) {
                if handle_edit_response(&json_val, &active_file, &dir, &on_event) {
                    return Ok(());
                }
            }
        }

        let _ = on_event.send(StreamEvent::Done {
            changes: vec![],
            explanation: None,
        });
    }

    Ok(())
}
