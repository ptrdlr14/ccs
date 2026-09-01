use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::config::{self, Config, PROFILE_FIELDS};
use crate::{ACCENT, MUTED};

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Left,
    Right,
}

enum InputState {
    None,
    Editing { buffer: String, cursor: usize },
    Creating { buffer: String, cursor: usize },
}

pub struct App {
    config: Config,
    names: Vec<String>,
    profile_list: ListState,
    focus: Focus,
    field_idx: usize,
    input: InputState,
}

impl App {
    pub fn new(config: Config) -> Self {
        let mut names: Vec<String> = config.profiles.keys().cloned().collect();
        names.sort();
        let mut profile_list = ListState::default();
        if !names.is_empty() {
            profile_list.select(Some(0));
        }
        Self {
            config,
            names,
            profile_list,
            focus: Focus::Left,
            field_idx: 0,
            input: InputState::None,
        }
    }

    fn selected_index(&self) -> usize {
        self.profile_list.selected().unwrap_or(0)
    }

    fn selected_name(&self) -> Option<&str> {
        self.names.get(self.selected_index()).map(|s| s.as_str())
    }

    fn profile_next(&mut self) {
        if self.names.is_empty() {
            return;
        }
        let i = (self.selected_index() + 1) % self.names.len();
        self.profile_list.select(Some(i));
    }

    fn profile_prev(&mut self) {
        if self.names.is_empty() {
            return;
        }
        let i = if self.selected_index() == 0 {
            self.names.len() - 1
        } else {
            self.selected_index() - 1
        };
        self.profile_list.select(Some(i));
    }

    /// Visual row of the focused field in the right panel.
    ///
    /// Mirrors the row walk in `build_field_items`: one row per field plus a
    /// header per section. The read-only Env rows are deliberately excluded —
    /// they sit below the focusable range and can never be selected.
    fn field_list_idx(&self) -> usize {
        let mut visual = 0;
        let mut prev_section: Option<&str> = None;
        for (i, field) in PROFILE_FIELDS.iter().enumerate() {
            if prev_section != Some(field.section) {
                visual += 1; // section header
                prev_section = Some(field.section);
            }
            if i == self.field_idx {
                return visual;
            }
            visual += 1;
        }
        unreachable!("field_idx is always kept in 0..PROFILE_FIELDS.len()")
    }

    fn field_next(&mut self) {
        self.field_idx = (self.field_idx + 1) % PROFILE_FIELDS.len();
    }

    fn field_prev(&mut self) {
        self.field_idx = if self.field_idx == 0 {
            PROFILE_FIELDS.len() - 1
        } else {
            self.field_idx - 1
        };
    }

    pub fn into_config(self) -> Config {
        self.config
    }

    fn refresh_names(&mut self) {
        self.names.clear();
        self.names.extend(self.config.profiles.keys().cloned());
        self.names.sort();
    }
}

/// Convert a character index to a byte position in `s`.
fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Handle a key press during text editing.
/// `cursor` is a **character** index (not a byte index).
fn handle_edit_key(buffer: &mut String, cursor: &mut usize, code: KeyCode) {
    match code {
        KeyCode::Backspace => {
            if *cursor > 0 {
                let prev = *cursor - 1;
                let byte_pos = char_to_byte(buffer, prev);
                buffer.remove(byte_pos);
                *cursor = prev;
            }
        }
        KeyCode::Left => *cursor = cursor.saturating_sub(1),
        KeyCode::Right => {
            let char_count = buffer.chars().count();
            *cursor = (*cursor + 1).min(char_count);
        }
        KeyCode::Char(c) => {
            let byte_pos = char_to_byte(buffer, *cursor);
            buffer.insert(byte_pos, c);
            *cursor += 1;
        }
        _ => {}
    }
}

fn footer_span(key: &str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::default()
            .fg(Color::Black)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD),
    )
}

/// Cursor marker for the focused row.
fn bullet(focused: bool, current: bool) -> &'static str {
    if focused && current {
        " ▸"
    } else {
        "  "
    }
}

/// Section header row for the right-hand field list.
fn section_header(title: &str) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        format!("  {title} "),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )))
}

/// `label  value` row: fixed marker column, dim label, caller-styled value.
fn kv_row<'a>(marker: &'static str, label: &'a str, value: Span<'a>) -> ListItem<'a> {
    ListItem::new(Line::from(vec![
        Span::raw(marker),
        Span::styled(label, Style::default().fg(MUTED)),
        Span::raw("  "),
        value,
    ]))
}

/// Build the field list items (with section headers) for the right panel.
/// `focused` indicates whether the right panel has focus.
fn build_field_items<'a>(
    profile: &'a config::Profile,
    field_idx: usize,
    input: &InputState,
    focused: bool,
) -> Vec<ListItem<'a>> {
    let mut items: Vec<ListItem> = Vec::new();
    let mut current_section: Option<&str> = None;

    for (i, field) in PROFILE_FIELDS.iter().enumerate() {
        if current_section != Some(field.section) {
            current_section = Some(field.section);
            items.push(section_header(field.section));
        }

        let value = (field.get)(profile);
        let is_set = value.is_some();
        let value_str = value.unwrap_or_default();

        let item = match input {
            InputState::Editing { buffer, cursor } if i == field_idx => {
                let byte_pos = char_to_byte(buffer, *cursor);
                let mut display = String::with_capacity(buffer.len() + 1);
                use std::fmt::Write;
                write!(display, "{}█{}", &buffer[..byte_pos], &buffer[byte_pos..]).unwrap();
                kv_row(
                    bullet(focused, true),
                    field.label,
                    Span::styled(display, Style::default().fg(Color::Yellow)),
                )
            }
            _ => {
                let val_style = if is_set {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(MUTED)
                };
                kv_row(
                    bullet(focused, i == field_idx),
                    field.label,
                    Span::styled(value_str, val_style),
                )
            }
        };
        items.push(item);
    }

    // ── read-only Env section: configured in config.toml, not editable here ──
    if let Some(env) = profile.env.as_ref().filter(|e| !e.is_empty()) {
        items.push(section_header("Env — edit in ~/.config/ccs/config.toml"));
        for (key, value) in env {
            items.push(kv_row(
                bullet(false, false),
                key,
                Span::styled(value.as_str(), Style::default().fg(Color::Green)),
            ));
        }
    }

    items
}

pub fn run(mut app: App) -> (App, Option<String>) {
    let mut terminal = ratatui::init();

    loop {
        if let Err(e) = terminal.draw(|frame| {
            let area = frame.area();

            let v = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(area);

            // ── header ──
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    "Claude Code Switcher",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  profile manager for Claude Code",
                    Style::default().fg(MUTED),
                ),
            ]));
            frame.render_widget(
                header.block(
                    Block::default()
                        .borders(Borders::BOTTOM)
                        .border_style(Style::default().fg(MUTED)),
                ),
                v[0],
            );

            // ── body ──
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(v[1]);

            // ── left: profile list ──
            let left_focused = app.focus == Focus::Left;
            let selected_idx = app.selected_index();
            let profile_items: Vec<ListItem> = app
                .names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let bullet = bullet(left_focused, i == selected_idx);
                    let desc = app.config.profiles[name]
                        .description
                        .as_deref()
                        .unwrap_or("");
                    if desc.is_empty() {
                        ListItem::new(Line::from(vec![
                            Span::raw(bullet),
                            Span::raw(name.as_str()),
                        ]))
                    } else {
                        ListItem::new(vec![
                            Line::from(vec![Span::raw(bullet), Span::raw(name.as_str())]),
                            Line::from(vec![
                                Span::raw("  "),
                                Span::styled(desc, Style::default().fg(MUTED)),
                            ]),
                        ])
                    }
                })
                .collect();

            let count = app.names.len();
            let border_style = if app.focus == Focus::Left {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(MUTED)
            };
            let list = List::new(profile_items)
                .block(
                    Block::bordered()
                        .border_style(border_style)
                        .title(Span::styled(
                            " Profiles ",
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        ))
                        .title_bottom(Span::styled(
                            format!(" {count} profiles "),
                            Style::default().fg(MUTED),
                        )),
                )
                .highlight_style(Style::default().fg(Color::Black).bg(ACCENT));

            frame.render_stateful_widget(list, body[0], &mut app.profile_list);

            // ── right: field editor or create form ──
            let right_border = if app.focus == Focus::Right {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(MUTED)
            };

            if let InputState::Creating { buffer, cursor } = &app.input {
                let byte_pos = char_to_byte(buffer, *cursor);
                let mut display = buffer.clone();
                display.insert(byte_pos, '█');
                let create_text = Paragraph::new(Line::from(vec![
                    Span::raw("\n"),
                    Span::raw("  "),
                    Span::styled("name: ", Style::default().fg(MUTED)),
                    Span::styled(display, Style::default().fg(Color::Yellow)),
                    Span::raw("\n\n  "),
                    Span::styled(
                        "Enter",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" to create, ", Style::default().fg(MUTED)),
                    Span::styled(
                        "Esc",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" to cancel", Style::default().fg(MUTED)),
                ]))
                .block(
                    Block::bordered()
                        .border_style(right_border)
                        .title(Span::styled(
                            " New Profile ",
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        )),
                );
                frame.render_widget(create_text, body[1]);
            } else {
                let selected = app.selected_name().unwrap_or("");
                let profile = app.config.profiles.get(selected);
                let right_focused = app.focus == Focus::Right;
                let field_items = if let Some(profile) = profile {
                    let items =
                        build_field_items(profile, app.field_idx, &app.input, right_focused);
                    // field_list_idx() and build_field_items() must keep counting rows alike
                    debug_assert!(app.field_list_idx() < items.len());
                    items
                } else {
                    vec![ListItem::new("")]
                };

                let field_list = List::new(field_items)
                    .block(
                        Block::bordered()
                            .border_style(right_border)
                            .title(Span::styled(
                                format!(" {} ", selected),
                                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                            )),
                    )
                    .highlight_style(Style::default().fg(Color::Black).bg(ACCENT));

                let mut field_state =
                    ListState::default().with_selected(Some(app.field_list_idx()));
                frame.render_stateful_widget(field_list, body[1], &mut field_state);
            }

            // ── footer ──
            let footer = match (&app.input, app.focus) {
                (InputState::Creating { .. }, _) => Line::from(vec![
                    footer_span("Enter"),
                    Span::raw(" "),
                    footer_span("Esc"),
                ]),
                (InputState::Editing { .. }, _) => Line::from(vec![
                    footer_span("Enter"),
                    Span::raw(" "),
                    footer_span("Esc"),
                ]),
                (InputState::None, Focus::Left) => Line::from(vec![
                    footer_span("↑↓/jk"),
                    Span::raw(" "),
                    footer_span("n"),
                    Span::raw(" "),
                    footer_span("d"),
                    Span::raw(" "),
                    footer_span("→/l"),
                    Span::raw(" "),
                    footer_span("Enter"),
                    Span::raw(" "),
                    footer_span("q/ESC"),
                ]),
                (InputState::None, Focus::Right) => Line::from(vec![
                    footer_span("↑↓/jk"),
                    Span::raw(" "),
                    footer_span("←/h"),
                    Span::raw(" "),
                    footer_span("Enter"),
                    Span::raw(" "),
                    footer_span("q/ESC"),
                ]),
            };
            frame.render_widget(Paragraph::new(footer).centered(), v[2]);
        }) {
            ratatui::restore();
            crate::fatal(&format!("terminal error: {e}"));
        }

        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match &mut app.input {
                InputState::Creating { buffer, cursor } => match key.code {
                    KeyCode::Enter => {
                        let name = buffer.trim().to_string();
                        if !name.is_empty() && !app.config.profiles.contains_key(&name) {
                            app.config
                                .profiles
                                .insert(name.clone(), config::Profile::default());
                            config::save_config(&app.config);
                            app.refresh_names();
                            if let Some(pos) = app.names.iter().position(|n| n == &name) {
                                app.profile_list.select(Some(pos));
                            }
                        }
                        app.input = InputState::None;
                    }
                    KeyCode::Esc => app.input = InputState::None,
                    _ => handle_edit_key(buffer, cursor, key.code),
                },
                InputState::Editing { buffer, cursor } => match key.code {
                    KeyCode::Enter => {
                        let saved = buffer.clone();
                        let name = app.selected_name().map(|s| s.to_string());
                        if let Some(ref name) = name {
                            if let Some(profile) = app.config.profiles.get_mut(name) {
                                let field = &PROFILE_FIELDS[app.field_idx];
                                (field.set)(profile, saved);
                                config::save_config(&app.config);
                            }
                        }
                        app.input = InputState::None;
                    }
                    KeyCode::Esc => {
                        app.input = InputState::None;
                    }
                    _ => handle_edit_key(buffer, cursor, key.code),
                },
                InputState::None => match app.focus {
                    Focus::Left => match key.code {
                        KeyCode::Enter => {
                            let name = app.selected_name().map(|s| s.to_string());
                            ratatui::restore();
                            return (app, name);
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            ratatui::restore();
                            return (app, None);
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            ratatui::restore();
                            return (app, None);
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.profile_next(),
                        KeyCode::Up | KeyCode::Char('k') => app.profile_prev(),
                        KeyCode::Right | KeyCode::Char('l') => app.focus = Focus::Right,
                        KeyCode::Char('n') => {
                            app.input = InputState::Creating {
                                buffer: String::new(),
                                cursor: 0,
                            };
                        }
                        KeyCode::Char('d') => {
                            if let Some(name) = app.selected_name().map(|s| s.to_string()) {
                                app.config.profiles.remove(&name);
                                config::save_config(&app.config);
                                app.refresh_names();
                                if app.names.is_empty() {
                                    app.profile_list.select(None);
                                } else {
                                    let idx = app.selected_index().min(app.names.len() - 1);
                                    app.profile_list.select(Some(idx));
                                }
                            }
                        }
                        _ => {}
                    },
                    Focus::Right => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            ratatui::restore();
                            return (app, None);
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.field_next(),
                        KeyCode::Up | KeyCode::Char('k') => app.field_prev(),
                        KeyCode::Left | KeyCode::Char('h') => app.focus = Focus::Left,
                        KeyCode::Enter => {
                            let name = app.selected_name().map(|s| s.to_string());
                            if let Some(ref name) = name {
                                if let Some(profile) = app.config.profiles.get(name) {
                                    let field = &PROFILE_FIELDS[app.field_idx];
                                    let value = (field.get)(profile).unwrap_or_default();
                                    let len = value.chars().count();
                                    app.input = InputState::Editing {
                                        buffer: value,
                                        cursor: len,
                                    };
                                }
                            }
                        }
                        _ => {}
                    },
                },
            },
            Err(_) => {
                ratatui::restore();
                crate::fatal("failed to read input");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Rows rendered for `profile` with nothing being edited.
    fn rows_for(profile: &config::Profile) -> Vec<ListItem<'_>> {
        build_field_items(profile, 0, &InputState::None, true)
    }

    /// Row count for a profile without any env entries.
    fn baseline_rows() -> usize {
        rows_for(&config::Profile::default()).len()
    }

    #[test]
    fn field_items_include_readonly_env_section() {
        let profile = config::Profile {
            env: Some(BTreeMap::from([(
                "CLAUDE_CODE_MAX_OUTPUT_TOKENS".to_string(),
                "16000".to_string(),
            )])),
            ..Default::default()
        };
        let items = rows_for(&profile);
        // the Env section contributes exactly its header plus one entry
        assert_eq!(items.len(), baseline_rows() + 2);

        // read-only rows render at the end of the list
        let texts = render_items(&items);
        let header = &texts[texts.len() - 2];
        let entry = &texts[texts.len() - 1];
        assert!(header.contains("Env") && header.contains("config.toml"));
        assert!(entry.contains("CLAUDE_CODE_MAX_OUTPUT_TOKENS"));
        assert!(entry.contains("16000"));
    }

    #[test]
    fn empty_env_table_renders_no_section() {
        let profile = config::Profile {
            env: Some(BTreeMap::new()),
            ..Default::default()
        };
        assert_eq!(rows_for(&profile).len(), baseline_rows());
    }

    /// Render a (block-less) list and return one trimmed string per row.
    fn render_items(items: &[ListItem]) -> Vec<String> {
        use ratatui::widgets::Widget;
        let width = 80u16;
        let height = items.len() as u16;
        let area = ratatui::layout::Rect::new(0, 0, width, height);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        List::new(items.to_vec()).render(area, &mut buf);
        buf.content()
            .chunks(width as usize)
            .map(|row| {
                row.iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect()
    }
}
