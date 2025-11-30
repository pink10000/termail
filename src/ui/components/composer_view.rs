use ratatui::{
    buffer::Buffer,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    style::{Color, Modifier, Style},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
};
use crate::core::email::EmailMessage;

#[derive(Clone, Debug, PartialEq)]
pub enum ComposeViewField {
    To,
    Subject,
    Body,
}

#[derive(Clone, Debug)]
pub struct Composer {
    pub draft: EmailMessage,
    pub current_field: ComposeViewField,
    pub cursor_to: usize,
    pub cursor_subject: usize,
    pub editor_name: String, 
}

impl Widget for Composer {
    /// Renders the compose pane.
    /// 
    /// The compose pane is a vertical layout with a header and a body. The 
    /// header is a horizontal layout with a label and input field. There are
    /// two of these fields (extensible, to add CC and BCC fields).
    /// 
    /// The body is a vertical layout with text from the temporary file.
    fn render(self, area: Rect, buf: &mut Buffer) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header height
                Constraint::Min(3),    // Body height
            ])
            .split(area);

        self.render_header(main_layout[0], buf);
        self.render_body(main_layout[1], buf);
    }
}

impl Composer {
    fn is_selected(&self, target: &ComposeViewField) -> bool {
        self.current_field == *target
    }

    /// Returns the style for the selected field.
    fn get_selection_style(&self, target: &ComposeViewField) -> Style {
        if self.is_selected(target) {
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    }

    /// Renders the header section containing To and Subject fields.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if matches!(self.current_field, ComposeViewField::To | ComposeViewField::Subject) {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::White)
            }.add_modifier(Modifier::BOLD));

        // Split header into To and Subject rows
        let header_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // To field
                Constraint::Length(1), // Subject Field
            ])
            .split(header_block.inner(area));
        
        header_block.render(area, buf);
        
        self.render_row(
            header_layout[0], 
            buf, 
            "To: [", &self.draft.to, 
            ComposeViewField::To
        );
        self.render_row(header_layout[1], 
            buf, 
            "Subject: [", &self.draft.subject, 
            ComposeViewField::Subject
        );
    }

    /// Renders a single field row with label and input value.
    /// 
    /// The row is split into a label area and an input area.
    fn render_row(
        &self, 
        area: Rect, 
        buf: &mut Buffer, 
        label: &str, 
        value: &str, 
        field_repr: ComposeViewField,
    ) {
        let style = self.get_selection_style(&field_repr);

        // Split row into [Label, Input, Right Bracket]
        let row_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Min(3), Constraint::Length(1)])
            .split(area);
        
        let label_area = row_layout[0];
        let input_area = row_layout[1];
        let right_bracket_area = row_layout[2];
            
        // Render the right bracket. Note that the left bracket is not rendered here 
        // because it is part of the string in label.
        // It's a little hacky, but it works :P
        Paragraph::new("]")
            .alignment(Alignment::Right)
            .style(style)
            .render(right_bracket_area, buf);

        let input_block = Block::default().style(style);
        let cursor_pos = match field_repr {
            ComposeViewField::To => self.cursor_to,
            ComposeViewField::Subject => self.cursor_subject,
            _ => 0, // this should never happen, because we'll never call this function for the Body field
        };
        
        let inner_area = input_block.inner(input_area);
        let max_width = inner_area.width as usize;
        
        // Calculate scroll offset to keep cursor visible
        let scroll_offset = if cursor_pos >= max_width {
            cursor_pos.saturating_sub(max_width.saturating_sub(1))
        } else {
            0
        };
        
        let visible_text: String = value
            .chars()
            .skip(scroll_offset)
            .take(max_width)
            .collect::<String>();
        
        // Calculate cursor position in visible area
        // Note that the visible cursor is not the same as the cursor position in the full text.
        let visible_cursor = cursor_pos.saturating_sub(scroll_offset);

        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(style)
            .render(label_area, buf);
       
        Paragraph::new(visible_text.as_str())
            .alignment(Alignment::Left)
            .block(input_block)
            .render(input_area, buf);
        
        // Highlight character at cursor position
        if self.is_selected(&field_repr) && visible_cursor <= max_width {
            let cursor_x = inner_area.x + visible_cursor.min(max_width) as u16;
            let cursor_y = inner_area.y;
            if cursor_x < inner_area.x + inner_area.width && cursor_y < inner_area.y + inner_area.height {
                let cell = &mut buf[(cursor_x, cursor_y)];
                let cursor_style = style.bg(Color::Blue);
                cell.set_style(cursor_style);
            }
        }
    }

    /// Renders the body section of the compose view.
    /// 
    /// Shows either placeholder text or the actual email body content.
    fn render_body(&self, area: Rect, buf: &mut Buffer) {
        let body_block = Block::default()
            .title("Body")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.get_selection_style(&ComposeViewField::Body));
        
        // Determine body content based on state
        let content = match (self.draft.body.is_empty(), &self.current_field) {
            (true, ComposeViewField::Body) => {
                format!("Press [Enter] to enter {} to write email.", self.editor_name)
            },
            (true, _) => "Select to begin writing email body.".to_string(),
            (false, _) => self.draft.body.clone(),
        };

        Paragraph::new(content.as_str())
            .block(body_block)
            .render(area, buf);
    }
}