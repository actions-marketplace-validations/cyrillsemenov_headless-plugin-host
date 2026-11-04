pub mod dispatch;
pub mod docs;

pub use dispatch::{dispatch_tool_call, handle_jsonrpc_message};
pub use docs::{tool_definitions, usage_guide_markdown, MCP_INSTRUCTIONS};

use anyhow::Result;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};

/// Run the JSON-RPC 2.0 MCP server over standard input / standard output.
/// Automatically detects and supports both newline-delimited JSON-RPC (standard MCP stdio)
/// and `Content-Length: <N>\r\n\r\n` framed JSON-RPC.
pub fn run_mcp_stdio_server() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for LSP-style `Content-Length:` framing vs standard MCP newline-delimited JSON
        if let Some(rest) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            let content_len: usize = rest.trim().parse().unwrap_or(0);
            // Consume remaining header lines until blank line
            loop {
                let mut hdr = String::new();
                if reader.read_line(&mut hdr)? == 0 || hdr.trim().is_empty() {
                    break;
                }
            }
            if content_len == 0 {
                continue;
            }
            let mut body = vec![0u8; content_len];
            reader.read_exact(&mut body)?;
            if let Ok(req) = serde_json::from_slice::<Value>(&body) {
                if let Some(resp) = handle_jsonrpc_message(&req) {
                    let payload = serde_json::to_string(&resp)?;
                    write!(
                        writer,
                        "Content-Length: {}\r\n\r\n{}",
                        payload.len(),
                        payload
                    )?;
                    writer.flush()?;
                }
            }
        } else if let Ok(req) = serde_json::from_str::<Value>(trimmed) {
            if let Some(resp) = handle_jsonrpc_message(&req) {
                let payload = serde_json::to_string(&resp)?;
                writeln!(writer, "{payload}")?;
                writer.flush()?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mcp_initialize_and_self_documenting_endpoints() {
        // 1. initialize returns instructions and capabilities
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-agent", "version": "1.0" }
            }
        });
        let init_resp = handle_jsonrpc_message(&init_req).expect("Expected initialize response");
        assert_eq!(init_resp["id"], 1);
        let instructions = init_resp["result"]["instructions"]
            .as_str()
            .expect("Expected instructions string");
        assert!(instructions.contains("hph_inspect"));
        assert!(instructions.contains("hph_validate"));
        assert!(instructions.contains("hph_analyze"));
        assert!(instructions.contains("hph_fuzz"));

        // 2. tools/list returns all 10 tools
        let tools_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });
        let tools_resp = handle_jsonrpc_message(&tools_req).expect("Expected tools/list response");
        let tools = tools_resp["result"]["tools"]
            .as_array()
            .expect("Expected tools array");
        assert_eq!(tools.len(), 10);
        assert_eq!(tools[0]["name"], "hph_usage_guide");

        // 3. tools/call hph_usage_guide returns DSP measurement handbook
        let guide_req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "hph_usage_guide",
                "arguments": { "topic": "measurements" }
            }
        });
        let guide_resp = handle_jsonrpc_message(&guide_req).expect("Expected usage_guide response");
        assert_eq!(guide_resp["result"]["isError"], false);
        let guide_txt = guide_resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(guide_txt.contains("k_weighted_lufs"));
        assert!(guide_txt.contains("Schroeder"));

        // 4. resources/list & resources/read
        let res_req = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "resources/read",
            "params": { "uri": "hph://docs/toml-suite-schema" }
        });
        let res_resp = handle_jsonrpc_message(&res_req).expect("Expected resources/read response");
        let res_txt = res_resp["result"]["contents"][0]["text"].as_str().unwrap();
        assert!(res_txt.contains("min_rt60_sec"));

        // 5. prompts/get
        let prompt_req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "prompts/get",
            "params": {
                "name": "audit-plugin",
                "arguments": { "plugin": "/tmp/Test.clap" }
            }
        });
        let prompt_resp =
            handle_jsonrpc_message(&prompt_req).expect("Expected prompts/get response");
        let msg_txt = prompt_resp["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap();
        assert!(msg_txt.contains("/tmp/Test.clap"));
    }
}
