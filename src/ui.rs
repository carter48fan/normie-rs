use crate::app::{ActivePane, App, AppState, DashboardState, FileBrowserState, InputMode, NormalizerApp};
use crate::config::Config;
use crate::utils::centered_rect;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    match &mut app.state {
        AppState::Dashboard(state) => {
            state.update_menu(&app.config);
            ui_dashboard(f, state, &app.config);
        }
        AppState::FileBrowser(state) => ui_file_browser(f, state),
        AppState::Normalizer(norm_app) => ui_normalizer(f, norm_app),
    }
}

fn ui_dashboard(f: &mut Frame, state: &mut DashboardState, _config: &Config) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0), Constraint::Length(2)].as_ref())
        .split(f.size());

    let title = Paragraph::new(vec![
        Line::from(""),
        Line::from(r#"  _   _    ____    ____    __  __   ___  _____     ____    ____  "#),
        Line::from(r#" | \ | |  / __ \  |  _ \  |  \/  | |_ _| | ____|   |  _ \  / ___| "#),
        Line::from(r#" |  \| | | |  | | | |_) | | |\/| |  | |  |  _|     | |_) | \___ \ "#),
        Line::from(r#" | |\  | | |__| | |  _ <  | |  | |  | |  | |___    |  _ <   ___) |"#),
        Line::from(r#" |_| \_|  \____/  |_| \_\ |_|  |_| |___| |_____|   |_| \_\ |____/ "#),
        Line::from(""),
    ])
    .style(Style::default().fg(Color::Cyan).bold())
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = state.menu.items.iter().map(|s| ListItem::new(format!(" {} ", s))).collect();

    let menu = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Main Menu "))
        .highlight_style(Style::default().bg(Color::Indexed(237)).fg(Color::Yellow).bold())
        .highlight_symbol(">> ");

    let area = centered_rect(50, 50, chunks[1]);
    f.render_stateful_widget(menu, area, &mut state.menu.state);

    if let Some(err) = &state.error_msg {
        let err_p = Paragraph::new(format!(" ⚠️  {} ", err))
            .style(Style::default().fg(Color::Red).bold())
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(err_p, chunks[2]);
    }
}

fn ui_file_browser(f: &mut Frame, state: &mut FileBrowserState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());

    let header = Paragraph::new(format!(" Path: {:?}", state.current_dir))
        .block(Block::default().borders(Borders::ALL).title(" File Browser "));
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = state.entries.items.iter().map(|p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("..");
        if p.is_dir() {
            ListItem::new(format!(" 📁 {}/", name)).style(Style::default().fg(Color::Blue))
        } else {
            ListItem::new(format!(" 📄 {}", name)).style(Style::default().fg(Color::Green))
        }
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Files "))
        .highlight_style(Style::default().bg(Color::Indexed(237)).fg(Color::Yellow).bold());
    f.render_stateful_widget(list, chunks[1], &mut state.entries.state);

    let footer_content = if let Some(err) = &state.error_msg {
        Line::from(vec![
            Span::styled(format!(" ERROR: {} ", err), Style::default().bg(Color::Red).fg(Color::White).bold()),
            Span::raw(" | ENTER: Open/Enter | BACKSPACE: Up | ESC: Dashboard "),
        ])
    } else {
        Line::from(" ENTER: Open/Enter | BACKSPACE: Up | ESC: Dashboard ")
    };
    let footer = Paragraph::new(footer_content);
    f.render_widget(footer, chunks[2]);
}

fn ui_normalizer(f: &mut Frame, app: &mut NormalizerApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)].as_ref())
        .split(f.size());

    if app.ui.active_pane == ActivePane::MasterList {
        ui_master_list(f, app, chunks[0]);
    } else {
        ui_normalization_panes(f, app, chunks[0]);
    }

    let selection_count = app
        .ui
        .multi_selected
        .iter()
        .filter(|(c, _)| *c == app.ui.col_list_state.selected().unwrap_or(0))
        .count();
    let sel_tag = if selection_count > 0 {
        format!(" [{} SELECTED] ", selection_count)
            .on_cyan()
            .black()
            .bold()
    } else {
        Span::raw("")
    };

    let filter_tag = if !app.ui.active_filters.is_empty() {
        let count = app.ui.active_filters.len();
        let label = if count == 1 {
            let f = &app.ui.active_filters[0];
            let op_str = match f.operator {
                crate::db::ConditionOperator::Equals => "=",
                crate::db::ConditionOperator::NotEquals => "!=",
                crate::db::ConditionOperator::Contains => "~",
                crate::db::ConditionOperator::NotContains => "!~",
                crate::db::ConditionOperator::GreaterThan => ">",
                crate::db::ConditionOperator::LessThan => "<",
                crate::db::ConditionOperator::GreaterOrEqual => ">=",
                crate::db::ConditionOperator::LessOrEqual => "<=",
                crate::db::ConditionOperator::InList => "IN",
            };
            format!(" [FILTER: {} {} '{}'] ", f.col_name, op_str, f.value)
        } else {
            format!(" [FILTERS ACTIVE: {}] ", count)
        };
        label.on_magenta().white().bold()
    } else {
        Span::raw("")
    };

    let master_hint = if app.ui.active_pane == ActivePane::MasterList {
        " | SHIFT+UP/DN or J/K: Reorder Step | DEL: Remove Step | TAB: Panes | ESC: Dashboard "
    } else {
        " | TAB: Switch | M: Master List | F: Filter | /: Search | SPACE: Select | ENTER: Map | DEL: Undo | S: Save | Q/ESC: Dashboard "
    };

    let footer = Line::from(vec![
        Span::styled(format!(" Status: {} ", app.status_msg), Style::default().bg(Color::Blue).fg(Color::White)),
        sel_tag,
        filter_tag,
        Span::raw(master_hint),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[1]);

    if app.ui.input_mode == InputMode::Searching || !app.ui.search_buffer.is_empty() {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25)].as_ref())
            .split(chunks[0]);
            
        let area = Rect::new(
            main_chunks[1].x + 2,
            main_chunks[1].y + 1,
            main_chunks[1].width - 4,
            3,
        );
        f.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Search Values ")
            .border_style(
                Style::default().fg(if app.ui.input_mode == InputMode::Searching {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            );
        let p = Paragraph::new(format!(" /{}", app.ui.search_buffer)).block(block);
        f.render_widget(p, area);
    }

    if app.ui.input_mode == InputMode::Editing || app.ui.input_mode == InputMode::Filtering {
        // Position at bottom instead of center
        let area = Rect::new(
            f.size().width / 4,
            f.size().height - 5,
            f.size().width / 2,
            4,
        );
        f.render_widget(Clear, area);
        let (title, style) = if app.ui.input_mode == InputMode::Editing {
            (" Normalize To: ", Style::default().fg(Color::Green))
        } else {
            let t = match app.ui.current_filter_operator {
                crate::db::ConditionOperator::Equals => " Filter Equals (=): ",
                crate::db::ConditionOperator::NotEquals => " Filter Not Equals (!=): ",
                crate::db::ConditionOperator::Contains => " Filter Like (~): ",
                crate::db::ConditionOperator::NotContains => " Filter Not Like (!~): ",
                crate::db::ConditionOperator::GreaterThan => " Filter Greater Than (>): ",
                crate::db::ConditionOperator::LessThan => " Filter Less Than (<): ",
                crate::db::ConditionOperator::GreaterOrEqual => " Filter Greater or Equal (>=): ",
                crate::db::ConditionOperator::LessOrEqual => " Filter Less or Equal (<=): ",
                _ => " Filter: ",
            };
            (t, Style::default().fg(Color::Magenta))
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(style);
        let p = Paragraph::new(format!("\n  {}", app.ui.input_buffer)).block(block);
        f.render_widget(p, area);
        f.set_cursor(
            area.x + 3 + app.ui.input_cursor as u16,
            area.y + 2,
        );
    }

    if app.ui.input_mode == InputMode::SelectingKey {
        let area = centered_rect(40, 60, f.size());
        f.render_widget(Clear, area);
        let items: Vec<ListItem> = app
            .db
            .headers
            .iter()
            .map(|h| ListItem::new(h.as_str()))
            .collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Registry Key Column ")
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .highlight_style(Style::default().bg(Color::Magenta).fg(Color::White));
        f.render_stateful_widget(list, area, &mut app.ui.key_list_state);
    }
}

fn ui_normalization_panes(f: &mut Frame, app: &mut NormalizerApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ]
            .as_ref(),
        )
        .split(area);

    let active_key_name = &app.db.headers[app.ui.registry_key_col];
    let col_items: Vec<ListItem> = app
        .db
        .headers
        .iter()
        .enumerate()
        .map(|(idx, h)| {
            let style = if idx == app.ui.registry_key_col {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(h, style)]))
        })
        .collect();

    let col_block = Block::default()
        .borders(Borders::ALL)
        .title(" 1. Columns ")
        .border_style(
            Style::default().fg(if app.ui.active_pane == ActivePane::Columns {
                Color::Yellow
            } else {
                Color::White
            }),
        );

    f.render_stateful_widget(
        List::new(col_items)
            .block(col_block)
            .highlight_style(Style::default().bg(Color::Indexed(237))),
        main_chunks[0],
        &mut app.ui.col_list_state,
    );

    let col_idx = app.ui.col_list_state.selected().unwrap_or(0);
    let col_name = &app.db.headers[col_idx];
    
    // Compute viewport window slicing for high performance
    let total_count = app.ui.filtered_indices.len();
    let height = main_chunks[1].height as usize;
    let selected = app.ui.val_list_state.selected();
    
    let visible_height = height.saturating_sub(2);
    let (start, end, relative_sel) = if total_count == 0 || visible_height == 0 {
        (0, 0, None)
    } else {
        let sel = selected.unwrap_or(0);
        let half_height = visible_height / 2;
        let start = sel.saturating_sub(half_height);
        let end = (start + visible_height).min(total_count);
        let start = end.saturating_sub(visible_height);
        let relative_sel = selected.map(|s| s - start);
        (start, end, relative_sel)
    };

    let mut val_items = Vec::new();
    let slice = if total_count > 0 { &app.ui.filtered_indices[start..end] } else { &[] };

    for &orig_idx in slice {
        let val = &app.db.unique_values[col_idx][orig_idx].0;
        let mut spans = vec![];
        let display_val = if val.is_empty() { "(EMPTY)" } else { val };

        if app.ui.multi_selected.contains(&(col_idx, val.clone())) {
            spans.push("[x] ".cyan().bold());
        } else {
            // Check pre-calculated mapping cache (O(1) lookup)
            let is_mapped = app.pipeline_cache.contains_key(&(col_name.clone(), val.clone()));
            if is_mapped {
                spans.push("[✓] ".green());
            } else {
                spans.push("[ ] ".dark_gray());
            }
        }

        // Find the LAST normalization applied to this value (O(1) lookup)
        let last_norm = app.pipeline_cache.get(&(col_name.clone(), val.clone()));

        if let Some(step) = last_norm {
            let norm_val = match step {
                crate::db::TransformationStep::SimpleMap { normalized, .. } => normalized,
                crate::db::TransformationStep::ConditionalMap { normalized, .. } => normalized,
            };
            spans.push(Span::raw(format!("{} ➜ ", display_val)));
            spans.push(norm_val.clone().yellow().bold());
        } else {
            spans.push(Span::raw(display_val));
        }
        val_items.push(ListItem::new(Line::from(spans)));
    }

    let val_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 2. Unique Values for '{}' ", col_name))
        .border_style(
            Style::default().fg(if app.ui.active_pane == ActivePane::Values {
                Color::Yellow
            } else {
                Color::White
            }),
        );

    let mut temp_val_state = ListState::default();
    temp_val_state.select(relative_sel);

    f.render_stateful_widget(
        List::new(val_items)
            .block(val_block)
            .highlight_style(Style::default().bg(Color::Indexed(237))),
        main_chunks[1],
        &mut temp_val_state,
    );

    // Render source key mappings on the fly for visible elements only
    let info_items: Vec<ListItem> = slice
        .iter()
        .map(|&orig_idx| {
            let key_val = calculate_single_info(app, col_idx, orig_idx);
            ListItem::new(key_val)
        })
        .collect();

    let key_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" 3. Source Key [{}] ", active_key_name))
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_stateful_widget(
        List::new(info_items)
            .block(key_block)
            .highlight_style(Style::default().bg(Color::Indexed(237))),
        main_chunks[2],
        &mut temp_val_state.clone(),
    );
}

fn ui_master_list(f: &mut Frame, app: &mut NormalizerApp, area: Rect) {
    let items: Vec<ListItem> = app.norm.pipeline.iter().enumerate().map(|(idx, step)| {
        let mut spans = vec![Span::raw(format!("{:02}. ", idx + 1))];
        match step {
            crate::db::TransformationStep::SimpleMap { column, original, normalized } => {
                spans.push(column.clone().magenta());
                spans.push(Span::raw(": "));
                spans.push(original.clone().white());
                spans.push(Span::raw(" ➜ "));
                spans.push(normalized.clone().yellow().bold());
            }
            crate::db::TransformationStep::ConditionalMap { column, original, normalized, conditions } => {
                spans.push(column.clone().magenta());
                spans.push(Span::raw(": "));
                spans.push(original.clone().white());
                spans.push(Span::raw(" ➜ "));
                spans.push(normalized.clone().yellow().bold());
                let cond_str = if conditions.len() == 1 {
                    let c = &conditions[0];
                    let op = match c.operator {
                        crate::db::ConditionOperator::Equals => "=",
                        crate::db::ConditionOperator::NotEquals => "!=",
                        crate::db::ConditionOperator::Contains => "~",
                        crate::db::ConditionOperator::NotContains => "!~",
                        crate::db::ConditionOperator::GreaterThan => ">",
                        crate::db::ConditionOperator::LessThan => "<",
                        crate::db::ConditionOperator::GreaterOrEqual => ">=",
                        crate::db::ConditionOperator::LessOrEqual => "<=",
                        crate::db::ConditionOperator::InList => "IN",
                    };
                    format!(" [IF {} {} '{}']", c.col_name, op, c.value)
                } else {
                    format!(" [{} FILTERS]", conditions.len())
                };
                spans.push(Span::raw(cond_str).cyan());
            }
        }
        ListItem::new(Line::from(spans))
    }).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Master Transformation Pipeline (Order Matters) ")
        .border_style(Style::default().fg(Color::Yellow));

    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Indexed(237)).fg(Color::Yellow).bold()),
        area,
        &mut app.ui.master_list_state,
    );
}

fn calculate_single_info(app: &NormalizerApp, col_idx: usize, orig_idx: usize) -> String {
    let val = &app.db.unique_values[col_idx][orig_idx].0;
    if let Some(indices) = app.db.row_registry[col_idx].get(val.as_str()) {
        let mut keys = Vec::with_capacity(5);
        let mut seen = std::collections::HashSet::new();
        for &idx in indices {
            let k = app.db.columns[app.ui.registry_key_col].values[idx].as_ref();
            if seen.insert(k) {
                keys.push(k);
                if keys.len() >= 5 {
                    break;
                }
            }
        }
        if keys.is_empty() {
            "-".to_string()
        } else {
            let mut joined = String::new();
            for (i, &k) in keys.iter().enumerate() {
                if i > 0 {
                    joined.push_str(", ");
                }
                if k.is_empty() {
                    joined.push_str("(EMPTY)");
                } else {
                    joined.push_str(k);
                }
            }
            if indices.len() > keys.len() {
                let diff = indices.len() - keys.len();
                joined.push_str(&format!(" (+{} rows)", diff));
            }
            joined
        }
    } else {
        "-".to_string()
    }
}
