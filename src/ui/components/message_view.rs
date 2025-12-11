use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    prelude::{Constraint, Direction, Layout, StatefulWidget},
};
use std::cell::RefCell;
use crate::core::email::EmailMessage;
use ratatui_image::{StatefulImage, thread::ThreadProtocol};

#[derive(Clone, Debug)]
pub struct Messager {
    pub email: EmailMessage,
    /// Vertical scroll offset (in lines) for the message view
    pub scroll: u16,
    /// The height of the Paragraph widget
    /// We wrap with a RefCell to allow for mutable access from the render function.
    content_height: RefCell<Option<u16>>,
    /// The width of the view. Primarily used for calculating the content height.
    /// We wrap with a RefCell to allow for mutable access from the render function.
    view_width: RefCell<Option<u16>>,
}

impl Messager {
    pub fn new(email: EmailMessage) -> Self {
        Self {
            email,
            scroll: 0,
            content_height: RefCell::new(None),
            view_width: RefCell::new(None),
        }
    }

    fn calculate_total_height(&self, width: u16, attachment_height: u16, has_attachments: bool) -> u16 {
        let content_height = self.email.body
            .lines()
            .map(|line| line.chars().count() / width as usize + 1) // +1 for the \n
            .sum::<usize>() as u16;
        if has_attachments {
            content_height + attachment_height
        } else {
            content_height
        }
    }

    fn update_content_height(&self, attachment_height: Option<u16>) {
        let width = self.view_width.borrow().unwrap();
        let height = self.calculate_total_height(
            width,
            attachment_height.unwrap_or(0),
            attachment_height.is_some()
        );
        self.content_height.replace(Some(height));
    }

    /// This function changes the scroll offset of the Messager
    /// Since ratatui's `Paragraph` widget does not limit how far we can scroll down,
    /// scroll down, we need to use the height of the Paragraph widget.
    ///
    /// Note that the value 15 is arbitrary, and can be changed to any value.
    /// TODO: Make this configurable by the config.toml file OR a way to determine
    /// the height without knowing the UI layout.
    pub fn scroll_down(&mut self) {
        let content_height = self.content_height.borrow().unwrap_or(0);
        let max_scroll = content_height.saturating_sub(15);
        self.scroll = self.scroll.saturating_add(1).clamp(0, max_scroll);
    }
    pub fn scroll_up(&mut self) {
        let content_height = self.content_height.borrow().unwrap_or(0);
        let max_scroll = content_height.saturating_sub(15);
        self.scroll = self.scroll.saturating_sub(1).clamp(0, max_scroll);
    }

    pub fn render_with_images(
        &self,
        area: Rect,
        buf: &mut Buffer,
        image_state: &mut Option<ThreadProtocol>
    ) {
        self.view_width.replace(Some(area.width));
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
        // At the end, update the content height to indicate maximum scroll offset.
        if let Some(mut async_state) = image_state.as_mut() {
            let attachment_height = 20;
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),      // Text area (flexible, minimum 5 lines)
                    Constraint::Length(attachment_height),  // Image area (fixed 20 lines)
                ])
                .split(inner_area);

            let paragraph = Paragraph::new(email_body.to_string())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(chunks[0], buf);
            let image_widget: StatefulImage<ThreadProtocol> = StatefulImage::default();
            image_widget.render(chunks[1], buf, &mut async_state);
            self.update_content_height(Some(attachment_height));
        } else {
            let paragraph = Paragraph::new(email_body.to_string())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(inner_area, buf);
            self.update_content_height(None);
        }
    }
}