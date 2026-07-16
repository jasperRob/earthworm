use super::Component;
use crate::action::Action;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph},
};

const DEFAULT_TICK_DURATION: u8 = 50;

#[derive(Default)]
pub struct Notification {
    msg: Option<String>,
    ticks_remaining: u8,
}

impl Notification {}

impl Component for Notification {
    fn is_capturing_input(&self) -> bool {
        false
    }
    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        match action {
            Action::Tick if self.msg.is_some() => {
                self.ticks_remaining = self.ticks_remaining.saturating_sub(1);
                if self.ticks_remaining == 0 {
                    self.msg = None;
                }
            }
            Action::Error(msg) => {
                self.msg = Some(msg);
                self.ticks_remaining = DEFAULT_TICK_DURATION;
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let Some(msg) = self.msg.as_deref() else {
            return Ok(());
        };
        // TODO: Add line wrapping for long messages.
        let width = (msg.len() as u16 + 4).min(area.width);
        let height = 3;
        let toast_area = Rect::new(area.width.saturating_sub(width + 1), 1, width, height);
        frame.render_widget(Clear, toast_area);
        frame.render_widget(
            Paragraph::new(msg)
                .block(
                    Block::bordered()
                        // TODO: Add different notification types and styles
                        .title("Error")
                        .border_style(Style::default().fg(Color::Red)),
                )
                .left_aligned(),
            toast_area,
        );
        Ok(())
    }
}
