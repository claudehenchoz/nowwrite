pub mod buffer;
pub mod cursor;
pub mod markdown;

use crate::platform::{Platform, KeyEvent};
use crate::ui::{renderer::Renderer, modal::{FileModal, KeyboardModal, ModalResult}};
use crate::file::{manager::FileManager, backup::BackupManager};
use crate::config::keyboard::KeyboardConfig;
use buffer::TextBuffer;
use cursor::Cursor;
use std::io::Result;

pub struct Editor<P: Platform> {
    renderer: Renderer<P>,
    file_manager: FileManager<P>,
    backup_manager: BackupManager<P>,
    keyboard_config: KeyboardConfig<P>,
    buffer: TextBuffer,
    running: bool,
}

impl<P: Platform + Clone> Editor<P> {
    pub fn new(platform: P) -> Result<Self> {
        let renderer = Renderer::new(platform.clone())?;
        let file_manager = FileManager::new(platform.clone());
        let backup_manager = BackupManager::new(platform.clone());
        let mut keyboard_config = KeyboardConfig::new(platform);
        
        keyboard_config.load().ok();
        
        Ok(Self {
            renderer,
            file_manager,
            backup_manager,
            keyboard_config,
            buffer: TextBuffer::new(),
            running: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.show_file_modal()?;
        
        while self.running {
            self.backup_manager.check_and_backup(&self.buffer)?;
            self.renderer.render_buffer(&self.buffer)?;
            
            let key = self.renderer.get_platform_mut().read_key()?;
            let mapped_key = self.map_key_input(key);
            self.handle_key(mapped_key)?;
        }
        
        Ok(())
    }

    fn map_key_input(&self, key: KeyEvent) -> KeyEvent {
        match key {
            KeyEvent::Char(ch) => {
                let mapped_ch = self.keyboard_config.current_layout().map_char(ch);
                KeyEvent::Char(mapped_ch)
            }
            other => other,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key {
            KeyEvent::Escape => {
                self.running = false;
            }
            KeyEvent::Char(ch) => {
                self.buffer.insert_char(ch);
            }
            KeyEvent::Enter => {
                self.buffer.insert_char('\n');
            }
            KeyEvent::Backspace => {
                self.buffer.delete_char();
            }
            KeyEvent::Delete => {
                // Delete character forward
                let cursor = self.buffer.cursor();
                let lines = self.buffer.lines();
                if cursor.column < lines[cursor.line].len() {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column + 1));
                    self.buffer.delete_char();
                }
            }
            KeyEvent::Up => {
                let cursor = self.buffer.cursor();
                if cursor.line > 0 {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line - 1, cursor.column));
                }
            }
            KeyEvent::Down => {
                let cursor = self.buffer.cursor();
                let lines = self.buffer.lines();
                if cursor.line < lines.len() - 1 {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line + 1, cursor.column));
                }
            }
            KeyEvent::Left => {
                let cursor = self.buffer.cursor();
                if cursor.column > 0 {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column - 1));
                } else if cursor.line > 0 {
                    let prev_line_len = self.buffer.lines()[cursor.line - 1].len();
                    self.buffer.move_cursor(Cursor::new_at(cursor.line - 1, prev_line_len));
                }
            }
            KeyEvent::Right => {
                let cursor = self.buffer.cursor();
                let lines = self.buffer.lines();
                if cursor.column < lines[cursor.line].len() {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column + 1));
                } else if cursor.line < lines.len() - 1 {
                    self.buffer.move_cursor(Cursor::new_at(cursor.line + 1, 0));
                }
            }
            KeyEvent::Home => {
                let cursor = self.buffer.cursor();
                self.buffer.move_cursor(Cursor::new_at(cursor.line, 0));
            }
            KeyEvent::End => {
                let cursor = self.buffer.cursor();
                let line_len = self.buffer.lines()[cursor.line].len();
                self.buffer.move_cursor(Cursor::new_at(cursor.line, line_len));
            }
            KeyEvent::CtrlS => {
                self.save_file()?;
            }
            KeyEvent::CtrlL => {
                self.show_file_modal()?;
            }
            KeyEvent::CtrlN => {
                self.new_file()?;
            }
            KeyEvent::CtrlK => {
                self.show_keyboard_modal()?;
            }
            KeyEvent::CtrlZ => {
                self.buffer.undo();
            }
            KeyEvent::CtrlY => {
                self.buffer.redo();
            }
            KeyEvent::CtrlB => {
                self.toggle_bold()?;
            }
            KeyEvent::CtrlI => {
                self.toggle_italic()?;
            }
            _ => {}
        }
        
        Ok(())
    }

    fn save_file(&mut self) -> Result<()> {
        if self.buffer.file_path().is_some() {
            self.file_manager.save_file(&mut self.buffer)?;
            self.backup_manager.create_manual_backup(&self.buffer)?;
        } else {
            let filename = self.file_manager.get_safe_filename("untitled");
            self.file_manager.save_file_as(&mut self.buffer, &filename)?;
        }
        Ok(())
    }

    fn show_file_modal(&mut self) -> Result<()> {
        let platform = self.file_manager.get_platform_mut().clone();
        let mut modal = FileModal::new(platform, ".")?;
        
        match modal.show()? {
            ModalResult::Selected(filename) => {
                if filename == "new_file.md" {
                    self.new_file()?;
                } else if self.file_manager.file_exists(&filename) {
                    self.buffer = self.file_manager.load_file(&filename)?;
                } else {
                    self.buffer = self.file_manager.create_new_file(&filename)?;
                }
            }
            ModalResult::Cancelled => {
                if self.buffer.file_path().is_none() {
                    self.new_file()?;
                }
            }
        }
        
        Ok(())
    }

    fn show_keyboard_modal(&mut self) -> Result<()> {
        let platform = self.keyboard_config.get_platform_mut().clone();
        let mut modal = KeyboardModal::new(platform);
        
        match modal.show()? {
            ModalResult::Selected(layout_code) => {
                self.keyboard_config.set_layout(&layout_code)?;
            }
            ModalResult::Cancelled => {}
        }
        
        Ok(())
    }

    fn new_file(&mut self) -> Result<()> {
        self.buffer = TextBuffer::new();
        Ok(())
    }

    fn toggle_bold(&mut self) -> Result<()> {
        if let Some(selection) = self.buffer.selection() {
            let _selection = selection.normalize();
            // Add **text** formatting around selection
            // This is a simplified implementation
        } else {
            self.buffer.insert_string("****");
            let cursor = self.buffer.cursor();
            self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column - 2));
        }
        Ok(())
    }

    fn toggle_italic(&mut self) -> Result<()> {
        if let Some(selection) = self.buffer.selection() {
            let _selection = selection.normalize();
            // Add *text* formatting around selection
            // This is a simplified implementation
        } else {
            self.buffer.insert_string("**");
            let cursor = self.buffer.cursor();
            self.buffer.move_cursor(Cursor::new_at(cursor.line, cursor.column - 1));
        }
        Ok(())
    }
}