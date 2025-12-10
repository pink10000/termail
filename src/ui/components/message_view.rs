use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},   
};
use crate::core::email::EmailMessage;

#[derive(Clone, Debug)]
pub struct Messager {
    pub email: EmailMessage,
    /// Vertical scroll offset (in lines) for the message view
    pub scroll: u16,
    /// The height of the Paragraph widget
    pub content_height: u16,
}

impl Widget for Messager {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let email_from = &self.email.from;
        let email_body = &self.email.body;
        
        // Count image attachments
        let image_count = self.email.get_image_attachments().len();
        let attachment_info = if image_count > 0 {
            format!("\n\n{} image attachment(s)", image_count)
        } else {
            String::new()
        };

        let block = Block::default()
            .title(email_from.display_name())
            .title(email_from.formatted_email())
            .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));
        
        let full_text = format!("{}{}", email_body, attachment_info);
        
        let paragraph = Paragraph::new(full_text)
            .wrap(ratatui::widgets::Wrap { trim: false }) 
            .scroll((self.scroll, 0))
            .block(block);
        
        paragraph.render(area, buf);
    }
}
