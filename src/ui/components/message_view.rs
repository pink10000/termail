use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};
use crate::types::EmailMessage;
use crate::ui::app::MessageViewState;

pub struct MessageView<'a> {
    pub email: &'a EmailMessage,
    pub scroll: u16,
    pub state: &'a MessageViewState,
}

impl<'a> Widget for MessageView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let email_from = &self.email.from;
        let email_body = &self.email.body;

        let block = Block::default()
            .title(email_from.display_name())
            .title(email_from.formatted_email())
            .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));
        
        let paragraph = Paragraph::new(email_body.clone())
            .wrap(ratatui::widgets::Wrap { trim: false }) 
            .scroll((self.scroll, 0))
            .block(block);
        
        paragraph.render(area, buf);
    }
}