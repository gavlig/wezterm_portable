// forked from wezterm/mux/src/lib.rs git commit f4abf8fde
// MIT License

use anyhow::Context;
use filedescriptor::{poll, pollfd, socketpair, AsRawSocketDescriptor, FileDescriptor, POLLIN};
#[cfg(unix)]
use libc::{SOL_SOCKET, SO_RCVBUF, SO_SNDBUF};
#[cfg(windows)]
use winapi::um::winsock2::{SOL_SOCKET, SO_RCVBUF, SO_SNDBUF};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::escape::csi::{DecPrivateMode, DecPrivateModeCode, Device, Mode};
use termwiz::escape::{Action, CSI};
use crossbeam_channel::{Sender, Receiver};

pub type ActionsVec = Vec<Action>;
pub type ActionsSender = Sender<ActionsVec>;
pub type ActionsReceiver = Receiver<ActionsVec>;

const BUFSIZE: usize = 1024 * 1024;

fn parse_buffered_data(dead: &Arc<AtomicBool>, mut buf_receiver: FileDescriptor, actions_sender: ActionsSender) {
    let mut buf = vec![0; 128 * 1024]; // TODO: mux_output_parser_buffer_size
    let mut parser = termwiz::escape::parser::Parser::new();
    let mut actions = vec![];
    let mut hold = false;
    let mut action_size = 0;
    let mut delay = Duration::from_millis(3); // TODO: mux_output_parser_coalesce_delay_ms
    let mut deadline = None;

    loop {
        match buf_receiver.read(&mut buf) {
            Ok(size) if size == 0 => {
                dead.store(true, Ordering::Relaxed);
                break;
            }
            Err(_) => {
                dead.store(true, Ordering::Relaxed);
                break;
            }
            Ok(size) => {
                parser.parse(&buf[0..size], |action| {
                    let mut flush = false;
                    match &action {
                        Action::CSI(CSI::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
                            DecPrivateModeCode::SynchronizedOutput,
                        )))) => {
                            hold = true;

                            // Flush prior actions
                            if !actions.is_empty() {
                                actions_sender.send(std::mem::take(&mut actions));
                                action_size = 0;
                            }
                        }
                        Action::CSI(CSI::Mode(Mode::ResetDecPrivateMode(
                            DecPrivateMode::Code(DecPrivateModeCode::SynchronizedOutput),
                        ))) => {
                            hold = false;
                            flush = true;
                        }
                        Action::CSI(CSI::Device(dev)) if matches!(**dev, Device::SoftReset) => {
                            hold = false;
                            flush = true;
                        }
                        _ => {}
                    };
                    action.append_to(&mut actions);

                    if flush && !actions.is_empty() {
                        actions_sender.send(std::mem::take(&mut actions));
                        action_size = 0;
                    }
                });
                action_size += size;
                if !actions.is_empty() && !hold {
                    // If we haven't accumulated too much data,
                    // pause for a short while to increase the chances
                    // that we coalesce a full "frame" from an unoptimized
                    // TUI program
                    if action_size < buf.len() {
                        let poll_delay = match deadline {
                            None => {
                                deadline.replace(Instant::now() + delay);
                                Some(delay)
                            }
                            Some(target) => target.checked_duration_since(Instant::now()),
                        };
                        if poll_delay.is_some() {
                            let mut pfd = [pollfd {
                                fd: buf_receiver.as_socket_descriptor(),
                                events: POLLIN,
                                revents: 0,
                            }];
                            if let Ok(1) = poll(&mut pfd, poll_delay) {
                                // We can read now without blocking, so accumulate
                                // more data into actions
                                continue;
                            }

                            // Not readable in time: let the data we have flow into
                            // the terminal model
                        }
                    }

                    actions_sender.send(std::mem::take(&mut actions));
                    deadline = None;
                    action_size = 0;
                }

                // let config = configuration();
                buf.resize(128 * 1024, 0); // TODO: mux_output_parser_buffer_size
                delay = Duration::from_millis(3); // TODO: mux_output_parser_coalesce_delay_ms
            }
        }
    }

    // Don't forget to send anything that we might have buffered
    // to be displayed before we return from here; this is important
    // for very short lived commands so that we don't forget to
    // display what they displayed.
    if !actions.is_empty() {
        actions_sender.send(std::mem::take(&mut actions));
    }
}

fn set_socket_buffer(fd: &mut FileDescriptor, option: i32, size: usize) -> anyhow::Result<()> {
    let socklen = std::mem::size_of_val(&size);
    unsafe {
        let res = libc::setsockopt(
            fd.as_socket_descriptor(),
            SOL_SOCKET,
            option,
            &size as *const usize as *const _,
            socklen as _,
        );
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error()).context("setsockopt")
        }
    }
}

fn allocate_socketpair() -> anyhow::Result<(FileDescriptor, FileDescriptor)> {
    let (mut tx, mut rx) = socketpair().context("socketpair")?;
    set_socket_buffer(&mut tx, SO_SNDBUF, BUFSIZE).context("SO_SNDBUF")?;
    set_socket_buffer(&mut rx, SO_RCVBUF, BUFSIZE).context("SO_RCVBUF")?;
    Ok((tx, rx))
}


/// This function is run in a separate thread; its purpose is to perform
/// blocking reads from the pty (non-blocking reads are not portable to
/// all platforms and pty/tty types), parse the escape sequences and
/// relay the actions to the mux thread to apply them to the pane.
pub fn read_from_pty(
    actions_sender: ActionsSender,
    mut pty_reader: Box<dyn std::io::Read>,
) {
    let mut buf = vec![0; BUFSIZE];

    // This is used to signal that an error occurred either in this thread,
    // or in the main mux thread.  If `true`, this thread will terminate.
    let dead = Arc::new(AtomicBool::new(false));

    let (mut tx, rx) = match allocate_socketpair() {
        Ok(pair) => pair,
        Err(err) => {
            log::error!("read_from_pane_pty: Unable to allocate a socketpair: {err:#}");
            // localpane::emit_output_for_pane(
            //     pane_id,
            //     &format!(
            //         "⚠️  wezterm: read_from_pane_pty: \
            //         Unable to allocate a socketpair: {err:#}"
            //     ),
            // );
            return;
        }
    };

    std::thread::spawn({
        let dead = Arc::clone(&dead);
        move || parse_buffered_data(&dead, rx, actions_sender)
    });

    while !dead.load(Ordering::Relaxed) {
        match pty_reader.read(&mut buf) {
            Ok(size) if size == 0 => {
                log::trace!("read_pty EOF");
                break;
            }
            Err(err) => {
                log::error!("read_pty failed: {:?}", err);
                break;
            }
            Ok(size) => {
                log::trace!("read_pty read {size} bytes");

                if let Err(err) = tx.write_all(&buf[..size]) {
                    log::error!(
                        "read_pty failed to write to parser: {:?}",
                        err
                    );
                    break;
                }
            }
        }
    }

    // match exit_behavior.unwrap_or_else(|| configuration().exit_behavior) {
    //     ExitBehavior::Hold | ExitBehavior::CloseOnCleanExit => {
    //         // We don't know if we can unilaterally close
    //         // this pane right now, so don't!
    //         promise::spawn::spawn_into_main_thread(async move {
    //             let mux = Mux::get();
    //             log::trace!("checking for dead windows after EOF on pane {}", pane_id);
    //             mux.prune_dead_windows();
    //         })
    //         .detach();
    //     }
    //     ExitBehavior::Close => {
    //         promise::spawn::spawn_into_main_thread(async move {
    //             let mux = Mux::get();
    //             mux.remove_pane(pane_id);
    //         })
    //         .detach();
    //     }
    // }

    dead.store(true, Ordering::Relaxed);
}
