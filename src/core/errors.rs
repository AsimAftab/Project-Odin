use thiserror::Error;

#[derive(Debug, Error)]
pub enum OdinError {
    #[error("required command `{0}` was not found in PATH")]
    MissingCommand(String),

    #[error("snapshot file `{0}` does not exist; run `odin snapshot` first")]
    MissingSnapshot(String),

    #[error("command `{command}` failed with exit code {code}: {stderr}")]
    CommandFailed {
        command: String,
        code: i32,
        stderr: String,
    },
}
