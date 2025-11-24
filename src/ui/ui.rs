use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Direction, Layout, Rect}, 
    style::{Color, Style, Stylize, Modifier}, 
    widgets::{Block, Borders, Paragraph, Widget, List, ListItem, ListState},
    text::{Line, Span}
};

use crate::ui::app::{App, ActiveViewState};
use crate::types::Label;

/// Helper function to create a ListItem from a Label
fn create_label_item(label: &Label) -> ListItem<'static> {
    let name = label.name.as_ref().map(|s| s.as_str()).unwrap_or("Unknown");

    if label.messages_total.is_none() || label.messages_unread.is_none() {
        return ListItem::new(name.to_string());
    }

    // let unread = label.messages_unread.unwrap();
    // let total = label.messages_total.unwrap();
    
    // Format: "LabelName (unread/total)"
    // let label_text = if unread > 0 {
        // format!("{} ({}/{})", name, unread, total)
    // } else {
        // format!("{} ({})", name, total)
    // };
    let label_text = format!("{}", name);

    // Create styled text with color indicator if available
    let line = if label.color.is_some() {
        // If label has a color, add a colored indicator
        Line::from(vec![
            Span::styled("● ".to_string(), Style::default().fg(Color::Cyan)),
            Span::raw(label_text),
        ])
    } else {
        Line::from(label_text)
    };
    
    ListItem::new(line)
}

/// Layout structure containing all UI component rectangles
struct AppLayouts {
    top_bar: Rect,
    bottom_bar: Rect,
    middle_section: Rect,
    folder_pane: Rect,
    email_pane: Rect,
    message_pane: Rect,
    border1: Rect,
    border2: Rect,
}

impl App {
    /// Calculate the optimal folder pane width based on loaded labels
    /// Returns the width in characters, or None if labels aren't loaded yet
    fn calculate_folder_pane_width(&self) -> Option<u16> {
        self.labels.as_ref().and_then(|labels| {
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
        })
    }

    /// Calculate all layout rectangles for the UI
    fn create_layouts(&self, area: Rect) -> AppLayouts {
        // Main vertical layout: top bar, middle section, bottom bar
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(2), 
                Constraint::Length(area.height - 4),
                Constraint::Length(2),
            ])
            .split(area);
        
        let top_bar = main_layout[0];
        let middle_section_container = main_layout[1];
        let bottom_bar = main_layout[2];

        // Determine folder pane width (default to 20 if labels not loaded)
        let folder_pane_width = self.calculate_folder_pane_width().unwrap_or(20);

        // Middle section: folder | border | emails | border | message
        let horizontal_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(folder_pane_width),  // Fixed width based on content
                Constraint::Length(1),  // Border
                Constraint::Min(0),
            ])
            .split(middle_section_container);
        
        let folder_pane = horizontal_split[0];
        let border1 = horizontal_split[1];
        let content_area = horizontal_split[2];

        // CONTENT: Split Email List vs Message Pane
        let content_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(50), 
                Constraint::Length(1),
                Constraint::Percentage(50), 
            ])
            .split(content_area);

        AppLayouts {
            top_bar,
            bottom_bar,
            middle_section: middle_section_container,
            folder_pane,
            border1,
            email_pane: content_split[0],
            border2: content_split[1],
            message_pane: content_split[2],
        }
    }

    fn render_top_bar(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::White));
        
        let text = format!("termail - {}", self.config.termail.default_backend);
        let paragraph = Paragraph::new(text)
            .block(block)
            .fg(Color::White)
            .centered();
        
        paragraph.render(area, buf);
    }

    fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::TOP)
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

    fn render_vertical_border(&self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::White))
            .fg(Color::White)
            .render(area, buf);
    }
    
    fn render_folder_pane(&self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, ActiveViewState::FolderView);
        
        let block = Block::default()
            .title("Folders")
            .title_style(if is_active {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            });
        
        // Create list items from labels
        let items: Vec<ListItem> = match &self.labels {
            None => {
                // Labels not loaded yet
                vec![ListItem::new("Loading labels...")]
            }
            Some(labels) if labels.is_empty() => {
                // No labels found
                vec![ListItem::new("No labels found")]
            }
            Some(labels) => {
                // Create a list item for each label using our reusable component
                labels.iter().map(create_label_item).collect()
            }
        };
        
        let list = List::new(items)
            .block(block)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            );
        
        list.render(area, buf);
    }

    fn render_email_list_pane(&self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, ActiveViewState::InboxView);
        
        let block = Block::default()
            .title("Emails")
            .title_style(if is_active {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            });
        
        let width = area.width as usize;
        let from_max_length: usize = 20;
        let _subject_max_length: usize = width.saturating_sub(from_max_length + 2); // +2 for "> " prefix

        // Create list items (each email = one row)
        let items: Vec<ListItem> = match &self.emails {
            None => vec![ListItem::new("Loading...")],
            Some(emails) if emails.is_empty() => vec![ListItem::new("No emails found")],
            Some(emails) => emails
                .iter()
                .map(|email| {
                    let from = &email.from;
                    let subject = &email.subject;
                    let line = Line::from(vec![
                        Span::styled(format!("{:.20}", from), Style::default().fg(Color::Cyan)),
                        Span::raw(" "),
                        Span::styled(subject, Style::default().fg(Color::White)),
                    ]);
                    ListItem::new(line)
                })
                .collect(),
        };

        let list = List::new(items)
            .block(block)
            .highlight_symbol("▶ ") 
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(if is_active { Color::Blue } else { Color::DarkGray })
                    .add_modifier(Modifier::BOLD),
            );

        // Manage which email is selected
        let mut state = ListState::default();
        state.select(self.selected_email_index);

        // Render with highlight state
        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut state);
    }

    /// This function formats the email header for display in the message pane by 
    /// aligning the fields to the left along the `:` character.
    fn format_email_header(&self, from: &str, to: &str, date: &str, subject: &str) -> String {
        // The longest label is "Subject" (7 chars). We use 8 for PAD to include one space 
        // between the label and the colon.
        const PAD: usize = 8; 
        format!(
            "{:>PAD$}: {}\n{:>PAD$}: {}\n{:>PAD$}: {}\n{:>PAD$}: {}",
            "From", from, "To", to, "Date", date, "Subject", subject,
        )
    }

    fn render_message_pane(&self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Clear.render(area, buf);
        let block = Block::default()
            .title("Messages")
            .title_style(if matches!(self.state, ActiveViewState::MessageView) {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            })
            .style(Style::default().bg(Color::Reset));
        block.render(area, buf);

        let content = match &self.emails {
            None => "Loading messages...".to_string(),
            Some(emails) if emails.is_empty() => "No message selected".to_string(),
            Some(emails) => {
                // Show the selected email based on selected_email_index
                if let Some(index) = self.selected_email_index {
                    if index < emails.len() {
                        let email = &emails[index];
                        // Include both header and body
                        format!(
                            "{}\n\n{}",
                            self.format_email_header(&email.from, &email.to, &email.date, &email.subject),
                            email.body
                        )
                    } else {
                        "No message selected".to_string()
                    }
                } else {
                    "No message selected".to_string()
                }
            }
        };
        // need to reset the message pane since the next email may have a different length
        // and may leave artifacts 
        // ratatui::widgets::Clear.render(area, buf);

        let paragraph = Paragraph::new(content)
            .fg(Color::White)
            .wrap(ratatui::widgets::Wrap { trim: false }); // Needs to be set false to ensure `format_email_header` works correctly.
        
        paragraph.render(area, buf);
    }

}

impl Widget for &App {
    /// Renders the user interface widgets.
    /// 
    /// The size of the layout should eventually be controlled by the config. 
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate all layout rectangles
        let layouts = self.create_layouts(area);
        
        // Render all components
        self.render_top_bar(layouts.top_bar, buf);
        self.render_bottom_bar(layouts.bottom_bar, buf);
        ratatui::widgets::Clear.render(layouts.middle_section, buf);
        self.render_folder_pane(layouts.folder_pane, buf);
        self.render_email_list_pane(layouts.email_pane, buf);
        self.render_message_pane(layouts.message_pane, buf);
        
        // Render borders
        self.render_vertical_border(layouts.border1, buf);
        self.render_vertical_border(layouts.border2, buf);
    }
}
