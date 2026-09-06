use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

use super::GitProcessError;

pub(super) const IO_CHUNK_BYTE_LIMIT: usize = 64 * 1024;

pub(super) type GitIoCapture = Result<(Vec<u8>, Vec<u8>), ()>;
type FaultCallback = Rc<dyn Fn(GitProcessError)>;
type CompleteCallback = Rc<dyn Fn(GitIoCapture)>;

pub(super) struct GitIoConfig {
    pub(super) stdin: Vec<u8>,
    pub(super) stdout_cap: usize,
    pub(super) stderr_cap: usize,
    pub(super) cleanup: gio::Cancellable,
    #[cfg(test)]
    pub(super) inject_stdin_error: bool,
}

struct PipeBuffer {
    bytes: Vec<u8>,
    retain_limit: usize,
    overflowed: bool,
    #[cfg(test)]
    in_flight_budget: usize,
}

impl PipeBuffer {
    fn new(cap: usize) -> Self {
        let retain_limit = cap.checked_add(1).unwrap_or(0);
        Self {
            bytes: Vec::new(),
            retain_limit,
            overflowed: retain_limit == 0,
            #[cfg(test)]
            in_flight_budget: 0,
        }
    }

    fn request_size(&self) -> usize {
        if self.overflowed {
            IO_CHUNK_BYTE_LIMIT
        } else {
            self.retain_limit
                .saturating_sub(self.bytes.len())
                .clamp(1, IO_CHUNK_BYTE_LIMIT)
        }
    }
}

#[derive(Clone, Copy)]
enum PipeKind {
    Stdout,
    Stderr,
}

struct GitIoState {
    stdout: PipeBuffer,
    stderr: PipeBuffer,
    pending: u8,
    failed: bool,
    fault: FaultCallback,
    complete: Option<CompleteCallback>,
    #[cfg(test)]
    inject_stdin_error: bool,
}

pub(super) fn start(
    stdin: Option<gio::OutputStream>,
    stdout: Option<gio::InputStream>,
    stderr: Option<gio::InputStream>,
    config: GitIoConfig,
    fault: FaultCallback,
    complete: CompleteCallback,
) {
    let stdout_buffer = PipeBuffer::new(config.stdout_cap);
    let stderr_buffer = PipeBuffer::new(config.stderr_cap);
    let stdout_overflowed = stdout_buffer.overflowed;
    let stderr_overflowed = stderr_buffer.overflowed;
    let state = Rc::new(RefCell::new(GitIoState {
        stdout: stdout_buffer,
        stderr: stderr_buffer,
        pending: 3,
        failed: false,
        fault,
        complete: Some(complete),
        #[cfg(test)]
        inject_stdin_error: config.inject_stdin_error,
    }));
    if stdout_overflowed || stderr_overflowed {
        record_fault(&state, GitProcessError::OutputTooLarge);
    }
    start_input(stdin, config.stdin, &config.cleanup, Rc::clone(&state));
    start_output(stdout, PipeKind::Stdout, &config.cleanup, Rc::clone(&state));
    start_output(stderr, PipeKind::Stderr, &config.cleanup, state);
}

fn start_input(
    stream: Option<gio::OutputStream>,
    bytes: Vec<u8>,
    cleanup: &gio::Cancellable,
    state: Rc<RefCell<GitIoState>>,
) {
    let Some(stream) = stream else {
        record_fault(
            &state,
            GitProcessError::CommandFailed(String::from("Git stdin pipe is unavailable.")),
        );
        settle_part(&state);
        return;
    };
    if bytes.is_empty() {
        close_input(&stream, state);
        return;
    }
    let stream_for_callback = stream.clone();
    stream.write_all_async(
        bytes,
        glib::Priority::DEFAULT,
        Some(cleanup),
        move |result| {
            let injected = take_injected_stdin_error(&state);
            match result {
                Ok((buffer, written, error)) => {
                    if injected {
                        record_fault(
                            &state,
                            GitProcessError::CommandFailed(String::from(
                                "injected Git stdin failure",
                            )),
                        );
                    } else if let Some(error) = error {
                        record_io_error(&state, &error);
                    } else if written != buffer.len() {
                        record_fault(
                            &state,
                            GitProcessError::CommandFailed(String::from(
                                "Git stdin ended before all input was written.",
                            )),
                        );
                    }
                }
                Err((_buffer, error)) => {
                    if injected {
                        record_fault(
                            &state,
                            GitProcessError::CommandFailed(String::from(
                                "injected Git stdin failure",
                            )),
                        );
                    } else {
                        record_io_error(&state, &error);
                    }
                }
            }
            close_input(&stream_for_callback, state);
        },
    );
}

fn close_input(stream: &gio::OutputStream, state: Rc<RefCell<GitIoState>>) {
    stream.close_async(
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |result| {
            if take_injected_stdin_error(&state) {
                record_fault(
                    &state,
                    GitProcessError::CommandFailed(String::from("injected Git stdin failure")),
                );
            } else if let Err(error) = result {
                record_io_error(&state, &error);
            }
            settle_part(&state);
        },
    );
}

fn start_output(
    stream: Option<gio::InputStream>,
    kind: PipeKind,
    cleanup: &gio::Cancellable,
    state: Rc<RefCell<GitIoState>>,
) {
    let Some(stream) = stream else {
        let label = match kind {
            PipeKind::Stdout => "stdout",
            PipeKind::Stderr => "stderr",
        };
        record_fault(
            &state,
            GitProcessError::CommandFailed(format!("Git {label} pipe is unavailable.")),
        );
        settle_part(&state);
        return;
    };
    read_next(&stream, kind, cleanup, state);
}

fn read_next(
    stream: &gio::InputStream,
    kind: PipeKind,
    cleanup: &gio::Cancellable,
    state: Rc<RefCell<GitIoState>>,
) {
    let count = {
        let mut run = state.borrow_mut();
        pipe_mut(&mut run, kind).request_size()
    };
    #[cfg(test)]
    {
        let mut run = state.borrow_mut();
        pipe_mut(&mut run, kind).in_flight_budget = count;
        super::test_hooks::observe_output_peak(logical_peak(&run));
    }
    let stream_for_callback = stream.clone();
    let cleanup_for_callback = cleanup.clone();
    stream.read_bytes_async(
        count,
        glib::Priority::DEFAULT,
        Some(cleanup),
        move |result| match result {
            Ok(chunk) if chunk.is_empty() => {
                clear_in_flight(&state, kind);
                close_output(&stream_for_callback, state);
            }
            Ok(chunk) => {
                let overflowed = append_chunk(&state, kind, chunk.as_ref());
                if overflowed {
                    record_fault(&state, GitProcessError::OutputTooLarge);
                }
                clear_in_flight(&state, kind);
                drop(chunk);
                read_next(&stream_for_callback, kind, &cleanup_for_callback, state);
            }
            Err(error) => {
                clear_in_flight(&state, kind);
                record_io_error(&state, &error);
                close_output(&stream_for_callback, state);
            }
        },
    );
}

fn close_output(stream: &gio::InputStream, state: Rc<RefCell<GitIoState>>) {
    stream.close_async(
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |result| {
            if let Err(error) = result {
                record_io_error(&state, &error);
            }
            settle_part(&state);
        },
    );
}

fn append_chunk(state: &Rc<RefCell<GitIoState>>, kind: PipeKind, chunk: &[u8]) -> bool {
    let overflowed = {
        let mut run = state.borrow_mut();
        let target = pipe_mut(&mut run, kind);
        let was_overflowed = target.overflowed;
        if !was_overflowed {
            target.bytes.extend_from_slice(chunk);
            target.overflowed = target.bytes.len() >= target.retain_limit;
        }
        !was_overflowed && target.overflowed
    };
    #[cfg(test)]
    super::test_hooks::observe_output_peak(logical_peak(&state.borrow()));
    overflowed
}

#[cfg(test)]
fn clear_in_flight(state: &Rc<RefCell<GitIoState>>, kind: PipeKind) {
    pipe_mut(&mut state.borrow_mut(), kind).in_flight_budget = 0;
}

#[cfg(not(test))]
fn clear_in_flight(_state: &Rc<RefCell<GitIoState>>, _kind: PipeKind) {}

#[cfg(test)]
fn logical_peak(state: &GitIoState) -> usize {
    state
        .stdout
        .bytes
        .len()
        .saturating_add(state.stderr.bytes.len())
        .saturating_add(state.stdout.in_flight_budget)
        .saturating_add(state.stderr.in_flight_budget)
}

fn record_io_error(state: &Rc<RefCell<GitIoState>>, error: &glib::Error) {
    record_fault(
        state,
        GitProcessError::CommandFailed(error.message().to_string()),
    );
}

fn record_fault(state: &Rc<RefCell<GitIoState>>, error: GitProcessError) {
    let callback = {
        let mut run = state.borrow_mut();
        if run.failed {
            return;
        }
        run.failed = true;
        Rc::clone(&run.fault)
    };
    callback(error);
}

fn settle_part(state: &Rc<RefCell<GitIoState>>) {
    let completion = {
        let mut run = state.borrow_mut();
        run.pending = run.pending.saturating_sub(1);
        if run.pending != 0 {
            None
        } else {
            let result = if run.failed {
                Err(())
            } else {
                Ok((
                    std::mem::take(&mut run.stdout.bytes),
                    std::mem::take(&mut run.stderr.bytes),
                ))
            };
            run.complete.take().map(|callback| (callback, result))
        }
    };
    if let Some((callback, result)) = completion {
        callback(result);
    }
}

fn pipe_mut(state: &mut GitIoState, kind: PipeKind) -> &mut PipeBuffer {
    match kind {
        PipeKind::Stdout => &mut state.stdout,
        PipeKind::Stderr => &mut state.stderr,
    }
}

#[cfg(test)]
fn take_injected_stdin_error(state: &Rc<RefCell<GitIoState>>) -> bool {
    let mut run = state.borrow_mut();
    std::mem::take(&mut run.inject_stdin_error)
}

#[cfg(not(test))]
fn take_injected_stdin_error(_state: &Rc<RefCell<GitIoState>>) -> bool {
    false
}
