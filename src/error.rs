#[derive(thiserror::Error, Debug)]
pub enum MinerError {
    #[error("Could not establish a connection")]
    Connection,
    #[error("Could not send command")]
    SendCommand,
    #[error("Could not receive command")]
    RecvCommand,
    #[error("Received invalid UTF-8")]
    InvalidUTF8,
    #[error("Received malformed job: `{0}`")]
    MalformedJob(String),
    #[error("unknown error")]
    Unknown,
}
