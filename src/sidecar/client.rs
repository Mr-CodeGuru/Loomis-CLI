use super::process::{find_python_path, find_sidecar_script};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

#[derive(Debug, Serialize)]
struct RequestPayload<'a> {
    v: u32,
    id: &'a str,
    action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ResponsePayload {
    v: u32,
    #[serde(default)]
    id: Option<String>,
    status: String,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    embedding: Option<Vec<f32>>,
    #[serde(default)]
    error: Option<String>,
}

pub struct SidecarClient {
    python_path: PathBuf,
    script_path: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    lines: Option<Lines<BufReader<ChildStdout>>>,
    req_counter: u64,
    timeout_duration: Duration,
}

impl SidecarClient {
    pub async fn new() -> Result<Self> {
        let python_path = find_python_path();
        let script_path = find_sidecar_script()?;

        let mut client = Self {
            python_path,
            script_path,
            child: None,
            stdin: None,
            lines: None,
            req_counter: 0,
            timeout_duration: Duration::from_secs(15),
        };

        client.start().await?;
        Ok(client)
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn python sidecar using '{}' and '{}'",
                self.python_path.display(),
                self.script_path.display()
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to capture child stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture child stdout")?;
        let mut lines = BufReader::new(stdout).lines();

        // Wait for handshake: {"v": 1, "status": "ready"}
        let handshake_line = timeout(Duration::from_secs(60), lines.next_line())
            .await
            .context("Timeout waiting for sidecar ready handshake (model loading took too long)")?
            .context("Failed to read handshake from sidecar")?
            .context("Sidecar exited before sending ready handshake")?;

        let resp: ResponsePayload = serde_json::from_str(&handshake_line)
            .with_context(|| format!("Invalid handshake JSON from sidecar: {handshake_line}"))?;

        if resp.status != "ready" {
            bail!("Sidecar failed to initialize: {:?}", resp.error);
        }

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.lines = Some(lines);

        Ok(())
    }

    pub async fn restart(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.stdin = None;
        self.lines = None;
        self.start().await
    }

    async fn send_request_raw(&mut self, action: &str, text: Option<&str>) -> Result<ResponsePayload> {
        if self.child.is_none() || self.stdin.is_none() || self.lines.is_none() {
            bail!("Sidecar process is not running");
        }

        self.req_counter += 1;
        let req_id = format!("req-{}", self.req_counter);
        let payload = RequestPayload {
            v: 1,
            id: &req_id,
            action,
            text,
        };

        let json_line = format!("{}\n", serde_json::to_string(&payload)?);

        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(json_line.as_bytes()).await.context("Failed to write to sidecar stdin")?;
        stdin.flush().await.context("Failed to flush sidecar stdin")?;

        let lines = self.lines.as_mut().unwrap();
        let line = timeout(self.timeout_duration, lines.next_line())
            .await
            .context("Timeout waiting for response from sidecar")?
            .context("Failed to read line from sidecar stdout")?
            .context("Sidecar closed stdout unexpectedly (EOF)")?;

        let resp: ResponsePayload = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse sidecar response as JSON: {line}"))?;

        if resp.status != "ok" {
            bail!(
                "Sidecar error: {}",
                resp.error.unwrap_or_else(|| "unknown error".to_string())
            );
        }

        Ok(resp)
    }

    async fn send_request_with_retry(&mut self, action: &str, text: Option<&str>) -> Result<ResponsePayload> {
        match self.send_request_raw(action, text).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                eprintln!("[WARN] Sidecar request failed: {e}. Restarting sidecar and retrying...");
                self.restart().await?;
                self.send_request_raw(action, text).await
            }
        }
    }

    pub async fn ping(&mut self) -> Result<()> {
        let resp = self.send_request_with_retry("ping", None).await?;
        if resp.action.as_deref() == Some("pong") {
            Ok(())
        } else {
            bail!("Unexpected ping response: {:?}", resp.action);
        }
    }

    pub async fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let resp = self.send_request_with_retry("embed", Some(text)).await?;
        resp.embedding.context("Sidecar response missing embedding vector")
    }

    pub async fn kill_raw_child_for_testing(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            child.kill().await?;
            child.wait().await?;
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.stdin = None;
        self.lines = None;
    }
}

impl Drop for SidecarClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}
