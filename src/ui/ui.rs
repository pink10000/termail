use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Direction, Layout, Rect}, 
    style::{Color, Style, Stylize}, 
    widgets::{Block, Borders, Paragraph, Widget}
};

use crate::ui::app::App;

impl Widget for &App {
    /// Renders the user interface widgets.
    /// 
    /// The size of the layout should eventually be controlled by the config. 
    fn render(self, area: Rect, buf: &mut Buffer) {
        // top bar, middle section, bottom bar 
        // top + bottom bar = 2 rows

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(2),
                Constraint::Percentage(100 - 6),
                Constraint::Length(2),
            ])
            .split(area);
        
        let top_bar = main_layout[0];
        let middle_section = main_layout[1];
        let bottom_bar = main_layout[2];

        let top_bar_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::White));
        let top_bar_paragraph = Paragraph::new("Top Bar")
            .block(top_bar_block)
            .fg(Color::White)
            .centered();
        top_bar_paragraph.render(top_bar, buf);

        let bottom_bar_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::White));
        let bottom_bar_paragraph = Paragraph::new("Bottom Bar")
            .block(bottom_bar_block)
            .fg(Color::White)
            .centered();
        bottom_bar_paragraph.render(bottom_bar, buf);

        let folder_pane_width: u16 = 20;
        let middle_section_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(folder_pane_width),
                // Border 
                Constraint::Length(1),
                Constraint::Percentage((100 - folder_pane_width) / 2),
                // Border
                Constraint::Length(1), 
                Constraint::Percentage((100 - folder_pane_width) / 2),
            ])
            .split(middle_section);
        
        let border_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::White))
            .fg(Color::White);
        border_block.render(middle_section_layout[1], buf);
        
        let border_block2 = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::White))
            .fg(Color::White);
        border_block2.render(middle_section_layout[3], buf);

        let folder_pane = middle_section_layout[0];
        let folder_block = Block::default()
            .title("Folder Pane");
        let folder_paragraph = Paragraph::new("Folders")
            .block(folder_block)
            .fg(Color::White)
            .centered();
        
        // TODO: should call the internal API to get the email list
        // TODO: special email widget
        // TODO: should be scrollable, selectable, and clickable
        let email_pane = middle_section_layout[2];
        let email_list = vec![
            "Email 1",
            "Email 2",
            "Email 3",
            "Email 4",
            "Email 5",
        ];
        let email_list_block = Block::default()
            .title("Emails");
        let email_list_paragraph = Paragraph::new(email_list.join("\n"))
            .block(email_list_block)
            .fg(Color::White)
            .centered();
        
        let message_pane = middle_section_layout[4];
        let message_text = "This is a test message.";
        let message_block = Block::default()
            .title("Message");
        let message_paragraph = Paragraph::new(message_text)
            .block(message_block)
            .fg(Color::White)
            .centered();

        folder_paragraph.render(folder_pane, buf);
        email_list_paragraph.render(email_pane, buf);
        message_paragraph.render(message_pane, buf);

    }
}
