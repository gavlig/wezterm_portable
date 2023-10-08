// forked from wezterm/term/src/terminalstate/mod.rs commit: f4abf8fde
// MIT License

use std::io::Write;
use std::sync::mpsc::{channel, Sender};

/// This struct implements a writer that sends the data across
/// to another thread so that the write side of the terminal
/// processing never blocks.
///
/// This is important for example when processing large pastes into
/// vim.  In that scenario, we can fill up the data pending
/// on vim's input buffer, while it is busy trying to send
/// output to the terminal.  A deadlock is reached because
/// send_paste blocks on the writer, but it is unable to make
/// progress until we're able to read the output from vim.
///
/// We either need input or output to be non-blocking.
/// Output seems safest because we want to be able to exert
/// back-pressure when there is a lot of data to read,
/// and we're in control of the write side, which represents
/// input from the interactive user, or pastes.
pub struct ThreadedWriter {
    sender: Sender<WriterMessage>,
}

pub enum WriterMessage {
    Data(Vec<u8>),
    Flush,
}

impl ThreadedWriter {
    pub fn new(mut writer: Box<dyn std::io::Write + Send>) -> Self {
        let (sender, receiver) = channel::<WriterMessage>();

        std::thread::spawn(move || {
            while let Ok(msg) = receiver.recv() {
                match msg {
                    WriterMessage::Data(buf) => {
                        if writer.write(&buf).is_err() {
                            break;
                        }
                    }
                    WriterMessage::Flush => {
                        if writer.flush().is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { sender }
    }
}

impl std::io::Write for ThreadedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(WriterMessage::Data(buf.to_vec()))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sender
            .send(WriterMessage::Flush)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
        Ok(())
    }
}
