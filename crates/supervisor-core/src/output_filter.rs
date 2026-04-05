/// Server-side terminal output filter.
///
/// Strips sequences that are dangerous or inappropriate for an embedded terminal
/// widget (OSC window title sets, OSC clipboard writes). All other sequences —
/// SGR, cursor movement, alternate screen, bracketed paste, scroll — pass through
/// unchanged.
///
/// The filter is stateful so it correctly handles sequences that are split across
/// PTY read chunks. Any incomplete sequence at the end of a chunk is held in an
/// internal buffer and prepended to the next call.
pub struct OutputFilter {
    /// Strip OSC 0 and OSC 2 (window/icon title set). Default: true.
    pub strip_osc_window_title: bool,
    /// Strip OSC 52 (clipboard write). Default: true.
    pub strip_osc_clipboard: bool,
    state: FilterState,
    /// Bytes of an in-flight escape sequence not yet written to output.
    pending: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterState {
    Normal,
    /// Saw 0x1B (ESC); pending holds the ESC byte.
    Escape,
    /// Saw ESC ] — accumulating an OSC sequence.
    Osc,
    /// Inside an OSC sequence, saw ESC — potential ESC \ (ST) terminator.
    OscEscape,
}

/// The result of filtering one chunk.
pub struct FilterResult {
    /// Bytes to be written to the replay buffer and forwarded to clients.
    pub output: Vec<u8>,
    /// OSC sequences that were stripped, hex-encoded for diagnostic logging.
    pub stripped: Vec<String>,
}

impl OutputFilter {
    pub fn new() -> Self {
        Self {
            strip_osc_window_title: true,
            strip_osc_clipboard: true,
            state: FilterState::Normal,
            pending: Vec::new(),
        }
    }

    /// Filter `chunk` and return the sanitized output plus any stripped sequences.
    ///
    /// Incomplete escape sequences at the end of `chunk` are held in the internal
    /// buffer and will be resolved when the next chunk arrives. Call [`flush`] to
    /// emit any remaining buffered bytes (e.g., at session end).
    pub fn filter(&mut self, chunk: &[u8]) -> FilterResult {
        let mut output = Vec::with_capacity(chunk.len());
        let mut stripped = Vec::new();

        for &byte in chunk {
            match self.state {
                FilterState::Normal => {
                    if byte == 0x1B {
                        self.state = FilterState::Escape;
                        self.pending.push(byte);
                    } else {
                        output.push(byte);
                    }
                }

                FilterState::Escape => {
                    self.pending.push(byte);
                    if byte == b']' {
                        self.state = FilterState::Osc;
                    } else {
                        // Not an OSC — pass through verbatim.
                        output.extend_from_slice(&self.pending);
                        self.pending.clear();
                        self.state = FilterState::Normal;
                    }
                }

                FilterState::Osc => {
                    if byte == 0x07 {
                        // BEL string terminator.
                        self.pending.push(byte);
                        self.finish_osc(&mut output, &mut stripped);
                    } else if byte == 0x1B {
                        // Could be ESC \ (ST) terminator.
                        self.pending.push(byte);
                        self.state = FilterState::OscEscape;
                    } else {
                        self.pending.push(byte);
                    }
                }

                FilterState::OscEscape => {
                    self.pending.push(byte);
                    if byte == b'\\' {
                        // ESC \ string terminator.
                        self.finish_osc(&mut output, &mut stripped);
                    } else {
                        // Not a terminator; keep accumulating.
                        self.state = FilterState::Osc;
                    }
                }
            }
        }

        FilterResult { output, stripped }
    }

    /// Emit any buffered bytes that form an incomplete (unterminated) sequence.
    /// Call this when the session ends to avoid silently dropping output.
    pub fn flush(&mut self) -> Vec<u8> {
        let remaining = std::mem::take(&mut self.pending);
        self.state = FilterState::Normal;
        remaining
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn finish_osc(&mut self, output: &mut Vec<u8>, stripped: &mut Vec<String>) {
        let seq = std::mem::take(&mut self.pending);
        self.state = FilterState::Normal;

        if self.should_strip(&seq) {
            stripped.push(hex_encode(&seq));
        } else {
            output.extend_from_slice(&seq);
        }
    }

    fn should_strip(&self, seq: &[u8]) -> bool {
        // seq layout: ESC ] <cmd> ; <data> <ST>
        // where ST is BEL (0x07) or ESC \ (0x1B 0x5C).
        let body = match seq.strip_prefix(b"\x1b]") {
            Some(b) => b,
            None => return false,
        };

        // The command number runs from the start of body up to ';', BEL, or ESC.
        let cmd_end = body
            .iter()
            .position(|&b| b == b';' || b == 0x07 || b == 0x1B)
            .unwrap_or(body.len());

        let cmd_str = match std::str::from_utf8(&body[..cmd_end]) {
            Ok(s) => s.trim(),
            Err(_) => return false,
        };

        let cmd: u32 = match cmd_str.parse() {
            Ok(n) => n,
            Err(_) => return false,
        };

        match cmd {
            0 | 2 => self.strip_osc_window_title,
            52 => self.strip_osc_clipboard,
            _ => false,
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join("")
}
