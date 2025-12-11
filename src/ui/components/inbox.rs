use ratatui::{
    buffer::Buffer, 
    layout::Rect, 
    style::{Color, Modifier, Style}, 
    text::{Line, Span}, 
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Widget}
};

use crate::{
    core::email::EmailMessage,
    ui::app::BaseViewState,
};

pub struct Inbox<'a> {
    pub emails: Option<&'a Vec<EmailMessage>>,
    pub selected_index: Option<usize>,
    pub state: &'a BaseViewState,
}

impl<'a> Widget for Inbox<'a> {
    /// Renders the Inbox view of the BaseView state.
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
        let from_max_length: usize = 20;
        let _subject_max_length: usize = width.saturating_sub(from_max_length + 2); // +2 for "> " prefix
    
        // Create list items (each email = one row)
        let items: Vec<ListItem> = match &self.emails {
            None => vec![ListItem::new("Loading...")],
            Some(emails) if emails.is_empty() => vec![ListItem::new("No emails found")],
            Some(emails) => emails
                .iter()
                .map(|email| {
                    let subject_style = if email.is_new {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let from_style = if email.is_new {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    };
                    let from = &email.from;
                    let subject = &email.subject;
                    let line = Line::from(vec![
                        Span::styled(format!("{:<25.25}", from), from_style),
                        Span::raw(" "),
                        Span::styled(subject, subject_style),
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
        state.select(self.selected_index);
    
        // Render with highlight state
        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut state);
    }
}