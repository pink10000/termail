use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, StatefulWidget},
};
use std::cell::RefCell;
use crate::core::email::EmailMessage;
use ratatui_image::{StatefulImage, thread::ThreadProtocol};

#[derive(Clone, Debug)]
pub struct Messager {
    pub email: EmailMessage,
    /// Vertical scroll offset (in lines) for the message view
    /// To be more precise, scroll is the number of lines from the top of
    /// the content of the email.
    pub scroll: u16,
    /// The height of the Paragraph widget
    /// We wrap with a RefCell to allow for mutable access from the render function.
    /// (text_height, attachment_height)
    content_height: RefCell<Option<(u16, u16)>>,
    /// The width of the view. Primarily used for calculating the content height.
    /// We wrap with a RefCell to allow for mutable access from the render function.
    view_width: RefCell<Option<u16>>,
    /// The height of the view. Used to determine the maximum scroll offset.
    view_height: RefCell<Option<u16>>,
}

impl Messager {
    pub fn new(email: EmailMessage) -> Self {
        Self {
            email,
            scroll: 0,
            content_height: RefCell::new(None),
            view_width: RefCell::new(None),
            view_height: RefCell::new(None),
        }
    }

    /// Calculate the total height of the content and attachment
    /// # Arguments
    /// * `width` - The width of the view.
    /// * `attachment_height` - The height of the attachment. If None, then no attachments are present.
    /// # Returns
    /// * `(text_height, attachment_height)` - The total height of the content and attachment.
    fn calculate_total_height(&self, width: u16, attachment_height: Option<u16>) -> (u16, u16) {
        let content_height = self.email.body
            .lines()
            .map(|line| line.chars().count() / width as usize + 1) // +1 for the \n
            .sum::<usize>() as u16;
        if attachment_height.is_some() {
            (content_height, attachment_height.unwrap())
        } else {
            (content_height, 0)
        }
    }

    /// Update the content height of the Messager
    ///
    /// # Arguments
    /// * `attachment_height` - The height of the attachment. If None, then no attachments are present.
    fn update_content_height(&self, attachment_height: Option<u16>) {
        let width = self.view_width.borrow().unwrap();
        let (text_height, attachment_height) = self.calculate_total_height(width, attachment_height);
        self.content_height.replace(Some((text_height, attachment_height)));
    }

    /// Scroll down by one line, clamped to content bounds.
    ///
    /// Uses content height from the last render to determine maximum scroll.
    pub fn scroll_down(&mut self) {
        let (text_h, attach_h) = self.content_height.borrow().unwrap_or((0, 0));
        let view_h = self.view_height.borrow().unwrap_or(0);
        let total_content_height = text_h.saturating_add(attach_h);
        let max_scroll = total_content_height.saturating_sub(view_h);
        self.scroll = self.scroll.saturating_add(1).clamp(0, max_scroll);
    }
    pub fn scroll_up(&mut self) {
        let (text_h, attach_h) = self.content_height.borrow().unwrap_or((0, 0));
        let view_h = self.view_height.borrow().unwrap_or(0);
        let total_content_height = text_h.saturating_add(attach_h);
        let max_scroll = total_content_height.saturating_sub(view_h);
        self.scroll = self.scroll.saturating_sub(1).clamp(0, max_scroll);
    }

    /// Render the message view with images
    /// Currently only supports one image attachment.
    pub fn render_with_images(
        &self,
        area: Rect,
        buf: &mut Buffer,
        image_state: &mut Option<ThreadProtocol>
    ) {
        self.view_width.replace(Some(area.width));
        self.view_height.replace(Some(area.height));
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

        let attachment_height = if image_state.is_some() { 20 } else { 0 };
        let (text_height, _) = self.calculate_total_height(inner_area.width, Some(attachment_height));
        self.update_content_height(Some(attachment_height));

        Paragraph::new(self.email.body.as_str())
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((self.scroll, 0))
            .render(inner_area, buf);

        if let Some(protocol) = image_state {
            // Calculate where the image starts relative to the viewport top
            // `logical_y` can be negative if the image is scrolled partially off the top
            //
            // One way to think about this is that `logical_y` is the number of lines
            // you need to scroll down (which increases self.scroll) until the end
            // end of the message body (text_height) is reached.
            //
            // Then, when you have scrolled more than the text height, logical_y will be negative.
            // This is room for termail to draw the image, which is calculated by
            // `logical_y + attachment_height`.
            let logical_y = (text_height as i32) - (self.scroll as i32);

            // Check if any part of the image is visible in the viewport
            if logical_y < inner_area.height as i32 && (logical_y + attachment_height as i32) > 0 {
                let render_y_offset = logical_y.max(0) as u16;
                let scrolled_off_top = logical_y.min(0).abs() as u16;
                let image_area = Rect {
                    x: inner_area.x,
                    y: inner_area.y + render_y_offset,
                    width: inner_area.width,
                    // Height is reduced if scrolled off top, and clamped to container bottom
                    height: attachment_height
                        .saturating_sub(scrolled_off_top)
                        .min(inner_area.height.saturating_sub(render_y_offset)),
                };

                StatefulWidget::render(
                    StatefulImage::default(),
                    image_area,
                    buf,
                    protocol
                );
            }
        } else {
            let paragraph = Paragraph::new(email_body.to_string())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(inner_area, buf);
            self.update_content_height(None);
        }
    }
}