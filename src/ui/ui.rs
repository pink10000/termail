use ratatui::{
    buffer::Buffer, 
    layout::{Alignment, Constraint, Direction, Layout, Rect}, 
    style::{Color, Modifier, Style, Stylize}, 
    text::{Line, Span}, 
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Widget}
};

use crate::{
    types::{EmailMessage, EmailSender}, 
    ui::app::{ActiveViewState, App, BaseViewState, ComposeViewState, ComposeViewField}
};
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
    middle: Rect,
    bottom_bar: Rect,
}

impl App {
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
    
    fn render_folder_pane(&self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, ActiveViewState::BaseView(BaseViewState::Labels));
        
        let block = Block::default()
            .title("Folders")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
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
        let is_active = matches!(self.state, ActiveViewState::BaseView(BaseViewState::Inbox));
        
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
                    let from = &email.from;
                    let subject = &email.subject;
                    let line = Line::from(vec![
                        Span::styled(format!("{:<25.25}", from), Style::default().fg(Color::Cyan)),
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

    fn render_message_pane(
        &self,
        area: Rect,
        buf: &mut Buffer,
        email_body: &String,
        email_from: &EmailSender,
        scroll: u16,
    ) {
        let block = Block::default()
            .title(email_from.display_name())
            .title(email_from.formatted_email())
            .title_style(if matches!(self.state, ActiveViewState::MessageView(_)) {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            })
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White));
        
        let paragraph = Paragraph::new(email_body.clone())
            .fg(Color::White)
            .wrap(ratatui::widgets::Wrap { trim: false }) 
            .scroll((scroll, 0))
            .block(block);
        
        paragraph.render(area, buf);
    }

    /// Renders the compose pane.
    /// 
    /// The compose pane is a vertical layout with a header and a body. The 
    /// header is a horizontal layout with a label and input field. There are
    /// two of these fields (extensible, to add CC and BCC fields).
    /// 
    /// The body is a vertical layout with text from the temporary file.
    fn render_compose_pane(
        &self,
        area: Rect,
        buf: &mut Buffer,
        compose_state: &ComposeViewState
    ) {
        let selected_field = &compose_state.current_field;

        let top_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // 2 for borders, 2 for subject, to
                Constraint::Min(3), // 2 for border, 1 for body
                ])
            .split(area);
        
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if *selected_field == ComposeViewField::To || *selected_field == ComposeViewField::Subject {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::White)
            });

        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // To field
                Constraint::Length(1), // Subject Field
            ])
            .split(header_block.inner(top_layout[0]));
        header_block.render(top_layout[0], buf);
        
        // Helper to split a row into [Label, Input]
        let split_row = |rect: Rect| {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(10), Constraint::Min(1)])
                .split(rect)
        };
        
        let to_area = split_row(header_layout[0]);
        let subject_area = split_row(header_layout[1]);

        Paragraph::new("To:")
            .alignment(Alignment::Right)
            .style(if matches!(selected_field, ComposeViewField::To) { 
                Style::new().fg(Color::Magenta)
            } else { 
                Style::new().fg(Color::White)
            })
            .add_modifier(Modifier::BOLD)
            .render(to_area[0], buf);

        Paragraph::new(compose_state.draft.to.clone())
            .alignment(Alignment::Left)
            .render(to_area[1], buf);

        Paragraph::new("Subject:")
            .alignment(Alignment::Right)
            .style(if matches!(selected_field, ComposeViewField::Subject) { 
                Style::new().fg(Color::Magenta)
            } else { 
                Style::new().fg(Color::White)
            })
            .add_modifier(Modifier::BOLD)
            .render(subject_area[0], buf);

        Paragraph::new(compose_state.draft.subject.clone())
            .alignment(Alignment::Left)
            .render(subject_area[1], buf);

        let body_layout = top_layout[1];
        let content = match (compose_state.draft.body.is_empty(), &compose_state.current_field) {
            (true, ComposeViewField::Body) => {
                format!("Press [Enter] to enter {} to write email.", self.config.termail.editor)
            },
            (true, _) => format!("Select to begin writing email body."),
            (false, _) => compose_state.draft.body.clone(),
        };

        let body_block = Block::default()
            .title("Body")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if matches!(compose_state.current_field, ComposeViewField::Body) {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            });

        Paragraph::new(content)
            .block(body_block)
            .render(body_layout, buf);
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
            ActiveViewState::BaseView(_) => {
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
                self.render_folder_pane(middle_layout[0], buf);
                self.render_email_list_pane(middle_layout[1], buf);
            },
            ActiveViewState::MessageView(view_state) => {
                let email = self.selected_email_index
                    .and_then(|index| self.emails.as_ref()?.get(index))
                    .cloned()
                    .unwrap_or_else(EmailMessage::new);

                self.render_top_bar(layouts.top_bar, buf, email.subject.clone());
                self.render_message_pane(
                    layouts.middle,
                    buf,
                    &email.body,
                    &email.from,
                    view_state.scroll,
                );
            },
            ActiveViewState::ComposeView(compose_state) => {
                self.render_top_bar(layouts.top_bar, buf, "Compose Email".to_string());
                self.render_compose_pane(layouts.middle, buf, compose_state);
            },
        }
    }
}
