//! A minimal, typed QMP client for the `:vm` service.
//!
//! QMP runs on a socketpair the service owns, so the only commands exposed here are the ones the
//! lifecycle needs: capabilities, status, powerdown, quit, stop, cont, and a CPU census. Replies
//! are treated as untrusted input: one bounded line per message, strict JSON, strict field types,
//! and QMP `error` replies surfaced as typed errors instead of being ignored.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use crate::json::{self, JsonError, Value};

/// QMP messages are one JSON object per line; anything larger is a protocol error.
pub const MAX_MESSAGE_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmpError {
    Io(String),
    Json(JsonError),
    MessageTooLarge,
    /// A reply did not have the shape QMP guarantees for that command.
    Protocol(String),
    /// QMP answered with an `error` object.
    Server {
        class: String,
        description: String,
    },
    /// A command was sent before `qmp_capabilities` was negotiated.
    NotNegotiated,
    TimedOut {
        command: String,
    },
}

impl From<JsonError> for QmpError {
    fn from(error: JsonError) -> Self {
        QmpError::Json(error)
    }
}

impl QmpError {
    pub fn message(&self) -> String {
        match self {
            QmpError::Io(message) => format!("QMP I/O error: {message}"),
            QmpError::Json(error) => format!("QMP sent invalid JSON: {error:?}"),
            QmpError::MessageTooLarge => {
                format!("QMP message exceeded {MAX_MESSAGE_BYTES} bytes")
            }
            QmpError::Protocol(message) => format!("QMP protocol error: {message}"),
            QmpError::Server { class, description } => {
                format!("QMP error {class}: {description}")
            }
            QmpError::NotNegotiated => "QMP capabilities were not negotiated".into(),
            QmpError::TimedOut { command } => format!("QMP {command} did not answer in time"),
        }
    }
}

/// The `status` field of `query-status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    Debug,
    Inmigrate,
    InternalError,
    IoError,
    Paused,
    Postmigrate,
    Prelaunch,
    RestoreVm,
    Running,
    SaveVm,
    Shutdown,
    Suspended,
    Watchdog,
    GuestPanicked,
    FinishMigrate,
    /// A state a newer QEMU added; kept as text so nothing is silently mapped to `running`.
    Unknown(String),
}

impl RunState {
    pub fn parse(value: &str) -> Self {
        match value {
            "debug" => RunState::Debug,
            "inmigrate" => RunState::Inmigrate,
            "internal-error" => RunState::InternalError,
            "io-error" => RunState::IoError,
            "paused" => RunState::Paused,
            "postmigrate" => RunState::Postmigrate,
            "prelaunch" => RunState::Prelaunch,
            "restore-vm" => RunState::RestoreVm,
            "running" => RunState::Running,
            "save-vm" => RunState::SaveVm,
            "shutdown" => RunState::Shutdown,
            "suspended" => RunState::Suspended,
            "watchdog" => RunState::Watchdog,
            "guest-panicked" => RunState::GuestPanicked,
            "finish-migrate" => RunState::FinishMigrate,
            other => RunState::Unknown(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            RunState::Debug => "debug",
            RunState::Inmigrate => "inmigrate",
            RunState::InternalError => "internal-error",
            RunState::IoError => "io-error",
            RunState::Paused => "paused",
            RunState::Postmigrate => "postmigrate",
            RunState::Prelaunch => "prelaunch",
            RunState::RestoreVm => "restore-vm",
            RunState::Running => "running",
            RunState::SaveVm => "save-vm",
            RunState::Shutdown => "shutdown",
            RunState::Suspended => "suspended",
            RunState::Watchdog => "watchdog",
            RunState::GuestPanicked => "guest-panicked",
            RunState::FinishMigrate => "finish-migrate",
            RunState::Unknown(other) => other,
        }
    }

    /// Whether the guest's vCPU threads are executing.
    pub fn is_running(&self) -> bool {
        matches!(self, RunState::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmpStatus {
    pub running: bool,
    pub status: RunState,
}

/// One entry of `query-cpus-fast`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmpCpu {
    pub cpu_index: u64,
    pub thread_id: Option<u64>,
    /// QEMU's host thread name (`CPU 0/TCG`), used for the affinity pass.
    pub thread_name: Option<String>,
    pub target: String,
}

pub struct QmpClient<S> {
    stream: S,
    pending: Vec<u8>,
    negotiated: bool,
    greeting_seen: bool,
}

impl<S: Read + Write> QmpClient<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            pending: Vec::new(),
            negotiated: false,
            greeting_seen: false,
        }
    }

    /// Reads the `{"QMP": ...}` greeting and negotiates capabilities.
    pub fn handshake(&mut self) -> Result<(), QmpError> {
        if !self.greeting_seen {
            let greeting = self.read_object()?;
            if greeting.get("QMP").is_none() {
                return Err(QmpError::Protocol(
                    "first message was not a QMP greeting".into(),
                ));
            }
            self.greeting_seen = true;
        }
        self.execute("qmp_capabilities", None)?;
        self.negotiated = true;
        Ok(())
    }

    pub fn query_status(&mut self) -> Result<QmpStatus, QmpError> {
        let reply = self.execute("query-status", None)?;
        let state = reply
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| QmpError::Protocol("query-status returned no status".into()))?;
        let running = reply
            .get("running")
            .and_then(Value::as_bool)
            .ok_or_else(|| QmpError::Protocol("query-status returned no running flag".into()))?;
        Ok(QmpStatus {
            running,
            status: RunState::parse(state),
        })
    }

    pub fn query_cpus_fast(&mut self) -> Result<Vec<QmpCpu>, QmpError> {
        let reply = self.execute("query-cpus-fast", None)?;
        let cpus = reply
            .as_array()
            .ok_or_else(|| QmpError::Protocol("query-cpus-fast returned no array".into()))?;
        cpus.iter()
            .map(|cpu| {
                let cpu_index = cpu
                    .get("cpu-index")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| QmpError::Protocol("CPU entry has no cpu-index".into()))?;
                let target = cpu
                    .get("target")
                    .and_then(Value::as_str)
                    .unwrap_or("aarch64")
                    .to_owned();
                Ok(QmpCpu {
                    cpu_index,
                    thread_id: cpu.get("thread-id").and_then(Value::as_u64),
                    thread_name: cpu
                        .get("thread-name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    target,
                })
            })
            .collect()
    }

    pub fn system_powerdown(&mut self) -> Result<(), QmpError> {
        self.execute("system_powerdown", None).map(|_| ())
    }

    pub fn stop(&mut self) -> Result<(), QmpError> {
        self.execute("stop", None).map(|_| ())
    }

    pub fn cont(&mut self) -> Result<(), QmpError> {
        self.execute("cont", None).map(|_| ())
    }

    pub fn quit(&mut self) -> Result<(), QmpError> {
        self.execute("quit", None).map(|_| ())
    }

    /// Waits until `query-status` reports `running == false` and the guest has powered off.
    pub fn wait_for_shutdown(&mut self, timeout: Duration) -> Result<RunState, QmpError> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = self.query_status()?;
            if !status.running || status.status == RunState::Shutdown {
                return Ok(status.status);
            }
            if Instant::now() >= deadline {
                return Err(QmpError::TimedOut {
                    command: "system_powerdown".into(),
                });
            }
            // A short sleep keeps the control channel quiet without a busy loop.
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Executes one command and returns its `return` value.
    pub fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value, QmpError> {
        if !self.negotiated && command != "qmp_capabilities" {
            return Err(QmpError::NotNegotiated);
        }
        if !is_plain_command(command) {
            return Err(QmpError::Protocol(format!(
                "refusing to send command name {command:?}"
            )));
        }
        let mut request = vec![("execute".to_owned(), Value::String(command.to_owned()))];
        if let Some(arguments) = arguments {
            request.push(("arguments".to_owned(), arguments));
        }
        let line = Value::Object(request).to_json();
        self.stream
            .write_all(line.as_bytes())
            .and_then(|()| self.stream.write_all(b"\n"))
            .and_then(|()| self.stream.flush())
            .map_err(|error| QmpError::Io(error.to_string()))?;
        loop {
            let reply = self.read_object()?;
            if let Some(error) = reply.get("error") {
                let class = error
                    .get("class")
                    .and_then(Value::as_str)
                    .unwrap_or("GenericError")
                    .to_owned();
                let description = error
                    .get("desc")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                return Err(QmpError::Server { class, description });
            }
            if let Some(value) = reply.get("return") {
                return Ok(value.clone());
            }
            match reply.get("event").and_then(Value::as_str) {
                // Asynchronous events are not part of this client's contract: skip them.
                Some(_) => continue,
                None => {
                    return Err(QmpError::Protocol(
                        "reply has neither return, error nor event".into(),
                    ));
                }
            }
        }
    }

    fn read_object(&mut self) -> Result<Value, QmpError> {
        loop {
            if let Some(position) = self.pending.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.pending.drain(..=position).collect();
                let text = std::str::from_utf8(&line[..line.len() - 1])
                    .map_err(|_| QmpError::Protocol("QMP message is not UTF-8".into()))?;
                if text.trim().is_empty() {
                    continue;
                }
                return json::parse(text).map_err(QmpError::from);
            }
            if self.pending.len() > MAX_MESSAGE_BYTES {
                return Err(QmpError::MessageTooLarge);
            }
            let mut chunk = [0u8; 4096];
            let read = self
                .stream
                .read(&mut chunk)
                .map_err(|error| QmpError::Io(error.to_string()))?;
            if read == 0 {
                return Err(QmpError::Protocol(
                    "QMP stream closed before a reply arrived".into(),
                ));
            }
            self.pending.extend_from_slice(&chunk[..read]);
        }
    }
}

fn is_plain_command(command: &str) -> bool {
    !command.is_empty()
        && command.len() <= 64
        && command
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scripted {
        input: Vec<u8>,
        position: usize,
        written: Vec<u8>,
    }

    impl Scripted {
        fn new(lines: &[&str]) -> Self {
            let mut input = Vec::new();
            for line in lines {
                input.extend_from_slice(line.as_bytes());
                input.push(b'\n');
            }
            Self {
                input,
                position: 0,
                written: Vec::new(),
            }
        }
    }

    impl Read for Scripted {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let remaining = self.input.len() - self.position;
            let count = remaining.min(buffer.len());
            buffer[..count].copy_from_slice(&self.input[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    impl Write for Scripted {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.written.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    const GREETING: &str = r#"{"QMP":{"version":{"qemu":{"major":11,"minor":1,"micro":3},"package":""},"capabilities":[]}}"#;
    const CAPS_OK: &str = r#"{"return":{}}"#;

    #[test]
    fn handshake_then_query_status() {
        let mut client = QmpClient::new(Scripted::new(&[
            GREETING,
            CAPS_OK,
            r#"{"return":{"status":"running","running":true}}"#,
        ]));
        client.handshake().unwrap();
        let status = client.query_status().unwrap();
        assert!(status.running);
        assert_eq!(status.status, RunState::Running);
    }

    #[test]
    fn refuses_commands_before_negotiation() {
        let mut client = QmpClient::new(Scripted::new(&[GREETING, CAPS_OK, CAPS_OK]));
        assert_eq!(client.quit().unwrap_err(), QmpError::NotNegotiated);
        client.handshake().unwrap();
        assert_eq!(client.quit().unwrap(), ());
    }

    #[test]
    fn surfaces_qmp_errors_typed() {
        let mut client = QmpClient::new(Scripted::new(&[
            GREETING,
            CAPS_OK,
            r#"{"error":{"class":"CommandNotFound","desc":"no such command"}}"#,
        ]));
        client.handshake().unwrap();
        assert_eq!(
            client.execute("query-bogus", None).unwrap_err(),
            QmpError::Server {
                class: "CommandNotFound".into(),
                description: "no such command".into()
            }
        );
    }

    #[test]
    fn skips_events_and_rejects_malformed_replies() {
        let mut client = QmpClient::new(Scripted::new(&[
            GREETING,
            CAPS_OK,
            r#"{"event":"RESUME","data":{}}"#,
            r#"{"return":{"status":"shutdown","running":false}}"#,
        ]));
        client.handshake().unwrap();
        let status = client.query_status().unwrap();
        assert_eq!(status.status, RunState::Shutdown);

        let mut client = QmpClient::new(Scripted::new(&[GREETING, CAPS_OK, r#"{"neither":1}"#]));
        client.handshake().unwrap();
        assert!(matches!(client.query_status(), Err(QmpError::Protocol(_))));
    }

    #[test]
    fn parses_cpu_census_and_unknown_states() {
        let mut client = QmpClient::new(Scripted::new(&[
            GREETING,
            CAPS_OK,
            r#"{"return":[{"cpu-index":0,"thread-id":1234,"thread-name":"CPU 0/TCG","target":"aarch64"}]}"#,
        ]));
        client.handshake().unwrap();
        let cpus = client.query_cpus_fast().unwrap();
        assert_eq!(cpus[0].thread_name.as_deref(), Some("CPU 0/TCG"));
        assert_eq!(cpus[0].thread_id, Some(1234));
        assert_eq!(RunState::parse("some-new-state").as_str(), "some-new-state");
    }

    #[test]
    fn refuses_command_injection_in_the_command_name() {
        let mut client = QmpClient::new(Scripted::new(&[GREETING, CAPS_OK]));
        client.handshake().unwrap();
        assert!(matches!(
            client.execute("quit\\n{\"execute\":\"stop\"}", None),
            Err(QmpError::Protocol(_))
        ));
    }
}
