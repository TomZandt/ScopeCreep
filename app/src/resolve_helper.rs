//! The Resolve scripting helper, as seen from the app: a small Python process (app/resolve/colour_setup_helper.py,
//! built into the app) run under Resolve's own Python, which reads the project when asked (ADR 0006).
//!
//! The app owns it: it is started on the first request, asked over its stdin, answers on its stdout, and is
//! killed when a read takes too long or the app quits. It never calls Resolve unless asked, and the Monitor asks
//! only while Resolve is not playing. Nothing here blocks the caller: answers are collected by a thread.
use crate::project::{Answer, ProjectReader};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// Resolve's bundled Python, which can import its scripting module with no setup (Resolve 21.1+).
pub const DEFAULT_PYTHON: &str = "/opt/resolve/bin/ResolvePython";
/// Overrides [`DEFAULT_PYTHON`].
pub const PYTHON_ENV: &str = "SCOPECREEP_RESOLVE_PYTHON";

const SCRIPT: &str = include_str!("../resolve/colour_setup_helper.py");

struct Running {
    child: Child,
    stdin: ChildStdin,
    answers: Receiver<Answer>,
}

pub struct ScriptHelper {
    program: PathBuf,
    envs: Vec<(OsString, OsString)>,
    running: Option<Running>,
    /// Answers made up here (the helper could not be started or stopped).
    local: VecDeque<Answer>,
}

fn made_up(result: &str, message: String) -> Answer {
    vec![
        ("result".to_string(), result.to_string()),
        ("message".to_string(), message),
    ]
}

impl ScriptHelper {
    /// A helper run by the Python interpreter at `program`.
    pub fn new(program: impl Into<PathBuf>) -> ScriptHelper {
        ScriptHelper {
            program: program.into(),
            envs: Vec::new(),
            running: None,
            local: VecDeque::new(),
        }
    }

    /// The helper under Resolve's Python: `$SCOPECREEP_RESOLVE_PYTHON`, else [`DEFAULT_PYTHON`].
    pub fn from_env() -> ScriptHelper {
        ScriptHelper::new(
            std::env::var_os(PYTHON_ENV)
                .filter(|p| !p.is_empty())
                .map_or_else(|| PathBuf::from(DEFAULT_PYTHON), PathBuf::from),
        )
    }

    /// An extra environment variable for the helper process.
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> ScriptHelper {
        self.envs.push((key.into(), value.into()));
        self
    }

    /// The helper's process id, while one is running.
    pub fn running_pid(&self) -> Option<u32> {
        self.running.as_ref().map(|r| r.child.id())
    }

    fn start(&mut self) -> Result<(), String> {
        let mut command = Command::new(&self.program);
        command
            .args(["-u", "-c", SCRIPT])
            .envs(self.envs.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // SAFETY: only async-signal-safe calls between fork and exec.
        unsafe {
            command.pre_exec(|| {
                // A helper stuck in a Resolve call must not outlive the app...
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                // ...nor hold anything the app inherited (found in testing: a lock file's descriptor). Marked
                // close-on-exec rather than closed, so the standard library's exec-error pipe still works.
                libc::close_range(3, u32::MAX, libc::CLOSE_RANGE_CLOEXEC as libc::c_int);
                Ok(())
            });
        }
        let mut child = command.spawn().map_err(|e| {
            format!(
                "Resolve's Python could not be started from {} ({e})",
                self.program.display()
            )
        })?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let (tx, answers) = channel();
        std::thread::Builder::new()
            .name("resolve-helper".into())
            .spawn(move || {
                let mut answer = Answer::new();
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if line.is_empty() {
                        let done = std::mem::take(&mut answer);
                        // The greeting a fresh helper prints is not an answer to anything.
                        let greeting = done.iter().any(|(k, v)| k == "result" && v == "started");
                        if !done.is_empty() && !greeting && tx.send(done).is_err() {
                            break;
                        }
                    } else if let Some((k, v)) = line.split_once('=') {
                        answer.push((k.to_string(), v.to_string()));
                    }
                }
            })
            .map_err(|e| format!("no thread for the scripting helper ({e})"))?;
        self.running = Some(Running {
            child,
            stdin,
            answers,
        });
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut r) = self.running.take() {
            let _ = r.child.kill();
            let _ = r.child.wait(); // after SIGKILL this returns at once
        }
    }

    fn send_read(&mut self) -> bool {
        self.running.as_mut().is_some_and(|r| {
            r.stdin
                .write_all(b"read\n")
                .and_then(|_| r.stdin.flush())
                .is_ok()
        })
    }
}

impl ProjectReader for ScriptHelper {
    fn request(&mut self) {
        for _attempt in 0..2 {
            if self.running.is_none()
                && let Err(why) = self.start()
            {
                self.local.push_back(made_up("unavailable", why));
                return;
            }
            if self.send_read() {
                return;
            }
            self.stop(); // it had exited: start a fresh one
        }
        self.local.push_back(made_up(
            "error",
            "the scripting helper would not take a request".to_string(),
        ));
    }

    fn take_answer(&mut self) -> Option<Answer> {
        if let Some(a) = self.local.pop_front() {
            return Some(a);
        }
        let r = self.running.as_mut()?;
        match r.answers.try_recv() {
            Ok(a) => Some(a),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.stop();
                Some(made_up("error", "the scripting helper stopped".to_string()))
            }
        }
    }

    fn abandon(&mut self) {
        self.stop();
    }
}

impl Drop for ScriptHelper {
    fn drop(&mut self) {
        self.stop();
    }
}
