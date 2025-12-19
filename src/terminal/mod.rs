use vte::{Params, Parser, Perform};

const MAX_SCROLLBACK: usize = 10_000;

#[derive(Clone, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub bold: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
        }
    }
}

pub struct VirtualTerminal {
    width: usize,
    height: usize,
    screen: Vec<Vec<Cell>>,
    scrollback: Vec<Vec<Cell>>,
    cursor_x: usize,
    cursor_y: usize,
    parser: Parser,
    current_fg: Option<u8>,
    current_bg: Option<u8>,
    current_bold: bool,
}

impl VirtualTerminal {
    pub fn new(width: usize, height: usize) -> Self {
        let mut screen = Vec::with_capacity(height);
        for _ in 0..height {
            screen.push(vec![Cell::default(); width]);
        }

        Self {
            width,
            height,
            screen,
            scrollback: Vec::new(),
            cursor_x: 0,
            cursor_y: 0,
            parser: Parser::new(),
            current_fg: None,
            current_bg: None,
            current_bold: false,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;

        // Recreate screen with new dimensions
        let mut new_screen = Vec::with_capacity(height);
        for _ in 0..height {
            new_screen.push(vec![Cell::default(); width]);
        }

        // Copy old content
        for (y, line) in self.screen.iter().enumerate() {
            if y >= height {
                break;
            }
            for (x, cell) in line.iter().enumerate() {
                if x >= width {
                    break;
                }
                new_screen[y][x] = cell.clone();
            }
        }

        self.screen = new_screen;
    }

    pub fn feed_bytes(&mut self, data: &[u8]) {
        // Temporarily swap out parser to avoid borrow checker issues
        let mut parser = std::mem::replace(&mut self.parser, Parser::new());
        for byte in data {
            parser.advance(self, *byte);
        }
        self.parser = parser;
    }

    pub fn get_screen(&self) -> &[Vec<Cell>] {
        &self.screen
    }

    pub fn get_scrollback(&self) -> &[Vec<Cell>] {
        &self.scrollback
    }

    pub fn clear(&mut self) {
        self.clear_screen();
        self.scrollback.clear();
    }

    fn write_char(&mut self, ch: char) {
        if ch == '\n' {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.height {
                self.scroll_up();
            }
            return;
        }

        if ch == '\r' {
            self.cursor_x = 0;
            return;
        }

        if ch == '\t' {
            // Tab to next 8-column boundary
            self.cursor_x = ((self.cursor_x / 8) + 1) * 8;
            if self.cursor_x >= self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
                if self.cursor_y >= self.height {
                    self.scroll_up();
                }
            }
            return;
        }

        if self.cursor_x >= self.width {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.height {
                self.scroll_up();
            }
        }

        if self.cursor_y < self.height {
            self.screen[self.cursor_y][self.cursor_x] = Cell {
                ch,
                fg: self.current_fg,
                bg: self.current_bg,
                bold: self.current_bold,
            };
            self.cursor_x += 1;
        }
    }

    fn scroll_up(&mut self) {
        // Move top line to scrollback
        if !self.screen.is_empty() {
            let top_line = self.screen.remove(0);
            self.scrollback.push(top_line);

            // Trim scrollback if too large
            if self.scrollback.len() > MAX_SCROLLBACK {
                self.scrollback.drain(0..1000);
            }
        }

        // Add blank line at bottom
        self.screen.push(vec![Cell::default(); self.width]);
        self.cursor_y = self.height.saturating_sub(1);
    }

    fn clear_screen(&mut self) {
        for line in &mut self.screen {
            for cell in line {
                *cell = Cell::default();
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }
}

impl Perform for VirtualTerminal {
    fn print(&mut self, ch: char) {
        self.write_char(ch);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.write_char('\n'),
            b'\r' => self.write_char('\r'),
            b'\t' => self.write_char('\t'),
            0x08 => {
                // Backspace
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _c: char) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, c: char) {
        match c {
            'H' | 'f' => {
                // Cursor position
                let mut iter = params.iter();
                let y = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                let x = iter.next().and_then(|p| p.first()).copied().unwrap_or(1) as usize;
                self.cursor_y = (y.saturating_sub(1)).min(self.height - 1);
                self.cursor_x = (x.saturating_sub(1)).min(self.width - 1);
            }
            'J' => {
                // Clear screen
                let param = params
                    .iter()
                    .next()
                    .and_then(|p| p.first())
                    .copied()
                    .unwrap_or(0);
                if param == 2 {
                    self.clear_screen();
                }
            }
            'K' => {
                // Clear line
                if self.cursor_y < self.height {
                    for x in self.cursor_x..self.width {
                        self.screen[self.cursor_y][x] = Cell::default();
                    }
                }
            }
            'm' => {
                // SGR - Set graphics rendition
                if params.is_empty() {
                    // Reset
                    self.current_fg = None;
                    self.current_bg = None;
                    self.current_bold = false;
                } else {
                    for param in params.iter() {
                        if let Some(&code) = param.first() {
                            match code {
                                0 => {
                                    self.current_fg = None;
                                    self.current_bg = None;
                                    self.current_bold = false;
                                }
                                1 => self.current_bold = true,
                                22 => self.current_bold = false,
                                30..=37 => self.current_fg = Some((code - 30) as u8),
                                40..=47 => self.current_bg = Some((code - 40) as u8),
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
