use anyhow::Context;
use ffprobe_types::FFProbeOutput;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct FFProbeTool {
    binary_path: PathBuf,
    args: Vec<String>,
}

impl FFProbeTool {
    pub fn new(binary_path: PathBuf, command: String) -> Self {
        let args = command.split(" ").map(ToString::to_string).collect();
        Self { binary_path, args }
    }

    pub async fn probe_file(&self, file_path: &Path) -> Result<FFProbeResult, FFProbeError> {
        self.probe(file_path.as_os_str(), file_path).await
    }

    /// Probes a Blu-ray's image, e.g. an `.iso`, through libbluray, which
    /// picks its longest title. Its output is empty if the image isn't a
    /// Blu-ray's.
    pub async fn probe_blu_ray_image(
        &self,
        file_path: &Path,
    ) -> Result<FFProbeResult, FFProbeError> {
        let mut input = OsString::from("bluray:");
        input.push(file_path);
        self.probe(&input, file_path).await
    }

    async fn probe(&self, input: &OsStr, file_path: &Path) -> Result<FFProbeResult, FFProbeError> {
        let cmd_output = tokio::process::Command::new(&self.binary_path)
            .args(&self.args)
            .arg(input)
            .output()
            .await
            .with_context(|| {
                format!(
                    "running {} against file: {}",
                    self.binary_path.display(),
                    file_path.display()
                )
            })?;
        let output = serde_json::from_slice(&cmd_output.stdout).with_context(|| {
            format!(
                "deserializing ffprobe output JSON, ffprobe run against file: {}",
                file_path.display()
            )
        })?;
        Ok(FFProbeResult {
            output,
            raw_stdout: cmd_output.stdout,
        })
    }
}

pub struct FFProbeResult {
    pub output: FFProbeOutput,
    pub raw_stdout: Vec<u8>,
}

/// Its message carries the whole cause chain, and it has no source, so that
/// whatever wraps it prints the chain once.
#[derive(Debug, thiserror::Error)]
#[error("ffprobe failed: {0:#}")]
pub struct FFProbeError(anyhow::Error);

impl From<anyhow::Error> for FFProbeError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}
