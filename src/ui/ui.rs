use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Direction, Layout, Rect}, 
    style::{Color, Style, Stylize, Modifier}, 
    widgets::{Block, Borders, Paragraph, Widget}
};

use crate::ui::app::{App, ActiveViewState};

/// Layout structure containing all UI component rectangles
struct AppLayouts {
    top_bar: Rect,
    bottom_bar: Rect,
    folder_pane: Rect,
    email_pane: Rect,
    message_pane: Rect,
    border1: Rect,
    border2: Rect,
}

impl App {
    /// Calculate all layout rectangles for the UI
    fn create_layouts(area: Rect) -> AppLayouts {
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
        let middle_section = main_layout[1];
        let bottom_bar = main_layout[2];

        // Middle section: folder | border | emails | border | message
        // TODO: find a way to pull this from the `Config`
        const FOLDER_PANE_WIDTH: u16 = 20;
        let middle_section_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(FOLDER_PANE_WIDTH),
                Constraint::Length(1),  // Border
                Constraint::Percentage((100 - FOLDER_PANE_WIDTH) / 2),
                Constraint::Length(1),  // Border
                Constraint::Percentage((100 - FOLDER_PANE_WIDTH) / 2),
            ])
            .split(middle_section);
        
        AppLayouts {
            top_bar,
            bottom_bar,
            folder_pane: middle_section_layout[0],
            border1: middle_section_layout[1],
            email_pane: middle_section_layout[2],
            border2: middle_section_layout[3],
            message_pane: middle_section_layout[4],
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
        
        let content = self.selected_folder.clone();
        let paragraph = Paragraph::new(content)
            .block(block)
            .fg(Color::White);
        
        paragraph.render(area, buf);
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
        let subject_max_length: usize = width.saturating_sub(from_max_length + 2); // +2 for "> " prefix

        let content = match &self.emails {
            None => "Loading...".to_string(),
            Some(emails) if emails.is_empty() => "No emails found".to_string(),
            Some(emails) => {
                emails.iter()
                    .map(|email| {
                        let from = if email.from.len() > from_max_length {
                            format!("{}...", &email.from[0..(from_max_length - 3)])
                        } else {
                            email.from.clone()
                        };
                        
                        let subject = if email.subject.len() > subject_max_length {
                            format!("{}...", &email.subject[0..(subject_max_length - 3)])
                        } else {
                            email.subject.clone()
                        };
                        
                        
                        format!("{}: {}", from, subject)
                    })
                    .collect::<Vec<String>>()
                    .join("\n")
            }
        };
        
        let paragraph = Paragraph::new(content)
            .block(block)
            .fg(Color::White);
        
        paragraph.render(area, buf);
    }

    fn render_message_pane(&self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, ActiveViewState::MessageView);
        
        let block = Block::default()
            .title("Message")
            .title_style(if is_active {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            });
        
        let content = match &self.emails {
            None => "Loading...".to_string(),
            Some(emails) if emails.is_empty() => "No message selected".to_string(),
            Some(emails) => {
                // Show the selected email based on selected_email_index
                if let Some(index) = self.selected_email_index {
                    if index < emails.len() {
                        let email = &emails[index];
                        format!(
                            "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
                            email.from,
                            email.to,
                            email.date,
                            email.subject,
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
        
        let paragraph = Paragraph::new(content)
            .block(block)
            .fg(Color::White);
        
        paragraph.render(area, buf);
    }

}

impl Widget for &App {
    /// Renders the user interface widgets.
    /// 
    /// The size of the layout should eventually be controlled by the config. 
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate all layout rectangles
        let layouts = App::create_layouts(area);
        
        // Render all components
        self.render_top_bar(layouts.top_bar, buf);
        self.render_bottom_bar(layouts.bottom_bar, buf);
        self.render_folder_pane(layouts.folder_pane, buf);
        self.render_email_list_pane(layouts.email_pane, buf);
        self.render_message_pane(layouts.message_pane, buf);
        
        // Render borders
        self.render_vertical_border(layouts.border1, buf);
        self.render_vertical_border(layouts.border2, buf);
    }
}
