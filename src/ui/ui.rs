use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Direction, Layout, Rect}, 
    style::{Color, Style, Stylize}, 
    widgets::{Block, BorderType, Borders, Paragraph, Widget}
};

use crate::{
    types::EmailMessage, 
    ui::app::{ActiveViewState, App},
    ui::components::{composer_view::Composer, folder_pane::FolderPane, inbox::Inbox, message_view::MessageView},
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

    fn render_top_bar(&self, area: Rect, buf: &mut Buffer, text: String) {
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

    fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
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
    fn calculate_folder_pane_width(&self) -> u16 {
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

impl Widget for &App {
    /// Renders the user interface widgets.
    /// 
    /// The size of the layout should eventually be controlled by the config. 
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate all layout rectangles for the base view
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
                FolderPane {
                    labels: self.labels.as_ref(),
                    state: bv,
                }.render(middle_layout[0], buf);
                Inbox {
                    emails: self.emails.as_ref(),
                    selected_index: self.selected_email_index,
                    state: bv,
                }.render(middle_layout[1], buf);
            },
            ActiveViewState::MessageView(mv) => {
                let email = self.selected_email_index
                    .and_then(|index| self.emails.as_ref()?.get(index))
                    .cloned()
                    .unwrap_or_else(EmailMessage::new);

                self.render_top_bar(layouts.top_bar, buf, email.subject.clone());
                MessageView { email: &email, scroll: mv.scroll, state: mv }.render(layouts.middle, buf);
            },
            ActiveViewState::ComposeView(compose_state) => {
                self.render_top_bar(layouts.top_bar, buf, "Compose Email".to_string());
                Composer {
                    state: compose_state,
                    editor_name: &self.config.termail.editor,
                }.render(layouts.middle, buf);
            },
        }
    }
}
