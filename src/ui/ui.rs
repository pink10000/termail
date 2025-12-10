use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, Borders, Paragraph, Widget}
};
use ratatui_image::StatefulImage;
use crate::{
    core::email::EmailMessage,
    ui::{
        app::{ActiveViewState, App},
        components::{folder_pane::FolderPane, inbox::Inbox}
    },
};

/// Layout structure containing all UI component rectangles
struct AppLayouts {
    top_bar: Rect,
    middle: Rect,
    bottom_bar: Rect,
}

impl App {
    /// Calculate all layout rectangles for the UI
    fn create_layouts(&self, area: Rect) -> AppLayouts {
        // Main vertical layout: top bar, middle section, bottom bar
        let top_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3), 
                Constraint::Length(area.height - 6),
                Constraint::Length(3),
            ])
            .split(area);
        
        let top_bar = top_layout[0];
        let middle = top_layout[1];
        let bottom_bar = top_layout[2];

        AppLayouts { top_bar, middle, bottom_bar }
    }

    pub fn render_top_bar(&self, area: Rect, buf: &mut Buffer, text: String) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));
        
        let paragraph = Paragraph::new(text)
            .block(block)
            .fg(Color::White)
            .centered();
        
        paragraph.render(area, buf);
    }

    pub fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));
        
        let status = match &self.emails {
            None => "Loading emails...".to_string(),
            Some(emails) => format!("{} email(s) | Press ESC to quit | Tab to cycle views", emails.len()),
        };
        
        let paragraph = Paragraph::new(status)
            .block(block)
            .fg(Color::White)
            .centered();
        
        paragraph.render(area, buf);
    }    

    /// Calculate the optimal folder pane width based on loaded labels
    /// Returns the width in characters + 2 for the borders, or 20 if labels aren't loaded yet
    pub fn calculate_folder_pane_width(&self) -> u16 {
        let max_label_len = self.labels.as_ref().and_then(|labels| {
            labels.iter()
                .filter_map(|label| {
                    // Only calculate for labels with all required fields
                    let name = label.name.as_ref()?;
                    // let unread = label.messages_unread?;
                    // let total = label.messages_total?;
                    
                    // Calculate the display width: "Name (unread/total)"
                    let width = name.len();
                    
                    Some(width)
                })
                .max()
                .map(|max_width| {
                    // Add some padding (title + borders = ~4 chars)
                    // Clamp between reasonable min/max values
                    (max_width).clamp(10, 50) as u16
                })
        });
        match max_label_len {
            Some(l) => l.saturating_add(2),  // Add 2 for the borders
            None => 20,  // Default to 20 if labels not loaded
        }
    }
}

impl App {
    /// Main render function that has access to Frame for stateful widgets
    ///
    /// Instead of using the `Widget` trait, we separate the rendering out into a
    /// separate function to allow for easy extension. This is because the `Widget` trait
    /// is not thread-safe, and we need to render the stateful widgets in a separate thread.
    /// This allows to render images.
    ///
    /// See: https://ratatui.rs/concepts/rendering/under-the-hood/
    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let layouts = self.create_layouts(area);
        self.render_bottom_bar(layouts.bottom_bar, buf);

        match &self.state {
            ActiveViewState::BaseView(bv) => {
                let text = format!("termail - {}", self.config.termail.default_backend);
                self.render_top_bar(layouts.top_bar, buf, text);

                // Middle section: folder | inbox
                let middle_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![
                        Constraint::Length(self.calculate_folder_pane_width()),  // Fixed width based on content
                        Constraint::Min(0),
                    ])
                    .split(layouts.middle);

                frame.render_widget(FolderPane {
                    labels: self.labels.as_ref(),
                    state: bv,
                }, middle_layout[0]);

                frame.render_widget(Inbox {
                    emails: self.emails.as_ref(),
                    selected_index: self.selected_email_index,
                    state: bv,
                }, middle_layout[1]);
            },
            ActiveViewState::MessageView(messager) => {
                let email = self.selected_email_index
                    .and_then(|index| self.emails.as_ref()?.get(index))
                    .cloned()
                    .unwrap_or_else(EmailMessage::new);

                self.render_top_bar(layouts.top_bar, buf, email.subject.clone());

                // Render message text
                frame.render_widget(messager.clone(), layouts.middle);

                // Render image if available (overlaid on top of message area)
                if let Some(async_state) = &mut self.async_state {
                    let image_widget: StatefulImage<ratatui_image::thread::ThreadProtocol> = StatefulImage::default();
                    frame.render_stateful_widget(
                        image_widget,
                        layouts.middle,
                        async_state
                    );
                }
            },
            ActiveViewState::ComposeView(composer) => {
                self.render_top_bar(layouts.top_bar, buf, "Compose Email".to_string());
                frame.render_widget(composer.clone(), layouts.middle);
            },
        }
    }
}
