use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Widget},
};
use crate::core::label::Label;
use crate::ui::app::BaseViewState;

pub struct FolderPane<'a> {
    /// Reference to the list of labels. None implies loading state.
    pub labels: Option<&'a Vec<Label>>,
    /// Whether the user focus is currently on this pane.
    pub state: &'a BaseViewState,
    /// Currently selected folder name for highlighting.
    pub selected_folder: &'a str,
}

impl<'a> Widget for FolderPane<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let is_active = matches!(self.state, BaseViewState::Labels);
        
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
            .highlight_symbol("▶ ")
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(if is_active { Color::Blue } else { Color::DarkGray })
                    .add_modifier(Modifier::BOLD)
            );
        
        // Determine selected folder index for highlighting
        let selected_index = self.labels.and_then(|labels| {
            labels
                .iter()
                .enumerate()
                .find_map(|(idx, label)| {
                    label
                        .name
                        .as_deref()
                        .and_then(|name| (name == self.selected_folder).then_some(idx))
                })
        });

        let mut state = ListState::default();
        state.select(selected_index);

        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut state);
    }
}

/// Helper function to create a ListItem from a Label
pub fn create_label_item(label: &Label) -> ListItem<'static> {
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