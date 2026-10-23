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

    let filter_mode = if state.show_all_files {
        " [Showing: ALL Files] "
    } else {
        " [Showing: Tabular Data (CSV, TSV, PSV, TXT, etc.)] "
    };

    let header = Paragraph::new(format!(" Path: {:?}{}", state.current_dir, filter_mode))
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
            Span::raw(" | ENTER: Open | BACKSPACE: Up | A: Toggle All Files | ESC: Dashboard "),
        ])
    } else {
        Line::from(" ENTER: Open | BACKSPACE: Up | A: Toggle All Files | ESC: Dashboard ")
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

    let current_col = app.ui.col_list_state.selected().unwrap_or(0);
    let selection_count = app
        .ui
        .multi_selected
        .iter()
        .filter(|(c, _)| *c == current_col)
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
            let filter = &app.ui.active_filters[0];
            let op_str = match filter.operator {
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
            format!(" [FILTER: {} {} '{}'] ", filter.col_name, op_str, filter.value)
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

    let format_info = format!(" [{} | {}] ", app.delimiter.name(), app.encoding_name);

    let footer = Line::from(vec![
        Span::styled(format!(" Status: {} ", app.status_msg), Style::default().bg(Color::Blue).fg(Color::White)),
        Span::styled(format_info, Style::default().bg(Color::DarkGray).fg(Color::White)),
        sel_tag,
        filter_tag,
        Span::raw(master_hint),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[1]);

    if app.ui.input_mode == InputMode::Searching {
        let area = centered_rect(60, 20, f.size());
        f.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Search Unique Values (Enter/Esc to confirm): ")
            .border_style(Style::default().fg(Color::Cyan));
        let p = Paragraph::new(format!("\n  / {}", app.ui.search_buffer)).block(block);
        f.render_widget(p, area);
        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_x = (area.x + 4 + app.ui.search_buffer.chars().count() as u16).min(max_cursor_x);
        let cursor_y = (area.y + 2).min(area.bottom().saturating_sub(1));
        f.set_cursor(cursor_x, cursor_y);
    }

    if app.ui.input_mode == InputMode::Editing || app.ui.input_mode == InputMode::Filtering {
        let area = centered_rect(60, 20, f.size());
        f.render_widget(Clear, area);
        let (title, style) = if app.ui.input_mode == InputMode::Editing {
            (" Map Selected Value(s) to (Enter: Apply, Esc: Cancel): ", Style::default().fg(Color::Green))
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
        let max_cursor_x = area.right().saturating_sub(2);
        let cursor_x = (area.x + 3 + app.ui.input_cursor as u16).min(max_cursor_x);
        let cursor_y = (area.y + 2).min(area.bottom().saturating_sub(1));
        f.set_cursor(cursor_x, cursor_y);
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

    let active_key_name = app
        .db
        .headers
        .get(app.ui.registry_key_col)
        .map(|s| s.as_str())
        .unwrap_or("(none)");

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
    let col_name = app.db.headers.get(col_idx).map(|s| s.as_str()).unwrap_or("");
    
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
        let relative_sel = selected.map(|s| s.saturating_sub(start));
        (start, end, relative_sel)
    };

    let mut val_items = Vec::new();
    let slice = if total_count > 0 && start < end && end <= total_count {
        &app.ui.filtered_indices[start..end]
    } else {
        &[]
    };

    for &orig_idx in slice {
        let val = match app.db.unique_values.get(col_idx).and_then(|v| v.get(orig_idx)) {
            Some((v, _)) => v.as_str(),
            None => continue,
        };

        // Sanitize newlines for single-line display in Ratatui list item
        let clean_display = if val.contains('\n') || val.contains('\r') {
            val.replace(['\n', '\r'], " ")
        } else {
            val.to_string()
        };
        let display_val = if clean_display.is_empty() { "(EMPTY)" } else { clean_display.as_str() };

        let mut spans = vec![];
        if app.ui.multi_selected.contains(&(col_idx, val.to_string())) {
            spans.push("[x] ".cyan().bold());
        } else {
            let is_mapped = app.pipeline_cache.contains_key(&(col_name.to_string(), val.to_string()));
            if is_mapped {
                spans.push("[✓] ".green());
            } else {
                spans.push("[ ] ".dark_gray());
            }
        }

        let last_norm = app.pipeline_cache.get(&(col_name.to_string(), val.to_string()));
        if let Some(step) = last_norm {
            let norm_val = match step {
                crate::db::TransformationStep::SimpleMap { normalized, .. } => normalized,
                crate::db::TransformationStep::ConditionalMap { normalized, .. } => normalized,
            };
            spans.push(Span::raw(format!("{} ➜ ", display_val)));
            spans.push(norm_val.clone().yellow().bold());
        } else {
            spans.push(Span::raw(display_val.to_string()));
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
    let val = match app.db.unique_values.get(col_idx).and_then(|v| v.get(orig_idx)) {
        Some((v, _)) => v.as_str(),
        None => return "-".to_string(),
    };
    if let Some(col_reg) = app.db.row_registry.get(col_idx) {
        if let Some(indices) = col_reg.get(val) {
            let mut keys = Vec::with_capacity(5);
            let mut seen = std::collections::HashSet::new();
            if let Some(key_col) = app.db.columns.get(app.ui.registry_key_col) {
                for &idx in indices {
                    if let Some(k) = key_col.values.get(idx) {
                        if seen.insert(k.as_ref()) {
                            keys.push(k.as_ref());
                            if keys.len() >= 5 {
                                break;
                            }
                        }
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
    } else {
        "-".to_string()
    }
}
