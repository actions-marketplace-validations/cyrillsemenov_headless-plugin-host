use crate::docs::{tool_definitions, usage_guide_markdown, MCP_INSTRUCTIONS};
use serde_json::{json, Value};
use std::process::Command;

/// Execute `hph --json <subcommand ...>` in an isolated child process so plugin crashes/segfaults
/// or non-zero validation exits are captured cleanly without terminating the MCP server.
fn run_hph_subcommand_isolated(args: &[String]) -> (bool, String) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return (true, format!("Failed to resolve current_exe(): {e}")),
    };

    let output = Command::new(&exe).arg("--json").args(args).output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.success() {
                (false, if stdout.is_empty() { stderr } else { stdout })
            } else if !stdout.is_empty() && stdout.starts_with('{') {
                // `validate` and `fuzz` exit with code 1 when checks fail, but still print valid JSON report on stdout!
                (false, stdout)
            } else {
                let code_desc = match out.status.code() {
                    Some(c) => format!("exit code {c}"),
                    None => {
                        "terminated by signal (possible plugin crash / SIGSEGV / SIGABRT)".into()
                    }
                };
                let combined = if stderr.is_empty() {
                    stdout
                } else if stdout.is_empty() {
                    stderr
                } else {
                    format!("{stdout}\n{stderr}")
                };
                (
                    true,
                    format!("Plugin host subprocess failed ({code_desc}):\n{combined}"),
                )
            }
        }
        Err(e) => (true, format!("Failed to spawn hph subprocess: {e}")),
    }
}

pub fn dispatch_tool_call(name: &str, args: &Value) -> Value {
    if name == "hph_usage_guide" {
        let topic = args.get("topic").and_then(|v| v.as_str()).unwrap_or("all");
        let text = usage_guide_markdown(topic);
        return json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        });
    }

    if name == "hph_suite" {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("run");
        let suite_file = args
            .get("suite_file")
            .and_then(|v| v.as_str())
            .unwrap_or("suite.toml");
        let mut cli_args = vec!["suite".to_string(), action.to_string()];
        match action {
            "init" => {
                let Some(plug) = args.get("plugin").and_then(|v| v.as_str()) else {
                    return json!({
                        "content": [{ "type": "text", "text": "Missing required 'plugin' for hph_suite init" }],
                        "isError": true
                    });
                };
                cli_args.push(plug.into());
                cli_args.push("--output".into());
                cli_args.push(suite_file.into());
                if args
                    .get("capture")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cli_args.push("--capture".into());
                }
            }
            "add" => {
                cli_args.push("--file".into());
                cli_args.push(suite_file.into());
                if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
                    cli_args.push("--name".into());
                    cli_args.push(n.into());
                }
                if let Some(params) = args.get("params").and_then(|v| v.as_array()) {
                    for p in params.iter().filter_map(|x| x.as_str()) {
                        cli_args.push("--param".into());
                        cli_args.push(p.into());
                    }
                }
                if let Some(asserts) = args.get("assertions").and_then(|v| v.as_array()) {
                    for a in asserts.iter().filter_map(|x| x.as_str()) {
                        cli_args.push("--assert".into());
                        cli_args.push(a.into());
                    }
                }
                if args
                    .get("capture")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cli_args.push("--capture".into());
                }
            }
            "remove" => {
                cli_args.push("--file".into());
                cli_args.push(suite_file.into());
                if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
                    cli_args.push("--name".into());
                    cli_args.push(n.into());
                }
            }
            "set" => {
                cli_args.push("--file".into());
                cli_args.push(suite_file.into());
                if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
                    cli_args.push("--step".into());
                    cli_args.push(n.into());
                }
                if let Some(unsets) = args.get("unsets").and_then(|v| v.as_array()) {
                    for u in unsets.iter().filter_map(|x| x.as_str()) {
                        cli_args.push("--unset".into());
                        cli_args.push(u.into());
                    }
                }
                if let Some(sets) = args.get("sets").and_then(|v| v.as_array()) {
                    for s in sets.iter().filter_map(|x| x.as_str()) {
                        cli_args.push(s.into());
                    }
                }
            }
            "show" => {
                cli_args.push(suite_file.into());
            }
            _ => {
                cli_args.push(suite_file.into());
                if args
                    .get("update_baselines")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    cli_args.push("--update-baselines".into());
                }
                if let Some(n) = args.get("name").and_then(|v| v.as_str()) {
                    cli_args.push("--step".into());
                    cli_args.push(n.into());
                }
            }
        }
        let (is_err, text) = run_hph_subcommand_isolated(&cli_args);
        return json!({
            "content": [{ "type": "text", "text": text }],
            "isError": is_err
        });
    }

    let plugin = match args.get("plugin").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => {
            return json!({
                "content": [{ "type": "text", "text": "Missing required argument: 'plugin'" }],
                "isError": true
            });
        }
    };

    let mut cli_args = Vec::new();
    match name {
        "hph_inspect" => {
            cli_args.push("inspect".into());
            cli_args.push(plugin);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
        }
        "hph_validate" => {
            cli_args.push("validate".into());
            cli_args.push(plugin);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(sr) = args.get("sample_rate").and_then(|v| v.as_f64()) {
                cli_args.push("--sample-rate".into());
                cli_args.push(sr.to_string());
            }
        }
        "hph_analyze" => {
            cli_args.push("analyze".into());
            cli_args.push(plugin);
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("all");
            cli_args.push(mode.into());
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(sr) = args.get("sample_rate").and_then(|v| v.as_f64()) {
                cli_args.push("--sample-rate".into());
                cli_args.push(sr.to_string());
            }
            if let Some(freq) = args.get("freq").and_then(|v| v.as_f64()) {
                cli_args.push("--freq".into());
                cli_args.push(freq.to_string());
            }
            if let Some(tail) = args.get("tail_sec").and_then(|v| v.as_f64()) {
                cli_args.push("--tail-sec".into());
                cli_args.push(tail.to_string());
            }
            if let Some(params) = args.get("params").and_then(|v| v.as_array()) {
                for p in params {
                    if let Some(s) = p.as_str() {
                        cli_args.push("--param".into());
                        cli_args.push(s.into());
                    }
                }
            }
        }
        "hph_fuzz" => {
            cli_args.push("fuzz".into());
            cli_args.push(plugin);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(iters) = args.get("iterations").and_then(|v| v.as_u64()) {
                cli_args.push("--iterations".into());
                cli_args.push(iters.to_string());
            }
            if let Some(seed) = args.get("seed").and_then(|v| v.as_u64()) {
                cli_args.push("--seed".into());
                cli_args.push(seed.to_string());
            }
            if let Some(mb) = args.get("max_block_size").and_then(|v| v.as_u64()) {
                cli_args.push("--max-block-size".into());
                cli_args.push(mb.to_string());
            }
            if args
                .get("corrupt_state")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                cli_args.push("--corrupt-state".into());
            }
        }
        "hph_bench" => {
            cli_args.push("bench".into());
            cli_args.push(plugin);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(blocks) = args.get("blocks").and_then(|v| v.as_u64()) {
                cli_args.push("--blocks".into());
                cli_args.push(blocks.to_string());
            }
            if args.get("gui").and_then(|v| v.as_bool()).unwrap_or(false) {
                cli_args.push("--gui".into());
            }
        }
        "hph_render" => {
            cli_args.push("render".into());
            cli_args.push(plugin);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(sig) = args.get("signal").and_then(|v| v.as_str()) {
                cli_args.push("--signal".into());
                cli_args.push(sig.into());
            }
            if let Some(freq) = args.get("freq").and_then(|v| v.as_f64()) {
                cli_args.push("--freq".into());
                cli_args.push(freq.to_string());
            }
            if let Some(amp) = args.get("amp").and_then(|v| v.as_f64()) {
                cli_args.push("--amp".into());
                cli_args.push(amp.to_string());
            }
            if let Some(dur) = args.get("duration").and_then(|v| v.as_f64()) {
                cli_args.push("--duration".into());
                cli_args.push(dur.to_string());
            }
            if let Some(sr) = args.get("sample_rate").and_then(|v| v.as_u64()) {
                cli_args.push("--sample-rate".into());
                cli_args.push(sr.to_string());
            }
            if let Some(blk) = args.get("block_size").and_then(|v| v.as_u64()) {
                cli_args.push("--block-size".into());
                cli_args.push(blk.to_string());
            }
            if let Some(in_wav) = args.get("input_wav").and_then(|v| v.as_str()) {
                cli_args.push("--input-wav".into());
                cli_args.push(in_wav.into());
            }
            if let Some(out_wav) = args.get("output_wav").and_then(|v| v.as_str()) {
                cli_args.push("--output-wav".into());
                cli_args.push(out_wav.into());
            }
            if let Some(note) = args.get("note").and_then(|v| v.as_i64()) {
                cli_args.push("--note".into());
                cli_args.push(note.to_string());
            }
            if let Some(params) = args.get("params").and_then(|v| v.as_array()) {
                for p in params {
                    if let Some(s) = p.as_str() {
                        cli_args.push("--param".into());
                        cli_args.push(s.into());
                    }
                }
            }
        }
        "hph_screenshot" => {
            let output_png = match args.get("output").and_then(|v| v.as_str()) {
                Some(o) => o.to_string(),
                None => {
                    return json!({
                        "content": [{ "type": "text", "text": "Missing required argument: 'output'" }],
                        "isError": true
                    });
                }
            };
            cli_args.push("screenshot".into());
            cli_args.push(plugin);
            cli_args.push("--output".into());
            cli_args.push(output_png);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(w_ms) = args.get("warmup_ms").and_then(|v| v.as_u64()) {
                cli_args.push("--warmup-ms".into());
                cli_args.push(w_ms.to_string());
            }
            if let Some(w) = args.get("width").and_then(|v| v.as_u64()) {
                cli_args.push("--width".into());
                cli_args.push(w.to_string());
            }
            if let Some(h) = args.get("height").and_then(|v| v.as_u64()) {
                cli_args.push("--height".into());
                cli_args.push(h.to_string());
            }
        }
        "hph_datasheet" => {
            let output_html = match args.get("output").and_then(|v| v.as_str()) {
                Some(o) => o.to_string(),
                None => {
                    return json!({
                        "content": [{ "type": "text", "text": "Missing required argument: 'output'" }],
                        "isError": true
                    });
                }
            };
            cli_args.push("datasheet".into());
            cli_args.push(plugin);
            cli_args.push("--output".into());
            cli_args.push(output_html);
            if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
                cli_args.push("--id".into());
                cli_args.push(id.into());
            }
            if let Some(sr) = args.get("sample_rate").and_then(|v| v.as_f64()) {
                cli_args.push("--sample-rate".into());
                cli_args.push(sr.to_string());
            }
            if let Some(sp) = args.get("sweep_param").and_then(|v| v.as_u64()) {
                cli_args.push("--sweep-param".into());
                cli_args.push(sp.to_string());
            }
            if let Some(ref_plug) = args.get("reference").and_then(|v| v.as_str()) {
                cli_args.push("--reference".into());
                cli_args.push(ref_plug.into());
            }
            if let Some(params) = args.get("params").and_then(|v| v.as_array()) {
                for p in params {
                    if let Some(s) = p.as_str() {
                        cli_args.push("--param".into());
                        cli_args.push(s.into());
                    }
                }
            }
        }
        other => {
            return json!({
                "content": [{ "type": "text", "text": format!("Unknown tool '{other}'") }],
                "isError": true
            });
        }
    }

    let (is_err, text) = run_hph_subcommand_isolated(&cli_args);
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_err
    })
}

pub fn handle_jsonrpc_message(req: &Value) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str())?;
    let id = req.get("id").cloned();

    // Notifications have no `id` and expect no response
    if id.is_none() || method.starts_with("notifications/") {
        return None;
    }
    let id = id.unwrap();
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

    let result = match method {
        "initialize" => {
            let client_proto = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2024-11-05");
            json!({
                "protocolVersion": client_proto,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                    "prompts": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "headless-plugin-host",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": MCP_INSTRUCTIONS
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({
            "tools": tool_definitions()
        }),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            dispatch_tool_call(name, &args)
        }
        "resources/list" => json!({
            "resources": [
                {
                    "uri": "hph://docs/guide",
                    "name": "Headless Plugin Host (hph) Agent Usage & DSP Measurement Guide",
                    "description": "Complete reference for inspecting, validating, analyzing (Bode, THD+N, IMD, RT60, Delay, Dynamics, Stereo, LUFS), fuzzing, and screenshotting CLAP/VST3/AUv2 plugins.",
                    "mimeType": "text/markdown"
                },
                {
                    "uri": "hph://docs/toml-suite-schema",
                    "name": "Declarative TOML Regression Suite Schema",
                    "description": "Schema and example for declarative `.toml` audio plugin test suites.",
                    "mimeType": "text/markdown"
                }
            ]
        }),
        "resources/read" => {
            let uri = params
                .get("uri")
                .and_then(|v| v.as_str())
                .unwrap_or("hph://docs/guide");
            let text = if uri == "hph://docs/toml-suite-schema" {
                usage_guide_markdown("suite-toml")
            } else {
                usage_guide_markdown("all")
            };
            json!({
                "contents": [
                    {
                        "uri": uri,
                        "mimeType": "text/markdown",
                        "text": text
                    }
                ]
            })
        }
        "prompts/list" => json!({
            "prompts": [
                {
                    "name": "audit-plugin",
                    "description": "Run a complete 5-stage engineering audit on a compiled `.clap`, `.vst3`, or `.component` plugin (inspect, validate, analyze, fuzz, and screenshot).",
                    "arguments": [
                        {
                            "name": "plugin",
                            "description": "Path to the `.clap`, `.vst3`, or `.component` plugin bundle.",
                            "required": true
                        }
                    ]
                },
                {
                    "name": "debug-dsp",
                    "description": "Diagnose a specific DSP or real-time issue (e.g. aliasing, RT heap allocations, denormal CPU spikes, latency misreporting, or reverb feedback runaway).",
                    "arguments": [
                        {
                            "name": "plugin",
                            "description": "Path to the plugin bundle.",
                            "required": true
                        },
                        {
                            "name": "symptom",
                            "description": "Observed symptom or DSP module to verify.",
                            "required": true
                        }
                    ]
                }
            ]
        }),
        "prompts/get" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("audit-plugin");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let plugin = args
                .get("plugin")
                .and_then(|v| v.as_str())
                .unwrap_or("<PLUGIN_PATH>");
            let symptom = args
                .get("symptom")
                .and_then(|v| v.as_str())
                .unwrap_or("general DSP verification");

            let prompt_text = if name == "debug-dsp" {
                format!(
                    "Please diagnose the plugin at `{plugin}` for symptom: `{symptom}`.\n\
                    1. Call `hph_inspect` on `{plugin}` to see its parameters, ports, and latency.\n\
                    2. Call `hph_validate` on `{plugin}` to check for real-time allocations, subnormal leaks, block-size invariance, or zipper discontinuities.\n\
                    3. Call `hph_analyze` on `{plugin}` with the appropriate mode (`bode`, `thdn`, `imd`, `rt60`, `delay`, `dynamics`, or `stereo`).\n\
                    4. Call `hph_fuzz` on `{plugin}` to test stability under parameter storms and extreme inputs."
                )
            } else {
                format!(
                    "Please perform a full engineering audit of the audio plugin at `{plugin}`:\n\
                    1. Call `hph_inspect` to list all parameters, ports, extensions, and latency.\n\
                    2. Call `hph_validate` and verify all 8 validation checks pass.\n\
                    3. Call `hph_analyze` (`mode: \"all\"`) to measure Spectrum, Bode/Group-Delay, THD+N, Two-Tone IMD, RT60, Delay/Latency, Dynamics, and Stereo Correlation.\n\
                    4. Call `hph_fuzz` (`iterations: 300`, `corrupt_state: true`) to verify zero crashes, zero NaNs, and zero RT allocations.\n\
                    5. If the plugin has a GUI, call `hph_screenshot` to capture and inspect its offscreen UI render."
                )
            };

            json!({
                "description": format!("Guided workflow for {name}"),
                "messages": [
                    {
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": prompt_text
                        }
                    }
                ]
            })
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {method}")
                }
            }));
        }
    };

    Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}
