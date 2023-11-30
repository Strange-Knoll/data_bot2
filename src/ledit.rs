use std::io::{stdout, Stdout};

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::style::{Print, SetForegroundColor, Color};
use crossterm::terminal;
use crossterm::{execute, queue};

#[derive(Default)]
pub struct LineBuffer {
    buffer: String,
    cursor_index: usize,
}

pub struct Context {
    // Stdout is stored to prevent needing to call `std::io::stdout()` repeatedly
    stdout: Stdout,
    // The prompt width is needed to accurately calculate the cursor position
    prompt_width: usize,
    // The terminal size is neededed for almost all calculations
    // $ This will need to be continuously updated once resizing is supported
    terminal_width: usize,
    terminal_height: usize,
    scroll: ScrollState,
}

/// Represents the amount of scrolling that has occurred so far.
/// This information is crucial for calculations relating to the cursor position.
#[derive(Clone)]
pub enum ScrollState {
    // The y origin is the original y-coordinate of the prompt, which will be offset by the scroll
    Unscrolled { y_origin: usize },
    // Once the line has gotten longer than the terminal height, different calculations are required
    // to determine the position of the cursor and the viewport
    Scrolled { y_origin: usize, scroll: usize },
    // Once the editor scrolls past the prompt, the original prompt position becomes irrelevant to
    // position calculations, so the scroll here represents the number of lines off-screen
    ScrolledPastPrompt { scroll: usize },
}

impl LineBuffer {
    pub fn insert(&mut self, c: char) {
        // TODO: Add bounds check? Maybe unnecessary
        self.buffer.insert(self.cursor_index, c);
        self.right();
    }

    pub fn insert_str(&mut self, s: &str) {
        self.buffer.insert_str(self.cursor_index, s);
        for _ in 0..s.chars().count() {
            self.right();
        }
    }

    pub fn left(&mut self) {
        // The cursor position should never go below 0 (underflow)
        self.cursor_index = self.cursor_index.saturating_sub(1);
    }

    pub fn right(&mut self) {
        // The cursor position should never overrun the length of the buffer
        if self.cursor_index == self.buffer.chars().count() {
            return;
        }

        self.cursor_index = self.cursor_index.saturating_add(1);
    }

    pub fn backspace(&mut self) {
        // Backspace should do nothing if the cursor is at the start of the line
        if self.cursor_index == 0 {
            return;
        }

        self.left();
        self.delete();
    }

    pub fn delete(&mut self) {
        self.buffer.remove(self.cursor_index);
    }

    pub fn width(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Calculates the height of the prompt and line buffer in relation to the terminal width.
    /// Makes some adjustments for an edge case where the line takes up the entire terminal.
    pub fn height(&self, ctx: &Context) -> usize {
        // If the a line of text takes up the entire width of the terminal, the cursor will be on
        // the line below it, so this offset is required for scrolling to work properly
        let true_width = ctx.prompt_width + self.width() + 1;
        // Quotient is rounded up to account for partial lines
        ciel_div(true_width, ctx.terminal_width)
    }

    pub fn segment(&self, scroll: ScrollState, terminal_width: usize) -> &str {
        if let ScrollState::ScrolledPastPrompt { scroll } = scroll {
            let chars_off_screen = terminal_width * scroll;
            // $ Needs to be changed to some sort of safe slicing
            &self.buffer[chars_off_screen..]
        } else {
            &self.buffer
        }
    }
}

pub fn main() {
    for _ in 0..5 {
        println!();
    }

    let input = prompt("***$ ");
    println!("{input}");
}

pub fn prompt(prefix: &str) -> String {
    let mut line_buffer = LineBuffer::default();
    let (terminal_width, terminal_height) = terminal::size().unwrap();
    let mut ctx = Context {
        stdout: stdout(),
        prompt_width: prefix.chars().count(),
        terminal_width: terminal_width as usize,
        terminal_height: terminal_height as usize,
        scroll: ScrollState::Unscrolled {
            y_origin: cursor::position().unwrap().1 as usize,
        },
    };

    terminal::enable_raw_mode().unwrap();
    execute!(ctx.stdout, Print(prefix)).unwrap();
    loop {
        if handle(&mut ctx, &mut line_buffer, event::read().unwrap()) {
            terminal::disable_raw_mode().unwrap();
            execute!(ctx.stdout, Print("\n")).unwrap();
            return line_buffer.buffer;
        }
    }
}

pub fn pretty_prompt(username:&str, seperator:&str ,working_dir:&str, end:&str) -> String{
    let combined_line = format!("{}{}{}{}", username, seperator, working_dir, end);
    let mut line_buffer = LineBuffer::default();
    let (terminal_width, terminal_height) = terminal::size().unwrap();
    let mut ctx = Context {
        stdout: stdout(),
        prompt_width: combined_line.chars().count(),
        terminal_width: terminal_width as usize,
        terminal_height: terminal_height as usize,
        scroll: ScrollState::Unscrolled {
            y_origin: cursor::position().unwrap().1 as usize,
        },
    };

    terminal::enable_raw_mode().unwrap();
    execute!(
        ctx.stdout, 
        SetForegroundColor(Color::Green),
        Print(username),
        SetForegroundColor(Color::Reset),
        Print(seperator),
        SetForegroundColor(Color::Blue),
        Print(working_dir),
        SetForegroundColor(Color::Reset),
        Print(end)
        
    ).unwrap();
    loop {
        if handle(&mut ctx, &mut line_buffer, event::read().unwrap()) {
            terminal::disable_raw_mode().unwrap();
            execute!(ctx.stdout, Print("\n")).unwrap();
            return line_buffer.buffer;
        }
    }
}

pub fn handle(ctx: &mut Context, line: &mut LineBuffer, event: Event) -> bool {
    match event {
        Event::Key(key_event) => {
            if key_event.modifiers == KeyModifiers::NONE {
                match key_event.code {
                    KeyCode::Char(c) => {
                        line.insert(c);
                        update_screen(ctx, line, true);
                    }
                    KeyCode::Backspace => {
                        line.backspace();
                        update_screen(ctx, line, false);
                    }
                    KeyCode::Delete => {
                        line.delete();
                        update_screen(ctx, line, false);
                    }
                    KeyCode::Left => {
                        line.left();
                        update_cursor(ctx, line);
                    }
                    KeyCode::Right => {
                        line.right();
                        update_cursor(ctx, line);
                    }
                    KeyCode::Enter => {
                        return true;
                    }
                    _ => {}
                }
            } else if key_event.modifiers == KeyModifiers::SHIFT {
                match key_event.code {
                    KeyCode::Char(c) => {
                        line.insert(c);
                        update_screen(ctx, line, true);
                    }
                    KeyCode::Right => {
                        line.insert_str("DEBUG ");
                        update_screen(ctx, line, true);
                    }
                    _ => exit(1, "UNSUPPORTED KEY COMBINATION"),
                }
            } else {
                #[allow(clippy::match_single_binding)]
                match (key_event.modifiers, key_event.code) {
                    _ => exit(1, "UNSUPPORTED KEY COMBINATION"),
                }
            }
        }
        Event::Mouse(_) => exit(1, "MOUSE CAPTURE SHOULD BE DISABLED"),
        // Need to reflow the text most likely
        Event::Resize(_, _) => exit(1, "RESIZING IS NOT SUPPORTED YET"),
        Event::FocusGained => (),
        Event::FocusLost => (),
        Event::Paste(_) => exit(1, "BRACKETED PASTE SHOULD BE DISABLED"),
    }

    false
}

/// Updates the frame by (optionally) scrolling, updating the cursor, and redrawing the line buffer.
pub fn update_screen(ctx: &mut Context, line: &LineBuffer, scroll: bool) {
    if scroll {
        update_scroll(ctx, line)
    };

    update_cursor(ctx, line);
    redraw_buffer(ctx, line);
}

pub fn redraw_buffer(ctx: &mut Context, line: &LineBuffer) {
    let (draw_start_x, draw_start_y) = prompt_end_coord(ctx);
    let scroll = ctx.scroll.clone();
    let terminal_width = ctx.terminal_width;
    execute!(ctx.stdout, cursor::SavePosition).unwrap();
    queue!(
        ctx.stdout,
        terminal::Clear(terminal::ClearType::FromCursorDown),
        cursor::MoveTo(draw_start_x, draw_start_y),
        Print(line.segment(scroll, terminal_width))
    )
    .unwrap();
    execute!(ctx.stdout, cursor::RestorePosition).unwrap();
}

/// Updates the position of the cursor depending on the scroll state and the buffer index.
/// This should be called after `update_scroll()`.
pub fn update_cursor(ctx: &mut Context, line: &LineBuffer) {
    let (x, y) = cursor_coord(ctx, line);
    execute!(ctx.stdout, cursor::MoveTo(x, y)).unwrap();
}

/// Scrolls the terminal down the necessary amount of lines, changing the scroll state as needed.
/// This should generally be called first in the event that any text is added to the buffer.
pub fn update_scroll(ctx: &mut Context, line: &LineBuffer) {
    // Check if scroll is required, and if it is, scroll as needed and update the scroll state
    match ctx.scroll {
        ScrollState::Unscrolled { y_origin } => {
            let lines = line.height(ctx);
            // $ Check for off-by-1s here
            let utilized_height = ctx.terminal_height - y_origin;
            let remaining_height = ctx.terminal_height - utilized_height;
            if lines > utilized_height {
                let overrun = lines - utilized_height;
                scroll_down(ctx, overrun);

                // If enough lines are inserted at once, `ScrollState::Scrolled` must be skipped
                ctx.scroll = match overrun > remaining_height {
                    true => ScrollState::ScrolledPastPrompt {
                        scroll: overrun - y_origin,
                    },
                    false => ScrollState::Scrolled {
                        y_origin,
                        scroll: overrun,
                    },
                };
            }
        }
        ScrollState::Scrolled { y_origin, scroll } => {
            let lines = line.height(ctx);
            let utilized_height = ctx.terminal_height - y_origin + scroll;
            let remaining_height = ctx.terminal_height - utilized_height;
            if lines > utilized_height {
                let overrun = lines - utilized_height;
                scroll_down(ctx, overrun);

                ctx.scroll = match overrun > remaining_height {
                    true => ScrollState::ScrolledPastPrompt {
                        scroll: scroll + overrun - y_origin,
                    },
                    false => ScrollState::Scrolled {
                        y_origin,
                        scroll: scroll + overrun,
                    },
                };
            }
        }
        ScrollState::ScrolledPastPrompt { scroll } => {
            let mut lines_on_screen = line.height(ctx) - scroll;
            if lines_on_screen > ctx.terminal_height {
                let overrun = lines_on_screen - ctx.terminal_height;
                scroll_down(ctx, overrun);

                lines_on_screen -= overrun;
                ctx.scroll = ScrollState::ScrolledPastPrompt {
                    scroll: scroll + overrun,
                };
            }

            // If the editor has been scrolled past the prompt, the last line should always be at
            // the bottom, so the on-screen portion will always take up the entire terminal height
            assert!(lines_on_screen == ctx.terminal_height);
        }
    }
}

/// Scrolls the terminal down a given number of lines.
/// Does not preserve the cursor position.
pub fn scroll_down(ctx: &mut Context, lines: usize) {
    queue!(
        ctx.stdout,
        cursor::MoveTo(ctx.terminal_width as u16, ctx.terminal_height as u16)
    )
    .unwrap();

    terminal::disable_raw_mode().unwrap();
    for _ in 0..lines {
        queue!(ctx.stdout, Print("\n")).unwrap();
    }

    execute!(ctx.stdout).unwrap();
    terminal::enable_raw_mode().unwrap();
}

/// Calculates the index of the cursor, accounting for the offset introduced by the prompt text.
pub fn true_index(ctx: &Context, line: &LineBuffer) -> usize {
    ctx.prompt_width + line.cursor_index
}

// $ This will definitely cause a bug when the terminal is resized or when the user scrolls
/// Calculates the coordinates of the cursor on the screen based on its index in the line buffer.
/// These coordinates are only correct if the correct amount of scroll has already been applied.
pub fn cursor_coord(ctx: &Context, line: &LineBuffer) -> (u16, u16) {
    (cursor_x_coord(ctx, line), cursor_y_coord(ctx, line))
}

/// Calculates the cursor coordinates of the end of the prompt, used for redrawing the buffer.
pub fn prompt_end_coord(ctx: &Context) -> (u16, u16) {
    (prompt_end_x_coord(ctx), prompt_end_y_coord(ctx))
}

pub fn cursor_x_coord(ctx: &Context, line: &LineBuffer) -> u16 {
    let x = (ctx.prompt_width + line.cursor_index) % ctx.terminal_width;
    assert!(x < ctx.terminal_width);
    x as u16
}

pub fn cursor_y_coord(ctx: &Context, line: &LineBuffer) -> u16 {
    let base = true_index(ctx, line) / ctx.terminal_width;
    let y = match ctx.scroll {
        ScrollState::Unscrolled { y_origin } => base + y_origin,
        ScrollState::Scrolled { y_origin, scroll } => base + y_origin - scroll,
        ScrollState::ScrolledPastPrompt { scroll } => base - scroll,
    };

    assert!(y < ctx.terminal_height);
    y as u16
}

/// Calculates the x-coordinate of the end of the prompt, used for redrawing the buffer.
pub fn prompt_end_x_coord(ctx: &Context) -> u16 {
    let x = ctx.prompt_width % ctx.terminal_width;
    assert!(x < ctx.terminal_width);
    x as u16
}

/// Calculates the y-coordinate of the end of the prompt, used for redrawing the buffer.
pub fn prompt_end_y_coord(ctx: &Context) -> u16 {
    let base = ctx.prompt_width / ctx.terminal_width;
    let y = match ctx.scroll {
        ScrollState::Unscrolled { y_origin } => base + y_origin,
        ScrollState::Scrolled { y_origin, scroll } => base + (y_origin - scroll),
        ScrollState::ScrolledPastPrompt { scroll: _ } => 0,
    };

    assert!(y < ctx.terminal_height);
    y as u16
}

pub fn exit(code: i32, msg: &str) -> ! {
    terminal::disable_raw_mode().unwrap();
    eprintln!("\n{}", msg);
    std::process::exit(code);
}

pub fn ciel_div(a: usize, b: usize) -> usize {
    let mut result = a / b;
    if a % b > 0 {
        result += 1;
    }
    result
}