use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    prelude::StatefulWidget,
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

    pub fn render_with_images(&self, area: Rect, buf: &mut Buffer, image_state: &mut Option<ThreadProtocol>) {
        // need to render this a scroll view, and then render the images inline after the text. if the image is not inline,
        // we need to render it at the end of the message view.
        // for now, assume the image is always at the end, since greenmail does not support inline images.
        // however, gmail does support inline images so we can handle this later. (this requires a change in the way we
        // parse the email since gmail has their own way of indicating inline images.)
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

        // TODO: Put this in the status bar in the future.
        let full_text = format!("{}{}", email_body, attachment_info);

        let paragraph = Paragraph::new(full_text)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((self.scroll, 0))
            .block(block);

        paragraph.render(area, buf);

        // Render every image at the end of the scroll view.
        if let Some(mut async_state) = image_state.as_mut() {
            let image_widget: StatefulImage<ThreadProtocol> = StatefulImage::default();
            image_widget.render(area, buf, &mut async_state);
        }
    }
}