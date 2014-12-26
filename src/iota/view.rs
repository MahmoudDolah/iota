use buffer::{Buffer, Direction, Mark};
use input::Input;
use uibuf::{UIBuffer, CharColor};
use frontends::Frontend;

/// A View is an abstract Window (into a Buffer).
///
/// It draws a portion of a Buffer to a UIBuffer which in turn is drawn to the
/// screen. It maintains the status bar for the current view, the "dirty status"
/// which is whether the buffer has been modified or not and a number of other
/// pieces of information.
pub struct View<'v> {
    pub buffer: Buffer,     //Text buffer
    first_char: Mark,       //First character to be displayed
    cursor: Mark,           //Cursor displayed by this buffer.
    uibuf: UIBuffer,        //UIBuffer
}

impl<'v> View<'v> {

    //----- CONSTRUCTORS ---------------------------------------------------------------------------

    pub fn new(source: Input, width: uint, height: uint) -> View<'v> {
        let mut buffer = match source {
            Input::Filename(path) => {
                match path {
                    Some(s) => Buffer::new_from_file(Path::new(s)),
                    None    => Buffer::new(),
                }
            },
            Input::Stdin(reader) => {
                Buffer::new_from_reader(reader)
            },
        };

        // NOTE(greg): this may not play well with resizing
        let uibuf = UIBuffer::new(width, height);

        let cursor = Mark::Cursor(0);
        buffer.set_mark(cursor, 0);
        let first_char = Mark::DisplayMark(0);
        buffer.set_mark(first_char, 0);

        View {
            buffer: buffer,
            first_char: first_char,
            cursor: cursor,
            uibuf: uibuf,
        }
    }

    //----- DRAWING METHODS ------------------------------------------------------------------------

    pub fn get_height(&self) -> uint {
        // NOTE(greg): when the status bar needs to move up, this value should be changed
        self.uibuf.get_height() -1
    }

    pub fn get_width(&self) -> uint {
        self.uibuf.get_width()
    }

    /// Clear the buffer
    ///
    /// Fills every cell in the UIBuffer with the space (' ') char.
    pub fn clear(&mut self, frontend: &mut Box<Frontend + 'v>) {
        self.uibuf.fill(' ');
        self.uibuf.draw_everything(frontend);
    }

    pub fn draw(&mut self, frontend: &mut Box<Frontend + 'v>) {
        for (index,line) in self.buffer
                                .lines_from(self.first_char)
                                .unwrap()
                                .enumerate() {
            draw_line(&mut self.uibuf, line, index);
            if index == self.get_height() { break; }
        }
        self.uibuf.draw_everything(frontend);
    }

    pub fn draw_status(&mut self, frontend: &mut Box<Frontend + 'v>) {
        let buffer_status = self.buffer.status_text();
        let mut cursor_status = self.buffer.get_mark_coords(self.cursor).unwrap_or((0,0));
        cursor_status = (cursor_status.0 + 1, cursor_status.1 + 1);
        let status_text = format!("{} {}", buffer_status, cursor_status).into_bytes();
        let status_text_len = status_text.len();
        let width = self.get_width();
        let height = self.get_height();


        for index in range(0, width) {
            let mut ch: char = ' ';
            if index < status_text_len {
                ch = status_text[index] as char;
            }
            self.uibuf.update_cell(index, height, ch, CharColor::Black, CharColor::Blue);
        }

        self.uibuf.draw_range(frontend, height, height+1);
    }

    pub fn draw_cursor(&mut self, frontend: &mut Box<Frontend + 'v>) {
        if let Some((x, y)) = self.buffer.get_mark_coords(self.cursor) {
            frontend.draw_cursor(x as int, y as int);
        }
    }

    pub fn resize(&mut self, frontend: &mut Box<Frontend + 'v>) {
        let width = self.uibuf.get_width();
        self.clear(frontend);
        self.uibuf = UIBuffer::new(width, 15);
    }

    //----- CURSOR METHODS -------------------------------------------------------------------------

    pub fn move_cursor(&mut self, direction: Direction) {
        self.buffer.shift_mark(self.cursor, direction);
        self.move_screen();
    }

    pub fn move_cursor_to_line_end(&mut self) {
        self.buffer.shift_mark(self.cursor, Direction::LineEnd);
        self.move_screen();
    }

    pub fn move_cursor_to_line_start(&mut self) {
        self.buffer.shift_mark(self.cursor, Direction::LineStart);
        self.move_screen();
    }

    fn move_screen(&mut self) {

        //Update the first_char mark if necessary to keep the cursor on the screen.
        let cursor_y = self.buffer.get_mark_coords(self.cursor).unwrap().1;
        let first_char_y = self.buffer.get_mark_coords(self.first_char).unwrap().1;
        if cursor_y < first_char_y {
            self.buffer.shift_mark(self.first_char, Direction::Up(1));
        } else if cursor_y > first_char_y + self.get_height() {
            self.buffer.shift_mark(self.first_char, Direction::Down(1));
        }

    }

    //----- TEXT EDIT METHODS ----------------------------------------------------------------------

    pub fn delete_char(&mut self, direction: Direction) {
        match direction {
            Direction::Left(1) if self.buffer.get_mark_idx(self.cursor) != Some(0) => {
                self.move_cursor(direction);
                self.buffer.remove_char(self.cursor);
            }
            Direction::Right(1) if self.buffer.get_mark_idx(self.cursor) != Some(self.buffer.len()) => {
                self.buffer.remove_char(self.cursor);
                self.move_cursor(direction);
            }
            _ => {}
        }
    }

    pub fn insert_tab(&mut self) {
        // A tab is just 4 spaces
        for _ in range(0i, 4) {
            self.insert_char(' ');
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert_char(self.cursor, ch as u8);
        self.move_cursor(Direction::Right(1))
    }

    pub fn undo(&mut self) {
        let point = if let Some(transaction) = self.buffer.undo() { transaction.end_point }
                    else { return; };
        self.buffer.set_mark(self.cursor, point);
    }

    pub fn redo(&mut self) {
        let point = if let Some(transaction) = self.buffer.redo() { transaction.end_point }
                    else { return; };
        self.buffer.set_mark(self.cursor, point);
    }

}

pub fn draw_line(buf: &mut UIBuffer, line: &[u8], idx: uint) {
    let width = buf.get_width() - 1;
    let mut wide_chars = 0;
    for line_idx in range(0, width) {
        if line_idx < line.len() {
            match line[line_idx] {
                b'\t'   => {
                    let w = 4 - line_idx % 4;
                    for _ in range(0, w) {
                        buf.update_cell_content(line_idx + wide_chars, idx, ' ');
                    }
                }
                b'\n'   => buf.update_cell_content(line_idx + wide_chars, idx, ' '),
                _       => buf.update_cell_content(line_idx + wide_chars, idx,
                                                   line[line_idx] as char),
            }
            wide_chars += (line[line_idx] as char).width(false).unwrap_or(1) - 1;
        } else { buf.update_cell_content(line_idx + wide_chars, idx, ' '); }
    }
    if line.len() >= width {
        buf.update_cell_content(width + wide_chars, idx, '→');
    }

}

#[cfg(test)]
mod tests {

    use buffer::Direction;
    use view::View;
    use input::Input;

    fn setup_view<'v>(testcase: &'static str) -> View<'v> {
        let mut view = View::new(Input::Filename(None), 50, 50);
        for ch in testcase.chars() {
            view.insert_char(ch);
        }
        view.buffer.set_mark(view.cursor, 0);
        view
    }

    #[test]
    fn test_move_cursor_down() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Down(1));
        assert_eq!(view.buffer.get_mark_coords(view.cursor).unwrap().1, 1);
        assert_eq!(view.buffer.lines_from(view.cursor).unwrap().next().unwrap(), b"second"[]);
    }

    #[test]
    fn test_move_cursor_up() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Down(1));
        view.move_cursor(Direction::Up(1));
        assert_eq!(view.buffer.get_mark_coords(view.cursor).unwrap().1, 0);
        assert_eq!(view.buffer.lines_from(view.cursor).unwrap().next().unwrap(), b"test\n"[]);
    }

    #[test]
    fn test_insert_line() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Right(1));
        view.insert_char('\n');
        let lines: Vec<&[u8]> = view.buffer.lines().collect();

        assert_eq!(view.buffer.get_mark_coords(view.cursor).unwrap(), (0, 1))
    }

    #[test]
    fn test_insert_char() {
        let mut view = setup_view("test\nsecond");
        view.insert_char('t');

        assert_eq!(view.buffer.lines().next().unwrap(), b"ttest\n"[]);
    }

    #[test]
    fn test_delete_char_to_right() {
        let mut view = setup_view("test\nsecond");
        view.delete_char(Direction::Right(1));

        assert_eq!(view.buffer.lines().next().unwrap(), b"est\n"[]);
    }

    #[test]
    fn test_delete_char_to_left() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Right(1));
        view.delete_char(Direction::Left(1));

        assert_eq!(view.buffer.lines().next().unwrap(), b"est\n"[]);
    }


    #[test]
    fn test_delete_char_at_start_of_line() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Down(1));
        view.delete_char(Direction::Left(1));

        assert_eq!(view.buffer.lines().next().unwrap(), b"testsecond"[]);
    }

    #[test]
    fn test_delete_char_at_end_of_line() {
        let mut view = setup_view("test\nsecond");
        view.move_cursor(Direction::Right(4));
        view.delete_char(Direction::Right(1));

        assert_eq!(view.buffer.lines().next().unwrap(), b"testsecond"[]);
    }

    #[test]
    fn deleting_backward_at_start_of_first_line_does_nothing() {
        let mut view = setup_view("test\nsecond");
        view.delete_char(Direction::Left(1));

        let lines: Vec<&[u8]> = view.buffer.lines().collect();

        assert_eq!(lines.len(), 2);
        assert_eq!(view.buffer.lines().next().unwrap(), b"test\n");
    }
}
