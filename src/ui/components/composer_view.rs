use crate::ui::app::{ComposeViewState, ComposeViewField};
use ratatui::{
    buffer::Buffer,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
    style::{Color, Modifier, Style},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
};

pub struct Composer<'a> {
    pub state: &'a ComposeViewState,
    pub editor_name: &'a str, 
}

impl<'a> Widget for Composer<'a> {
    /// Renders the compose pane.
    /// 
    /// The compose pane is a vertical layout with a header and a body. The 
    /// header is a horizontal layout with a label and input field. There are
    /// two of these fields (extensible, to add CC and BCC fields).
    /// 
    /// The body is a vertical layout with text from the temporary file.
    fn render(mut self, area: Rect, buf: &mut Buffer) {
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

impl<'a> Composer<'a> {
    fn is_selected(&self, target: &ComposeViewField) -> bool {
        self.state.current_field == *target
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
    fn render_header(&mut self,area: Rect, buf: &mut Buffer) {
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if matches!(self.state.current_field, ComposeViewField::To | ComposeViewField::Subject) {
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
            "To:", &self.state.draft.to, 
            ComposeViewField::To
        );
        self.render_row(header_layout[1], 
            buf, 
            "Subject:", &self.state.draft.subject, 
            ComposeViewField::Subject
        );
    }

    /// Renders a single field row with label and input value.
    /// 
    /// The row is split into a label area and an input area.
    fn render_row(
        &mut self, 
        area: Rect, 
        buf: &mut Buffer, 
        label: &str, 
        value: &str, 
        field_repr: ComposeViewField,
    ) {
        let style = self.get_selection_style(&field_repr);

        // Split row into [Label, Input]
        let row_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Min(1)])
            .split(area);
        
        let label_area = row_layout[0];
        let input_area = row_layout[1];

        // Add borders so it looks like a proper input field
        let input_block = Block::default().style(style);

        let visible_len = row_layout[1].width.saturating_sub(2).max(20) as usize;
        let display_value = format!("[{:_<width$.prec$}]", value, width = visible_len, prec = visible_len);

        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(style)
            .render(label_area, buf);
       
        Paragraph::new(display_value)
            .alignment(Alignment::Left)
            .block(input_block)
            .render(input_area, buf);
    }

    /// Renders the body section of the compose view.
    /// 
    /// Shows either placeholder text or the actual email body content.
    fn render_body(&self, area: Rect, buf: &mut Buffer) {
        // Determine body content based on state
        let content = match (self.state.draft.body.is_empty(), &self.state.current_field) {
            (true, ComposeViewField::Body) => {
                format!("Press [Enter] to enter {} to write email.", self.editor_name)
            },
            (true, _) => "Select to begin writing email body.".to_string(),
            (false, _) => self.state.draft.body.clone(),
        };

        let body_block = Block::default()
            .title("Body")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.get_selection_style(&ComposeViewField::Body));

        Paragraph::new(content)
            .block(body_block)
            .render(area, buf);
    }


}