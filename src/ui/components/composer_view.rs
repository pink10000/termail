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
    fn render(self, area: Rect, buf: &mut Buffer) {
        let selected_field = &self.state.current_field;

        // This captures 'selected_field' so we don't have to pass it every time
        let get_style = |target: ComposeViewField| -> Style {
            if *selected_field == target {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            }
        };

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header height
                Constraint::Min(3),    // Body height
            ])
            .split(area);

        self.render_header(main_layout[0], buf, &get_style);
        self.render_body(main_layout[1], buf, get_style(ComposeViewField::Body));
    }
}

impl<'a> Composer<'a> {
    /// Renders the header section containing To and Subject fields.
    fn render_header(
        &self,
        area: Rect, 
        buf: &mut Buffer, 
        get_style: &impl Fn(ComposeViewField) -> Style
    ) {
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
        
        self.render_row(header_layout[0], buf, "To:", &self.state.draft.to, get_style(ComposeViewField::To));
        self.render_row(header_layout[1], buf, "Subject:", &self.state.draft.subject, get_style(ComposeViewField::Subject));
    }

    /// Renders a single field row with label and input value.
    /// 
    /// The row is split into a label area and an input area.
    fn render_row(&self, area: Rect, buf: &mut Buffer, label: &str, value: &str, style: Style) {
        // Split row into [Label, Input]
        let row_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(10), Constraint::Min(1)])
            .split(area);
        
        let label_area = row_layout[0];
        let input_area = row_layout[1];

        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(style)
            .render(label_area, buf);

        Paragraph::new(value)
            .alignment(Alignment::Left)
            .render(input_area, buf);
    }

    /// Renders the body section of the compose view.
    /// 
    /// Shows either placeholder text or the actual email body content.
    fn render_body(&self, area: Rect, buf: &mut Buffer, style: Style) {
        // Determine body content based on state
        let content = match (self.state.draft.body.is_empty(), &self.state.current_field) {
            (true, ComposeViewField::Body) => {
                format!("Press [Enter] to enter {} to write email.", self.editor_name)
            },
            (true, _) => "Select to begin writing email body.".to_string(),
            (false, _) => self.state.draft.body.clone(),
        };

        // Determine border style based on whether body field is selected
        let body_block = Block::default()
            .title("Body")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(style);

        Paragraph::new(content)
            .block(body_block)
            .render(area, buf);
    }


}