// Flick - buffer.rs
// Per PRD §8.1: Per-window text buffer management
// Single global buffer, max 5000 characters, with reset conditions.

use std::sync::{Arc, Mutex};

const MAX_BUFFER_SIZE: usize = 5000;

#[derive(Debug, Clone)]
pub struct TextBuffer {
    inner: Arc<Mutex<BufferInner>>,
}

#[derive(Debug)]
struct BufferInner {
    data: String,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BufferInner {
                data: String::with_capacity(256),
            })),
        }
    }

    /// Append a character to the buffer, enforcing the max size cap.
    pub fn push_char(&self, c: char) {
        let mut inner = self.inner.lock().unwrap();
        if inner.data.len() >= MAX_BUFFER_SIZE {
            // Drop oldest characters to make room
            let drain_count = inner.data.len() - MAX_BUFFER_SIZE + 1;
            let mut char_boundary = 0;
            let mut count = 0;
            for (i, _) in inner.data.char_indices() {
                if count >= drain_count {
                    char_boundary = i;
                    break;
                }
                count += 1;
            }
            if count < drain_count {
                inner.data.clear();
            } else {
                inner.data = inner.data[char_boundary..].to_string();
            }
        }
        inner.data.push(c);
    }

    /// Remove the last character (Backspace).
    pub fn pop_char(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.data.pop();
    }

    /// Clear the entire buffer (reset conditions: Enter, Tab, Escape, Arrow, mouse click).
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.data.clear();
    }

    /// Get the last `n` characters for trigger matching.
    pub fn get_tail(&self, n: usize) -> String {
        let inner = self.inner.lock().unwrap();
        let chars: Vec<char> = inner.data.chars().collect();
        if chars.len() <= n {
            inner.data.clone()
        } else {
            chars[chars.len() - n..].iter().collect()
        }
    }

    /// Get the full buffer content.
    pub fn get_full(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner.data.clone()
    }

    /// Get the full buffer content, then clear it.
    pub fn take(&self) -> String {
        let mut inner = self.inner.lock().unwrap();
        let data = inner.data.clone();
        inner.data.clear();
        data
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let buf = TextBuffer::new();
        buf.push_char('H');
        buf.push_char('i');
        assert_eq!(buf.get_full(), "Hi");
    }

    #[test]
    fn test_pop() {
        let buf = TextBuffer::new();
        buf.push_char('A');
        buf.push_char('B');
        buf.pop_char();
        assert_eq!(buf.get_full(), "A");
    }

    #[test]
    fn test_clear() {
        let buf = TextBuffer::new();
        buf.push_char('X');
        buf.clear();
        assert!(buf.is_empty());
    }

    #[test]
    fn test_tail() {
        let buf = TextBuffer::new();
        for c in "Hello World!fix".chars() {
            buf.push_char(c);
        }
        assert_eq!(buf.get_tail(4), "!fix");
        assert_eq!(buf.get_tail(40), "Hello World!fix");
    }

    #[test]
    fn test_take() {
        let buf = TextBuffer::new();
        buf.push_char('A');
        let taken = buf.take();
        assert_eq!(taken, "A");
        assert!(buf.is_empty());
    }

    #[test]
    fn test_max_size() {
        let buf = TextBuffer::new();
        for _ in 0..MAX_BUFFER_SIZE + 100 {
            buf.push_char('A');
        }
        let full = buf.get_full();
        assert!(full.len() <= MAX_BUFFER_SIZE);
    }
}
