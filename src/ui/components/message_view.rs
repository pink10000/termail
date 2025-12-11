use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    prelude::{Constraint, Direction, Layout, StatefulWidget},
};
use crate::core::email::EmailMessage;
use ratatui_image::{StatefulImage, thread::ThreadProtocol};

#[derive(Clone, Debug)]
pub struct Messager {
    pub email: EmailMessage,
    /// Vertical scroll offset (in lines) for the message view
    pub scroll: u16,
    /// The height of the Paragraph widget
    pub content_height: u16,
}

impl Messager {
    pub fn new(email: EmailMessage, content_height: u16) -> Self {
        Self {
            email,
            scroll: 0,
            content_height,
        }
    }

    pub fn render_with_images(
        &self,
        area: Rect,
        buf: &mut Buffer,
        image_state: &mut Option<ThreadProtocol>
    ) {
        let email_from = &self.email.from;
        let email_body = &self.email.body;

        // This block defines the entire border of the text and attachments.
        let total_block = Block::default()
            .title(email_from.display_name())
            .title(email_from.formatted_email())
            .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));

        let inner_area = total_block.inner(area);
        total_block.render(area, buf);

        // Split the inner area if there's an image. Otherwise, termail will just render the text.
        if let Some(mut async_state) = image_state.as_mut() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),      // Text area (flexible, minimum 5 lines)
                    Constraint::Length(20),  // Image area (fixed 20 lines)
                ])
                .split(inner_area);

            let paragraph = Paragraph::new(email_body.to_string())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(chunks[0], buf);
            let image_widget: StatefulImage<ThreadProtocol> = StatefulImage::default();
            image_widget.render(chunks[1], buf, &mut async_state);
        } else {
            let paragraph = Paragraph::new(email_body.to_string())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(inner_area, buf);
        }
    }
}