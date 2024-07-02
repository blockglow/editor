use std::{io::stdout, iter, mem, ops::Index, thread, time::{Duration, Instant}};

use crossterm::{
    cursor::{self, MoveTo, SetCursorStyle}, event::{
        poll, read, Event, KeyCode, KeyEvent, KeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    }, execute, queue, style::Print, terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, EnterAlternateScreen, LeaveAlternateScreen,
    }
};
use derive_more::{Deref, DerefMut};
use glam::{I64Vec2, IVec3, U16Vec2};

#[derive(Default)]
pub struct Line {
	dirty: Vec<u8>,
	forward: String,
	backward: String,
}
impl Line {
    fn len(&self) -> usize {
	self.forward.len() + self.backward.len()
    }
}

#[derive(Deref, DerefMut, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogicalPos(I64Vec2);

impl From<Move> for LogicalPos {
    fn from(Move { x, y }: Move) -> Self {
	Self(I64Vec2 { x: x as _, y: y as _ })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Move { x: i64, y: i64 }

impl From<I64Vec2> for Move {
    fn from(I64Vec2 { x, y }: I64Vec2) -> Self {
	Self { x: x as _, y: y as _ }
    }
}

impl From<Move> for I64Vec2 {
	fn from(Move { x, y }: Move) -> Self {
	    Self {
		x: x as _,
		y: y as _,
	    }
	}
}

#[derive(Deref, DerefMut, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphicalPos(U16Vec2);

pub struct File {
	lines: Vec<Line>,
	current_caret: LogicalPos,
	desired_caret: LogicalPos,
}

impl Default for File {
    fn default() -> Self {
	Self { lines: Default::default(), current_caret: Default::default(), desired_caret: Default::default() }
    }
}

pub enum ControlFlow {
	Continue,
	Exit
}

struct Editor {
	open: usize,
	files: Vec<File>,
	last_activity: Instant,
}

impl Editor {
	fn apply(&mut self, action: Action) -> ControlFlow {
		use Action::*;

		match action {
			Exit => return ControlFlow::Exit,
			Right => self.move_caret(Move::from(I64Vec2::X)),	
			Left => self.move_caret(Move::from(-I64Vec2::X)),	
			Up => self.move_caret(Move::from(I64Vec2::Y)),
			Down => self.move_caret(Move::from(-I64Vec2::Y)),
			Delete => {},
			Place(s) => self.place(s),
			Remove => self.remove(),
		}

		ControlFlow::Continue
	}
	fn remove(&mut self) {
		let file = &mut self.files[self.open];
		file.lines[file.current_caret.y as usize].forward.pop();
		file.current_caret.x.saturating_sub(1);
	}
	fn place(&mut self, s: impl AsRef<str>) {
		let file = &mut self.files[self.open];
		file.lines.resize_with(file.current_caret.y as usize + 1, Line::default);
		let line = &mut file.lines[file.current_caret.y as usize];
		line.forward += s.as_ref();
		file.current_caret.x = file.current_caret.x.saturating_add(s.as_ref().len() as _).min(line.len() as _);
		line.dirty.resize(line.len() / mem::size_of::<u8>() + 1, 0);
		line.dirty[file.current_caret.x as usize / mem::size_of::<u8>()] |= 1 << (file.current_caret.x as usize % mem::size_of::<u8>());
	}

	fn move_caret(&mut self, m: Move) {
		let file = &mut self.files[self.open];
		let line_count = file.lines.len();
		*file.desired_caret += I64Vec2::from(m);
		file.desired_caret.y = file.desired_caret.y.max(line_count as _);
		let line = &file.lines[file.desired_caret.y as usize];
		let line_len = line.len();
		file.desired_caret.x = file.desired_caret.x.max(line_len as _);
	}
	
	fn draw(&self) {

		for (y, line) in self.files[self.open].lines.iter().enumerate() {
			let Line { dirty, forward, backward } = &line;

			for x in 0..dirty.len() {
				if dirty[x / mem::size_of::<u8>()] & (1 << (x % mem::size_of::<u8>())) != 0 {
					
					if x < forward.len() {
						execute! {
							stdout(),
							MoveTo(x as u16, y as u16),
						}.unwrap();
						execute! {
							stdout(),
							Print(forward.get(x..=x).unwrap()),
						}.unwrap();
					} else {
						let x = x - forward.len();
						let Some(s) = backward.get(x..=x) else {
							continue;
						};
						execute! {
							stdout(),
							MoveTo(x as u16, y as u16),
						}.unwrap();
						execute! {
							stdout(),
							Print(s),
						}.unwrap();
					}
				}
			}
		}
		
	}
}

impl Default for Editor {
    fn default() -> Self {
	Self { open: Default::default(), files: vec![File::default()], last_activity: Instant::now() }
    }
}

enum Action {
	Exit,
	Up,
	Down,
	Left,
	Right,
	Place(String),
	Remove,
	Delete,
}

fn main() {
    enable_raw_mode().unwrap();

    let supports_keyboard_enhancement = matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    );

    if supports_keyboard_enhancement {
        queue!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )
        .unwrap();
    }
    execute! { stdout(), EnterAlternateScreen }.unwrap();

    let mut editor = Editor::default();
    loop {
        thread::sleep(Duration::from_millis(5));

        let (w, h) = size().unwrap();

	editor.draw();

        let Ok(true) = poll(Duration::from_millis(1)) else {
		continue;
        };

        let event = read().unwrap();

        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => {
                break;
            }
	    Event::Key(KeyEvent {
                code: KeyCode::Backspace, ..
            }) => {
		editor.apply(Action::Remove);
	    }	
	    Event::Key(KeyEvent {
                code: KeyCode::Char(c), ..
            }) => {
		editor.apply(Action::Place(c.to_string()));
            }
            _ => {}
        }
    }

    disable_raw_mode().unwrap();

    execute! { stdout(), LeaveAlternateScreen }.unwrap();
}
