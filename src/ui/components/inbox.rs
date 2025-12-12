use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Widget}
};
use chrono::DateTime;
use unicode_width::UnicodeWidthChar;

use crate::{
    core::email::EmailMessage,
    ui::app::BaseViewState,
};

pub struct Inbox<'a> {
    pub emails: Option<&'a Vec<EmailMessage>>,
    pub selected_index: Option<usize>,
    pub state: &'a BaseViewState,
}

/// Formats a date string to MM/DD/YYYY format
/// TODO: Support other date formats. They should be defined in the config.toml file.
fn format_date(date_str: &str) -> String {
    DateTime::parse_from_rfc2822(date_str)
        .map(|dt| dt.format("%m/%d/%Y").to_string())
        .unwrap_or_else(|_| "??/??/????".to_string())
}

/// Strip emojis and other wide characters from text
fn strip_emojis(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.width().unwrap_or(1) <= 1)
        .collect()
}

/// Truncate and pad string to exact visual width (handles emojis)
fn fit_to_width(text: &str, target_width: usize) -> String {
    let text = text.trim_start();
    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        // This is where `unicode_width::UnicodeWidthChar` is used.
        let ch_width = ch.width().unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    // Pad to exact width
    if current_width < target_width {
        result.push_str(&" ".repeat(target_width - current_width));
    }
    result
}

impl<'a> Widget for Inbox<'a> {
    /// Renders the Inbox view of the BaseView state.
    ///
    /// The email subjects have their emojis strip. In the future, we will
    /// support displaying emojis in the subject.
    fn render(self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, BaseViewState::Inbox);
        
        let block = Block::default()
            .title("Emails")
            .title_style(if is_active {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            })
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));

        let width = area.width as usize;
        let from_max_width: usize = 20;
        let date_width: usize = 10 + 1; // MM/DD/YYYY = 10 chars + 1 space (see format_date function)
        let spacing: usize = 2; // spaces between columns
        // Calculate remaining space for subject (accounting for highlight symbol "▶ " = 2 chars)
        let subject_width: usize = width.saturating_sub(from_max_width + date_width + (spacing * 2) + 2);
    
        // Create list items (each email = one row)
        let items: Vec<ListItem> = match &self.emails {
            None => vec![ListItem::new("Loading...")],
            Some(emails) if emails.is_empty() => vec![ListItem::new("No emails found")],
            Some(emails) => emails
                .iter()
                .map(|email| {
                    let from = fit_to_width(email.from.display_name(), from_max_width);
                    let subject = fit_to_width(&strip_emojis(&email.subject), subject_width);
                    let date = format_date(&email.date);
                    
                    // Style unread emails: white and bold, read emails: dark gray
                    let from_style = if email.is_unread {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    
                    let subject_style = if email.is_unread {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    
                    ListItem::new(Line::from(vec![
                        Span::styled(from, from_style),
                        Span::raw(" "), // space between from and subject
                        Span::styled(subject, subject_style),
                        Span::raw(" "), // space between subject and date
                        Span::styled(format!("{:>width$}", date, width = date_width), Style::default().fg(Color::Green)),
                        Span::raw(" "), // space between date and border
                    ]))
                })
                .collect(),
        };
    
        let list = List::new(items)
            .block(block)
            .highlight_symbol("▶ ") 
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
    
        // Manage which email is selected
        let mut state = ListState::default();
        state.select(self.selected_index);
    
        // Render with highlight state
        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut state);
    }
}