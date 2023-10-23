// forked from wezterm/term/src/terminalstate/keyboard.rs git commit f4abf8fde
// MIT License

use super::TerminalState;
use std::io::Write;
use termwiz::input::{KeyCode, Modifiers as KeyModifiers, KeyCodeEncodeModes, KeyboardEncoding};

impl TerminalState {
    fn effective_keyboard_encoding(&self) -> KeyboardEncoding {
        match self
            .screen()
            .keyboard_stack
            .last()
            .unwrap_or(&self.keyboard_encoding)
        {
            KeyboardEncoding::Xterm if self.config.enable_csi_u_key_encoding() => {
                KeyboardEncoding::CsiU
            }
            enc => *enc,
        }
    }

    /// Processes a key event generated by the gui/render layer
    /// that is embedding the Terminal.  This method translates the
    /// keycode into a sequence of bytes to send to the slave end
    /// of the pty via the `Write`-able object provided by the caller.
    pub fn key_up_down(
        &mut self,
        key: KeyCode,
        mods: KeyModifiers,
        is_down: bool,
    ) -> anyhow::Result<()> {
        let scroll_cached = self.vertical_scroll_offset;

        match key {
			// scroll to bottom on esc or arrow key input
            KeyCode::UpArrow | KeyCode::DownArrow | KeyCode::Escape => { self.reset_vertical_scroll() },
			// scroll with PageUp
			KeyCode::PageUp if is_down && mods.contains(KeyModifiers::SHIFT) => {
                self.vertical_scroll_offset += self.screen().physical_rows / 2;
             },
			// scroll with PageDown
			KeyCode::PageDown if is_down && mods.contains(KeyModifiers::SHIFT) => {
                self.vertical_scroll_offset = self.vertical_scroll_offset.saturating_sub(self.screen().physical_rows / 2);
            },
            _ => (),
        }

        if scroll_cached != self.vertical_scroll_offset {
            self.vertical_scroll_offset = self.vertical_scroll_offset.clamp(0, self.screen.scrollback_rows().saturating_sub(1) - self.screen.physical_rows);
        }

        let encoding = self.effective_keyboard_encoding();

        let to_send = key.encode(
            mods,
            KeyCodeEncodeModes {
                encoding,
                newline_mode: self.newline_mode,
                application_cursor_keys: self.application_cursor_keys,
                modify_other_keys: self.modify_other_keys,
            },
            is_down,
        )?;

        if to_send.is_empty() {
            return Ok(());
        }

        let label = if is_down { "key_down" } else { "key_up" };
        if self.config.debug_key_events() {
            log::info!("{}: sending {:?}, {:?} {:?}", label, to_send, key, mods);
        } else {
            log::trace!("{}: sending {:?}, {:?} {:?}", label, to_send, key, mods);
        }

        self.writer.write_all(to_send.as_bytes())?;
        self.writer.flush()?;

        Ok(())
    }

    pub fn key_up(&mut self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        self.key_up_down(key, mods, false)
    }

    pub fn key_down(&mut self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        self.key_up_down(key, mods, true)
    }
}
