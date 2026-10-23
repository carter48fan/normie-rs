use crate::config::Config;
use crate::db::{Database, NormalizationState};
use crate::utils::StatefulList;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    rc::Rc,
};

pub struct Interner {
    pool: HashMap<String, Rc<str>>,
}

impl Interner {
    pub fn new() -> Self {
        Self { pool: HashMap::new() }
    }
    
    pub fn intern(&mut self, s: &str) -> Rc<str> {
        if let Some(rc) = self.pool.get(s) {
            rc.clone()
        } else {
            let rc: Rc<str> = Rc::from(s);
            self.pool.insert(s.to_string(), rc.clone());
            rc
        }
    }
}

fn handle_text_key(buffer: &mut String, cursor: &mut usize, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('u') => {
                buffer.clear();
                *cursor = 0;
                return;
            }
            KeyCode::Char('w') => {
                if *cursor > 0 {
                    let chars: Vec<char> = buffer.chars().collect();
                    let mut idx = *cursor;
                    while idx > 0 && chars[idx - 1].is_whitespace() {
                        idx -= 1;
                    }
                    while idx > 0 && !chars[idx - 1].is_whitespace() {
                        idx -= 1;
                    }
                    let mut new_chars = Vec::new();
                    new_chars.extend_from_slice(&chars[..idx]);
                    new_chars.extend_from_slice(&chars[*cursor..]);
                    *buffer = new_chars.into_iter().collect();
                    *cursor = idx;
                }
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Left => {
            *cursor = cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            if *cursor < buffer.chars().count() {
                *cursor += 1;
            }
        }
        KeyCode::Home => {
            *cursor = 0;
        }
        KeyCode::End => {
            *cursor = buffer.chars().count();
        }
        KeyCode::Backspace => {
            if *cursor > 0 {
                let mut chars: Vec<char> = buffer.chars().collect();
                chars.remove(*cursor - 1);
                *buffer = chars.into_iter().collect();
                *cursor -= 1;
            }
        }
        KeyCode::Delete => {
            let mut chars: Vec<char> = buffer.chars().collect();
            if *cursor < chars.len() {
                chars.remove(*cursor);
                *buffer = chars.into_iter().collect();
            }
        }
        KeyCode::Char(c) => {
            let mut chars: Vec<char> = buffer.chars().collect();
            chars.insert(*cursor, c);
            *buffer = chars.into_iter().collect();
            *cursor += 1;
        }
        _ => {}
    }
}

pub enum Action {
    None,
    Quit,
    Navigate(AppState),
}

pub enum AppState {
    Dashboard(DashboardState),
    FileBrowser(FileBrowserState),
    Normalizer(Box<NormalizerApp>),
}

pub struct App {
    pub state: AppState,
    pub config: Config,
    pub should_quit: bool,
}

impl App {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            state: AppState::Dashboard(DashboardState::new()),
            config: Config::load(),
            should_quit: false,
        }
    }

    pub fn handle_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        let action = match &mut self.state {
            AppState::Dashboard(state) => state.handle_event(&mut self.config, key),
            AppState::FileBrowser(state) => state.handle_event(&mut self.config, key),
            AppState::Normalizer(state) => state.handle_event(&mut self.config, key),
        };

        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::Navigate(new_state) => self.state = new_state,
        }

        Ok(())
    }
}

pub struct DashboardState {
    pub menu: StatefulList<String>,
    pub error_msg: Option<String>,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            menu: StatefulList::with_items(vec!["📂 Open New Session".to_string()]),
            error_msg: None,
        }
    }

    pub fn update_menu(&mut self, config: &Config) {
        let mut items = vec!["📂 Open New Session".to_string()];
        for path in &config.recent_files {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("Unknown");
            items.push(format!("📄 Recent: {}", name));
        }
        items.push("🚪 Quit".to_string());

        let selected = self.menu.state.selected();
        self.menu.items = items;
        if let Some(i) = selected {
            if i < self.menu.items.len() {
                self.menu.state.select(Some(i));
            } else {
                self.menu.state.select(Some(0));
            }
        } else {
            self.menu.state.select(Some(0));
        }
    }

    pub fn handle_event(&mut self, config: &mut Config, key: KeyEvent) -> Action {
        self.error_msg = None;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Up => {
                self.menu.previous();
                Action::None
            }
            KeyCode::Down => {
                self.menu.next();
                Action::None
            }
            KeyCode::Enter => {
                let i = self.menu.state.selected().unwrap_or(0);
                let recent_len = config.recent_files.len();
                if i == 0 {
                    Action::Navigate(AppState::FileBrowser(FileBrowserState::new(
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                    )))
                } else if i <= recent_len {
                    let path = config.recent_files[i - 1].clone();
                    match load_normalizer(path.clone()) {
                        Ok(normalizer) => {
                            config.add_recent(path);
                            Action::Navigate(AppState::Normalizer(Box::new(normalizer)))
                        }
                        Err(e) => {
                            self.error_msg = Some(format!("Could not open file: {}", e));
                            Action::None
                        }
                    }
                } else {
                    Action::Quit
                }
            }
            _ => Action::None,
        }
    }
}

pub struct FileBrowserState {
    pub current_dir: PathBuf,
    pub entries: StatefulList<PathBuf>,
    pub error_msg: Option<String>,
    pub show_all_files: bool,
}

impl FileBrowserState {
    pub fn new(dir: PathBuf) -> Self {
        let mut s = Self {
            current_dir: dir,
            entries: StatefulList::new(),
            error_msg: None,
            show_all_files: false,
        };
        s.refresh();
        s
    }

    pub fn refresh(&mut self) {
        let show_all = self.show_all_files;
        let mut items: Vec<PathBuf> = fs::read_dir(&self.current_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        if p.is_dir() {
                            return true;
                        }
                        if show_all {
                            return true;
                        }
                        p.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| {
                                let lower = ext.to_lowercase();
                                matches!(
                                    lower.as_str(),
                                    "csv" | "tsv" | "tab" | "txt" | "psv" | "dsv" | "dat" | "json"
                                )
                            })
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir != b_is_dir {
                b_is_dir.cmp(&a_is_dir)
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });
        self.entries = StatefulList::with_items(items);
    }

    pub fn handle_event(&mut self, config: &mut Config, key: KeyEvent) -> Action {
        self.error_msg = None;
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Char('.') => {
                self.show_all_files = !self.show_all_files;
                self.refresh();
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('q') => Action::Navigate(AppState::Dashboard(DashboardState::new())),
            KeyCode::Up => {
                self.entries.previous();
                Action::None
            }
            KeyCode::Down => {
                self.entries.next();
                Action::None
            }
            KeyCode::Backspace => {
                if let Some(parent) = self.current_dir.parent() {
                    self.current_dir = parent.to_path_buf();
                    self.refresh();
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some(i) = self.entries.state.selected() {
                    let path = self.entries.items[i].clone();
                    if path.is_dir() {
                        self.current_dir = path;
                        self.refresh();
                        Action::None
                    } else {
                        match load_normalizer(path.clone()) {
                            Ok(normalizer) => {
                                config.add_recent(path);
                                Action::Navigate(AppState::Normalizer(Box::new(normalizer)))
                            }
                            Err(e) => {
                                self.error_msg = Some(format!("{}", e));
                                Action::None
                            }
                        }
                    }
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }
}

#[derive(PartialEq)]
pub enum ActivePane {
    Columns,
    Values,
    MasterList,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
    SelectingKey,
    Searching,
    Filtering,
}

pub struct UiState {
    pub active_pane: ActivePane,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub search_buffer: String,
    pub filtered_indices: Vec<usize>,
    pub col_list_state: ListState,
    pub val_list_state: ListState,
    pub key_list_state: ListState,
    pub master_list_state: ListState,
    pub multi_selected: HashSet<(usize, String)>,
    pub registry_key_col: usize,
    pub active_filters: Vec<crate::db::Condition>,
    pub current_filter_operator: crate::db::ConditionOperator,
}

pub struct NormalizerApp {
    pub path: PathBuf,
    pub db: Database,
    pub norm: NormalizationState,
    pub pipeline_cache: HashMap<(String, String), crate::db::TransformationStep>,
    pub ui: UiState,
    pub status_msg: String,
    pub delimiter: crate::table::Delimiter,
    pub encoding_name: &'static str,
}

impl NormalizerApp {
    pub fn rebuild_pipeline_cache(&mut self) {
        self.pipeline_cache.clear();
        for step in &self.norm.pipeline {
            match step {
                crate::db::TransformationStep::SimpleMap { column, original, .. } => {
                    self.pipeline_cache.insert((column.clone(), original.clone()), step.clone());
                }
                crate::db::TransformationStep::ConditionalMap { column, original, .. } => {
                    self.pipeline_cache.insert((column.clone(), original.clone()), step.clone());
                }
            }
        }
    }

    pub fn save_norm(&mut self) {
        if let Err(e) = self.norm.save(&self.path) {
            self.status_msg = format!("ERROR: Failed to save mappings: {}", e);
        }
    }

    pub fn new(
        path: PathBuf,
        headers: Vec<String>,
        raw_rows: Vec<Vec<String>>,
        delimiter: crate::table::Delimiter,
        encoding_name: &'static str,
    ) -> Self {
        let headers: Vec<String> = if headers.is_empty() {
            vec!["Column_1".to_string()]
        } else {
            headers
        };
        let num_rows = raw_rows.len();
        let num_cols = headers.len();

        let mut interner = Interner::new();
        let mut columns = Vec::with_capacity(num_cols);
        for _ in 0..num_cols {
            columns.push(crate::db::ColumnData {
                values: Vec::with_capacity(num_rows),
                numeric_values: Vec::with_capacity(num_rows),
            });
        }

        let mut row_registry: Vec<HashMap<Rc<str>, Vec<usize>>> = vec![HashMap::new(); num_cols];
        let mut unique_values_sets: Vec<HashSet<Rc<str>>> = vec![HashSet::new(); num_cols];

        for (r_idx, row) in raw_rows.into_iter().enumerate() {
            for c_idx in 0..num_cols {
                let val = if c_idx < row.len() {
                    row[c_idx].as_str()
                } else {
                    ""
                };
                let trimmed = val.trim();
                let interned = interner.intern(trimmed);
                let parsed_num = trimmed.parse::<f64>().ok();

                columns[c_idx].values.push(interned.clone());
                columns[c_idx].numeric_values.push(parsed_num);

                unique_values_sets[c_idx].insert(interned.clone());
                row_registry[c_idx].entry(interned).or_default().push(r_idx);
            }
        }

        let unique_values: Vec<Vec<(String, String)>> = unique_values_sets
            .into_iter()
            .map(|set| {
                let mut v: Vec<Rc<str>> = set.into_iter().collect();
                v.sort_by_cached_key(|a| a.to_lowercase());
                v.into_iter()
                    .map(|rc| {
                        let s = rc.to_string();
                        let lower = s.to_lowercase();
                        (s, lower)
                    })
                    .collect()
            })
            .collect();

        let mut col_state = ListState::default();
        if !headers.is_empty() {
            col_state.select(Some(0));
        }

        let initial_key_idx = headers
            .iter()
            .position(|h| h.to_lowercase() == "id")
            .unwrap_or(0);
        let safe_key_idx = if headers.is_empty() {
            0
        } else {
            initial_key_idx.min(headers.len() - 1)
        };

        let mut app = Self {
            norm: NormalizationState::load(&path),
            path,
            db: Database {
                headers,
                columns,
                unique_values,
                row_registry,
                num_rows,
            },
            pipeline_cache: HashMap::new(),
            ui: UiState {
                active_pane: ActivePane::Columns,
                input_mode: InputMode::Normal,
                input_buffer: String::new(),
                input_cursor: 0,
                search_buffer: String::new(),
                filtered_indices: Vec::new(),
                col_list_state: col_state,
                val_list_state: ListState::default(),
                key_list_state: ListState::default(),
                master_list_state: ListState::default(),
                multi_selected: HashSet::new(),
                registry_key_col: safe_key_idx,
                active_filters: Vec::new(),
                current_filter_operator: crate::db::ConditionOperator::Contains,
            },
            status_msg: "Ready (Mappings Loaded)".to_string(),
            delimiter,
            encoding_name,
        };
        app.rebuild_pipeline_cache();
        app.update_filtered_vals();
        app
    }

    pub fn handle_event(&mut self, _config: &mut Config, key: KeyEvent) -> Action {
        match self.ui.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    return Action::Navigate(AppState::Dashboard(DashboardState::new()));
                }
                KeyCode::Char('s') => {
                    if let Err(e) = self.save_csv() {
                        self.status_msg = format!("ERROR: Failed to save CSV: {}", e);
                    }
                }
                KeyCode::Char('m') => {
                    self.ui.active_pane = if self.ui.active_pane == ActivePane::MasterList {
                        ActivePane::Columns
                    } else {
                        ActivePane::MasterList
                    };
                }
                KeyCode::Tab => {
                    self.ui.active_pane = match self.ui.active_pane {
                        ActivePane::Columns => ActivePane::Values,
                        ActivePane::Values => ActivePane::Columns,
                        ActivePane::MasterList => ActivePane::Columns,
                    };
                }
                KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) && self.ui.active_pane == ActivePane::MasterList => {
                    self.reorder_master_step(-1);
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) && self.ui.active_pane == ActivePane::MasterList => {
                    self.reorder_master_step(1);
                }
                KeyCode::Char('K') | KeyCode::Char('-') if self.ui.active_pane == ActivePane::MasterList => {
                    self.reorder_master_step(-1);
                }
                KeyCode::Char('J') | KeyCode::Char('+') if self.ui.active_pane == ActivePane::MasterList => {
                    self.reorder_master_step(1);
                }
                KeyCode::Down => self.move_list(1),
                KeyCode::Up => self.move_list(-1),
                KeyCode::Char(' ') => self.toggle_select(),
                KeyCode::Char('k') => self.ui.input_mode = InputMode::SelectingKey,
                KeyCode::Char('f') => {
                    let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
                    if let Some(pos) = self.ui.active_filters.iter().position(|f| f.col_idx == col_idx) {
                        self.ui.active_filters.remove(pos);
                        self.status_msg = "Filter cleared".to_string();
                        self.update_filtered_vals();
                    } else if self.has_selections_in_current_col() {
                        self.apply_list_filter();
                    } else {
                        self.ui.input_mode = InputMode::Filtering;
                        self.ui.input_buffer.clear();
                        self.ui.input_cursor = 0;
                    }
                }
                KeyCode::Char('/') => {
                    self.ui.input_mode = InputMode::Searching;
                }
                KeyCode::Enter => {
                    if self.ui.active_pane == ActivePane::Values {
                        self.ui.input_mode = InputMode::Editing;
                        self.ui.input_buffer.clear();
                        self.ui.input_cursor = 0;
                    }
                }
                KeyCode::Delete | KeyCode::Backspace => self.delete_current_mapping(),
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter => {
                    self.apply_mapping();
                    self.ui.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => self.ui.input_mode = InputMode::Normal,
                _ => {
                    handle_text_key(&mut self.ui.input_buffer, &mut self.ui.input_cursor, key);
                }
            },
            InputMode::Searching => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.ui.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => {
                    self.ui.search_buffer.push(c);
                    self.update_filtered_vals();
                }
                KeyCode::Backspace => {
                    self.ui.search_buffer.pop();
                    self.update_filtered_vals();
                }
                _ => {}
            },
            InputMode::Filtering => match key.code {
                KeyCode::Tab => {
                    self.ui.current_filter_operator = match self.ui.current_filter_operator {
                        crate::db::ConditionOperator::Contains => crate::db::ConditionOperator::NotContains,
                        crate::db::ConditionOperator::NotContains => crate::db::ConditionOperator::Equals,
                        crate::db::ConditionOperator::Equals => crate::db::ConditionOperator::NotEquals,
                        crate::db::ConditionOperator::NotEquals => crate::db::ConditionOperator::GreaterThan,
                        crate::db::ConditionOperator::GreaterThan => crate::db::ConditionOperator::LessThan,
                        crate::db::ConditionOperator::LessThan => crate::db::ConditionOperator::GreaterOrEqual,
                        crate::db::ConditionOperator::GreaterOrEqual => crate::db::ConditionOperator::LessOrEqual,
                        _ => crate::db::ConditionOperator::Contains,
                    };
                }
                KeyCode::BackTab => {
                    self.ui.current_filter_operator = match self.ui.current_filter_operator {
                        crate::db::ConditionOperator::Contains => crate::db::ConditionOperator::LessOrEqual,
                        crate::db::ConditionOperator::LessOrEqual => crate::db::ConditionOperator::GreaterOrEqual,
                        crate::db::ConditionOperator::GreaterOrEqual => crate::db::ConditionOperator::LessThan,
                        crate::db::ConditionOperator::LessThan => crate::db::ConditionOperator::GreaterThan,
                        crate::db::ConditionOperator::GreaterThan => crate::db::ConditionOperator::NotEquals,
                        crate::db::ConditionOperator::NotEquals => crate::db::ConditionOperator::Equals,
                        crate::db::ConditionOperator::Equals => crate::db::ConditionOperator::NotContains,
                        _ => crate::db::ConditionOperator::Contains,
                    };
                }
                KeyCode::Enter => {
                    self.set_active_filter();
                    self.ui.input_mode = InputMode::Normal;
                    self.ui.input_buffer.clear();
                    self.ui.input_cursor = 0;
                }
                KeyCode::Esc => {
                    self.ui.input_mode = InputMode::Normal;
                    self.ui.input_buffer.clear();
                    self.ui.input_cursor = 0;
                }
                _ => {
                    handle_text_key(&mut self.ui.input_buffer, &mut self.ui.input_cursor, key);
                }
            },
            InputMode::SelectingKey => match key.code {
                KeyCode::Up => {
                    let i = self.ui.key_list_state.selected().unwrap_or(0);
                    let next = if i == 0 {
                        self.db.headers.len() - 1
                    } else {
                        i - 1
                    };
                    self.ui.key_list_state.select(Some(next));
                }
                KeyCode::Down => {
                    let i = self.ui.key_list_state.selected().unwrap_or(0);
                    let next = if i >= self.db.headers.len() - 1 {
                        0
                    } else {
                        i + 1
                    };
                    self.ui.key_list_state.select(Some(next));
                }
                KeyCode::Enter => {
                    if let Some(i) = self.ui.key_list_state.selected() {
                        self.ui.registry_key_col = i;
                        self.calculate_info();
                    }
                    self.ui.input_mode = InputMode::Normal;
                }
                KeyCode::Esc | KeyCode::Char('k') => self.ui.input_mode = InputMode::Normal,
                _ => {}
            },
        }
        Action::None
    }

    pub fn set_active_filter(&mut self) {
        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        let col_name = self.db.headers[col_idx].clone();
        let val = self.ui.input_buffer.trim().to_string();
        
        // Remove existing filter for this column
        self.ui.active_filters.retain(|f| f.col_idx != col_idx);

        if val.is_empty() {
            self.status_msg = format!("Filter cleared for {}", col_name);
        } else {
            let op = self.ui.current_filter_operator.clone();
            let op_str = match op {
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
            self.ui.active_filters.push(crate::db::Condition {
                col_idx,
                col_name: col_name.clone(),
                operator: op,
                value: val.clone(),
                values: None,
            });
            self.status_msg = format!("Filter set: {} {} '{}'", col_name, op_str, val);
        }
        self.update_filtered_vals();
    }

    pub fn apply_list_filter(&mut self) {
        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        let col_name = self.db.headers[col_idx].clone();
        
        let mut selected_vals: Vec<String> = self.ui.multi_selected
            .iter()
            .filter(|(c, _)| *c == col_idx)
            .map(|(_, v)| v.clone())
            .collect();

        if selected_vals.is_empty() {
            return;
        }

        selected_vals.sort_unstable();

        self.ui.active_filters.retain(|f| f.col_idx != col_idx);
        
        let count = selected_vals.len();
        self.ui.active_filters.push(crate::db::Condition {
            col_idx,
            col_name: col_name.clone(),
            operator: crate::db::ConditionOperator::InList,
            value: format!("<{} values>", count),
            values: Some(selected_vals),
        });

        self.ui.multi_selected.retain(|(c, _)| *c != col_idx);
        self.status_msg = format!("Filter set: {} IN list ({} items)", col_name, count);
        self.update_filtered_vals();
    }

    pub fn move_list(&mut self, delta: i32) {
        match self.ui.active_pane {
            ActivePane::Columns => {
                let len = self.db.headers.len();
                if len == 0 {
                    return;
                }
                let i = self.ui.col_list_state.selected().unwrap_or(0) as i32;
                let next = (i + delta).rem_euclid(len as i32) as usize;
                self.ui.col_list_state.select(Some(next));
                self.ui.search_buffer.clear();
                self.update_filtered_vals();
                self.ui.val_list_state.select(Some(0));
            }
            ActivePane::Values => {
                let len = self.ui.filtered_indices.len();
                if len == 0 {
                    return;
                }
                let i = self.ui.val_list_state.selected().unwrap_or(0) as i32;
                let next = (i + delta).rem_euclid(len as i32) as usize;
                self.ui.val_list_state.select(Some(next));
            }
            ActivePane::MasterList => {
                let len = self.norm.pipeline.len();
                if len == 0 {
                    return;
                }
                let i = self.ui.master_list_state.selected().unwrap_or(0) as i32;
                let next = (i + delta).rem_euclid(len as i32) as usize;
                self.ui.master_list_state.select(Some(next));
            }
        };
    }

    pub fn toggle_select(&mut self) {
        if self.ui.active_pane != ActivePane::Values {
            return;
        }
        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        if let Some(val_idx) = self.ui.val_list_state.selected() {
            if val_idx >= self.ui.filtered_indices.len() {
                return;
            }
            let orig_idx = self.ui.filtered_indices[val_idx];
            let val = self.db.unique_values[col_idx][orig_idx].0.clone();
            let key = (col_idx, val);
            if !self.ui.multi_selected.remove(&key) {
                self.ui.multi_selected.insert(key);
            }
            self.move_list(1);
        }
    }

    pub fn update_filtered_vals(&mut self) {
        if self.db.headers.is_empty() || self.db.num_rows == 0 {
            self.ui.filtered_indices.clear();
            return;
        }

        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        let search = self.ui.search_buffer.to_lowercase();
        
        let mut row_indices = vec![true; self.db.num_rows];
        
        for filter in &self.ui.active_filters {
            let actual_col_idx = filter.resolve_col_idx(&self.db.headers).unwrap_or(filter.col_idx);
            if actual_col_idx >= self.db.columns.len() {
                continue;
            }
            let filter_f = filter.value.trim().parse::<f64>().ok();

            for (r_idx, pass) in row_indices.iter_mut().enumerate() {
                if !*pass {
                    continue;
                }
                let val = &self.db.columns[actual_col_idx].values[r_idx];
                let num_val = self.db.columns[actual_col_idx].numeric_values[r_idx];
                if !filter.matches(val.as_ref(), num_val, filter_f) {
                    *pass = false;
                }
            }
        }

        let filtered_unique: Vec<usize> = self.db.unique_values[col_idx]
            .iter()
            .enumerate()
            .filter(|(_, (val, lower))| {
                // Must be in search
                if !lower.contains(&search) {
                    return false;
                }
                // AND must exist in the rows that pass the filter
                if let Some(registry_indices) = self.db.row_registry[col_idx].get(val.as_str()) {
                    registry_indices.iter().any(|&idx| idx < row_indices.len() && row_indices[idx])
                } else {
                    false
                }
            })
            .map(|(i, _)| i)
            .collect();

        self.ui.filtered_indices = filtered_unique;

        if let Some(selected) = self.ui.val_list_state.selected() {
            if selected >= self.ui.filtered_indices.len() {
                self.ui
                    .val_list_state
                    .select(if self.ui.filtered_indices.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
            }
        } else if !self.ui.filtered_indices.is_empty() {
            self.ui.val_list_state.select(Some(0));
        }

        self.calculate_info();
    }

    pub fn calculate_info(&mut self) {
        // Calculated on the fly in the UI rendering loop to avoid overhead
    }

    pub fn has_selections_in_current_col(&self) -> bool {
        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        self.ui.multi_selected.iter().any(|(c, _)| *c == col_idx)
    }

    pub fn reorder_master_step(&mut self, delta: i32) {
        if self.norm.pipeline.is_empty() {
            return;
        }
        let current = self.ui.master_list_state.selected().unwrap_or(0);
        let target = (current as i32 + delta).clamp(0, (self.norm.pipeline.len() - 1) as i32) as usize;
        if current != target {
            self.norm.pipeline.swap(current, target);
            self.ui.master_list_state.select(Some(target));
            self.rebuild_pipeline_cache();
            self.save_norm();
            self.status_msg = format!("Moved pipeline step to #{}.", target + 1);
        }
    }

    pub fn delete_current_mapping(&mut self) {
        if self.ui.active_pane == ActivePane::MasterList {
            if let Some(i) = self.ui.master_list_state.selected() {
                if i < self.norm.pipeline.len() {
                    self.norm.pipeline.remove(i);
                    self.rebuild_pipeline_cache();
                    self.status_msg = "Removed step from pipeline".to_string();
                    self.save_norm();
                    if !self.norm.pipeline.is_empty() {
                        let new_sel = i.min(self.norm.pipeline.len() - 1);
                        self.ui.master_list_state.select(Some(new_sel));
                    } else {
                        self.ui.master_list_state.select(None);
                    }
                }
            }
            return;
        }

        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        if col_idx >= self.db.headers.len() {
            return;
        }
        let col_name = self.db.headers[col_idx].clone();
        if let Some(val_idx) = self.ui.val_list_state.selected() {
            if val_idx >= self.ui.filtered_indices.len() {
                return;
            }
            let orig_idx = self.ui.filtered_indices[val_idx];
            let val = self.db.unique_values[col_idx][orig_idx].0.clone();
            
            // Remove steps that affect this specific column and value
            let old_len = self.norm.pipeline.len();
            self.norm.pipeline.retain(|step| {
                !(step.column() == col_name && step.original() == val)
            });
            
            let removed = old_len - self.norm.pipeline.len();
            if removed > 0 {
                self.rebuild_pipeline_cache();
                self.status_msg = format!("Removed {} mapping rule(s) for '{}'", removed, val);
                self.save_norm();
            } else {
                self.status_msg = format!("No mapping rules to remove for '{}'", val);
            }
        }
    }

    pub fn apply_mapping(&mut self) {
        let col_idx = self.ui.col_list_state.selected().unwrap_or(0);
        let col_name = self.db.headers[col_idx].clone();

        let targets: Vec<String> = if !self.has_selections_in_current_col() {
            self.ui
                .val_list_state
                .selected()
                .and_then(|i| {
                    self.ui
                        .filtered_indices
                        .get(i)
                        .map(|&orig_idx| vec![self.db.unique_values[col_idx][orig_idx].0.clone()])
                })
                .unwrap_or_default()
        } else {
            let (mine, others): (HashSet<_>, HashSet<_>) = self
                .ui
                .multi_selected
                .drain()
                .partition(|(c, _)| *c == col_idx);
            self.ui.multi_selected = others;
            mine.into_iter().map(|(_, v)| v).collect()
        };

        if targets.is_empty() {
            self.status_msg = "Nothing selected".to_string();
            return;
        }

        let batch_size = targets.len();
        for t in targets {
            let step = if !self.ui.active_filters.is_empty() {
                crate::db::TransformationStep::ConditionalMap {
                    column: col_name.clone(),
                    original: t.clone(),
                    normalized: self.ui.input_buffer.clone(),
                    conditions: self.ui.active_filters.clone(),
                }
            } else {
                crate::db::TransformationStep::SimpleMap {
                    column: col_name.clone(),
                    original: t.clone(),
                    normalized: self.ui.input_buffer.clone(),
                }
            };

            // Deduplicate: If replacing an existing SimpleMap (or identical ConditionalMap),
            // update in-place instead of creating conflicting/dead duplicate pipeline steps!
            let existing_idx = if self.ui.active_filters.is_empty() {
                self.norm.pipeline.iter().position(|s| match s {
                    crate::db::TransformationStep::SimpleMap { column, original, .. } => {
                        column == &col_name && original == &t
                    }
                    _ => false,
                })
            } else {
                self.norm.pipeline.iter().position(|s| match s {
                    crate::db::TransformationStep::ConditionalMap { column, original, conditions, .. } => {
                        column == &col_name && original == &t && conditions == &self.ui.active_filters
                    }
                    _ => false,
                })
            };

            if let Some(idx) = existing_idx {
                self.norm.pipeline[idx] = step;
            } else {
                self.norm.pipeline.push(step);
            }
        }

        self.rebuild_pipeline_cache();
        self.save_norm();

        self.status_msg = format!(
            "Applied mapping for {} value(s) in '{}'.",
            batch_size, col_name
        );
    }

    pub fn save_csv(&mut self) -> Result<()> {
        let stem = self.path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let orig_ext = self.path.extension().and_then(|s| s.to_str());
        let ext = self.delimiter.default_extension(orig_ext);
        let output_path = self.path.with_file_name(format!("{}_normalized.{}", stem, ext));
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(self.delimiter.as_byte())
            .from_path(&output_path)?;
        wtr.write_record(&self.db.headers)?;

        // Pre-resolve steps and conditions before the row loop to avoid linear searches and float parsing on every row
        struct ResolvedCondition {
            col_idx: usize,
            operator: crate::db::ConditionOperator,
            value: String,
            target_num: Option<f64>,
            values_set: Option<HashSet<String>>,
        }

        struct ResolvedStep {
            target_col_idx: usize,
            original: String,
            normalized: String,
            conditions: Vec<ResolvedCondition>,
        }

        let resolved_steps: Vec<ResolvedStep> = self.norm.pipeline.iter().filter_map(|step| {
            let col_name = step.column();
            let target_col_idx = self.db.headers.iter().position(|h| h == col_name)?;

            let conditions = match step {
                crate::db::TransformationStep::SimpleMap { .. } => Vec::new(),
                crate::db::TransformationStep::ConditionalMap { conditions, .. } => {
                    let mut res_conds = Vec::new();
                    for cond in conditions {
                        let cond_col_idx = cond.resolve_col_idx(&self.db.headers)?;
                        let target_num = cond.value.trim().parse::<f64>().ok();
                        let values_set = cond.values.as_ref().map(|vals| vals.iter().cloned().collect());
                        res_conds.push(ResolvedCondition {
                            col_idx: cond_col_idx,
                            operator: cond.operator.clone(),
                            value: cond.value.clone(),
                            target_num,
                            values_set,
                        });
                    }
                    res_conds
                }
            };

            Some(ResolvedStep {
                target_col_idx,
                original: step.original().to_string(),
                normalized: step.normalized().to_string(),
                conditions,
            })
        }).collect();

        for r_idx in 0..self.db.num_rows {
            let mut new_row: Vec<String> = self.db.columns.iter()
                .map(|col| col.values[r_idx].to_string())
                .collect();
            
            for step in &resolved_steps {
                if new_row[step.target_col_idx].trim() == step.original {
                    let matches = if step.conditions.is_empty() {
                        true
                    } else {
                        step.conditions.iter().all(|c| {
                            let cell_val = &new_row[c.col_idx];
                            let cell_num = cell_val.trim().parse::<f64>().ok();
                            match c.operator {
                                crate::db::ConditionOperator::Equals => cell_val == &c.value,
                                crate::db::ConditionOperator::NotEquals => cell_val != &c.value,
                                crate::db::ConditionOperator::Contains => cell_val.contains(&c.value),
                                crate::db::ConditionOperator::NotContains => !cell_val.contains(&c.value),
                                crate::db::ConditionOperator::GreaterThan => {
                                    if let (Some(cv), Some(tv)) = (cell_num, c.target_num) {
                                        cv > tv
                                    } else {
                                        cell_val > &c.value
                                    }
                                }
                                crate::db::ConditionOperator::LessThan => {
                                    if let (Some(cv), Some(tv)) = (cell_num, c.target_num) {
                                        cv < tv
                                    } else {
                                        cell_val < &c.value
                                    }
                                }
                                crate::db::ConditionOperator::GreaterOrEqual => {
                                    if let (Some(cv), Some(tv)) = (cell_num, c.target_num) {
                                        cv >= tv
                                    } else {
                                        cell_val >= &c.value
                                    }
                                }
                                crate::db::ConditionOperator::LessOrEqual => {
                                    if let (Some(cv), Some(tv)) = (cell_num, c.target_num) {
                                        cv <= tv
                                    } else {
                                        cell_val <= &c.value
                                    }
                                }
                                crate::db::ConditionOperator::InList => {
                                    if let Some(ref set) = c.values_set {
                                        set.contains(cell_val)
                                    } else {
                                        true
                                    }
                                }
                            }
                        })
                    };
                    if matches {
                        new_row[step.target_col_idx] = step.normalized.clone();
                    }
                }
            }
            wtr.write_record(&new_row)?;
        }
        wtr.flush()?;
        self.status_msg = format!("SUCCESS: Saved to {:?}", output_path);
        Ok(())
    }
}

pub fn load_normalizer(path: PathBuf) -> Result<NormalizerApp> {
    let bytes = fs::read(&path).context(format!("Could not read {:?}", path))?;
    let (decoded_text, encoding_name) = crate::table::decode_file_bytes(&bytes);
    let delimiter = crate::table::detect_delimiter(&decoded_text, Some(&path));
    let (headers, rows) = crate::table::parse_tabular(&decoded_text, delimiter)?;
    let mut app = NormalizerApp::new(path, headers, rows, delimiter, encoding_name);
    app.status_msg = format!("Loaded ({} | {})", delimiter.name(), encoding_name);
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Delimiter;

    #[test]
    fn test_mixed_encoding_csv_loads() {
        let temp_path = std::env::temp_dir().join("normie_test_cp1252.csv");
        // 0x97 = cp1252 em dash, 0xAE = cp1252 ®, E2 80 99 = valid UTF-8 ’
        let bytes = b"id,desc\n1,size fool you\x97this banjo\n2,Mastertone\xAE don\xE2\x80\x99t\n3,plain\n";
        fs::write(&temp_path, bytes).unwrap();

        let app = load_normalizer(temp_path.clone()).unwrap();
        assert_eq!(&*app.db.columns[1].values[0], "size fool you\u{2014}this banjo");
        assert_eq!(&*app.db.columns[1].values[1], "Mastertone\u{AE} don\u{2019}t");
        assert_eq!(&*app.db.columns[1].values[2], "plain");

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_valid_utf8_with_windows1252_characters() {
        let temp_path = std::env::temp_dir().join("normie_test_utf8_win1252.csv");
        // Valid UTF-8 containing C1 control points U+0096 (en-dash), U+0093/U+0094 (smart quotes), and UTF-8 curly apostrophe
        let content = "id,product_title,price\n1,Mastertone\u{0096}Special: \u{0093}Bluegrass\u{0094} 5-String,1299.00\n2,Banjolele Concert 8\u{201D},599.99\n";
        fs::write(&temp_path, content.as_bytes()).unwrap();

        let app = load_normalizer(temp_path.clone()).unwrap();
        assert_eq!(app.db.headers, vec!["id", "product_title", "price"]);
        assert_eq!(&*app.db.columns[1].values[0], "Mastertone–Special: “Bluegrass” 5-String");
        assert_eq!(&*app.db.columns[1].values[1], "Banjolele Concert 8”");

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_tsv_loading_and_saving() {
        let temp_path = std::env::temp_dir().join("normie_test_data.tsv");
        let norm_output = std::env::temp_dir().join("normie_test_data_normalized.tsv");
        let _ = fs::remove_file(&norm_output);
        let _ = fs::remove_file(temp_path.with_extension("normie.json"));

        let content = "id\tmodel\tcategory\n1001\tTwanger\tBanjo\n1002\tBanjolele\tUke\n";
        fs::write(&temp_path, content.as_bytes()).unwrap();

        let mut app = load_normalizer(temp_path.clone()).unwrap();
        assert_eq!(app.delimiter, Delimiter::Tab);
        assert_eq!(app.db.headers, vec!["id", "model", "category"]);
        assert_eq!(app.db.num_rows, 2);

        // Normalize Uke -> Ukulele
        app.norm.pipeline.push(crate::db::TransformationStep::SimpleMap {
            column: "category".to_string(),
            original: "Uke".to_string(),
            normalized: "Ukulele".to_string(),
        });

        app.save_csv().unwrap();
        assert!(norm_output.exists());

        let saved = fs::read_to_string(&norm_output).unwrap();
        assert!(saved.contains("1002\tBanjolele\tUkulele"));

        let _ = fs::remove_file(temp_path);
        let _ = fs::remove_file(norm_output);
    }

    #[test]
    fn test_semicolon_format_detection() {
        let temp_path = std::env::temp_dir().join("normie_test_euro.csv");
        let content = "id;title;price\n1;Banjo;499,00\n2;Mandolin;399,00\n";
        fs::write(&temp_path, content.as_bytes()).unwrap();

        let app = load_normalizer(temp_path.clone()).unwrap();
        assert_eq!(app.delimiter, Delimiter::Semicolon);
        assert_eq!(app.db.headers, vec!["id", "title", "price"]);
        assert_eq!(&*app.db.columns[1].values[0], "Banjo");

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_ragged_csv_padding() {
        let temp_path = std::env::temp_dir().join("normie_test_ragged.csv");
        let headers = vec!["id".to_string(), "name".to_string(), "category".to_string()];
        let rows = vec![
            vec!["1".to_string(), "Banjo".to_string()], // Missing column 3
            vec!["2".to_string(), "Guitar".to_string(), "Strings".to_string(), "Extra".to_string()], // Extra column
        ];
        let app = NormalizerApp::new(temp_path, headers, rows, Delimiter::Comma, "UTF-8");
        assert_eq!(app.db.num_rows, 2);
        assert_eq!(app.db.columns.len(), 3);
        assert_eq!(app.db.columns[0].values.len(), 2);
        assert_eq!(app.db.columns[1].values.len(), 2);
        assert_eq!(app.db.columns[2].values.len(), 2);
        assert_eq!(&*app.db.columns[2].values[0], ""); // Padded with empty string
        assert_eq!(&*app.db.columns[2].values[1], "Strings");
    }

    #[test]
    fn test_apply_mapping_deduplication() {
        let temp_path = std::env::temp_dir().join("normie_test_dedup.csv");
        let _ = fs::remove_file(temp_path.with_extension("normie.json"));

        let headers = vec!["id".to_string(), "category".to_string()];
        let rows = vec![
            vec!["1".to_string(), "Banjo".to_string()],
            vec!["2".to_string(), "Banjo".to_string()],
        ];
        let mut app = NormalizerApp::new(temp_path.clone(), headers, rows, Delimiter::Comma, "UTF-8");
        app.ui.col_list_state.select(Some(1));
        app.ui.val_list_state.select(Some(0));

        // Map Banjo -> Banjos
        app.ui.input_buffer = "Banjos".to_string();
        app.apply_mapping();
        assert_eq!(app.norm.pipeline.len(), 1);
        assert_eq!(app.norm.pipeline[0].normalized(), "Banjos");

        // Map Banjo -> 5-String Banjo (should replace in-place, NOT append a conflicting rule)
        app.ui.input_buffer = "5-String Banjo".to_string();
        app.apply_mapping();
        assert_eq!(app.norm.pipeline.len(), 1);
        assert_eq!(app.norm.pipeline[0].normalized(), "5-String Banjo");

        let _ = fs::remove_file(temp_path.with_extension("normie.json"));
    }

    #[test]
    fn test_reorder_master_step() {
        let temp_path = std::env::temp_dir().join("normie_test_reorder.csv");
        let _ = fs::remove_file(temp_path.with_extension("normie.json"));

        let headers = vec!["id".to_string(), "category".to_string()];
        let rows = vec![vec!["1".to_string(), "A".to_string()]];
        let mut app = NormalizerApp::new(temp_path.clone(), headers, rows, Delimiter::Comma, "UTF-8");
        app.norm.pipeline.clear();

        app.norm.pipeline.push(crate::db::TransformationStep::SimpleMap {
            column: "category".to_string(),
            original: "A".to_string(),
            normalized: "Step 1".to_string(),
        });
        app.norm.pipeline.push(crate::db::TransformationStep::SimpleMap {
            column: "category".to_string(),
            original: "B".to_string(),
            normalized: "Step 2".to_string(),
        });

        app.ui.master_list_state.select(Some(1));
        app.reorder_master_step(-1);
        assert_eq!(app.norm.pipeline[0].normalized(), "Step 2");
        assert_eq!(app.norm.pipeline[1].normalized(), "Step 1");
        assert_eq!(app.ui.master_list_state.selected(), Some(0));

        let _ = fs::remove_file(temp_path.with_extension("normie.json"));
    }

    #[test]
    fn test_save_csv_transformation() {
        let temp_path = std::env::temp_dir().join("normie_test_save.csv");
        let norm_output = std::env::temp_dir().join("normie_test_save_normalized.csv");
        let _ = fs::remove_file(&norm_output);
        let _ = fs::remove_file(temp_path.with_extension("normie.json"));

        let headers = vec!["id".to_string(), "category".to_string(), "price".to_string()];
        let rows = vec![
            vec!["1".to_string(), "Banjo".to_string(), "500".to_string()],
            vec!["2".to_string(), "Banjo".to_string(), "1500".to_string()],
        ];
        let mut app = NormalizerApp::new(temp_path.clone(), headers, rows, Delimiter::Comma, "UTF-8");

        // Add a conditional mapping: IF price > 1000 => "Pro Banjo"
        app.norm.pipeline.push(crate::db::TransformationStep::ConditionalMap {
            column: "category".to_string(),
            original: "Banjo".to_string(),
            normalized: "Pro Banjo".to_string(),
            conditions: vec![crate::db::Condition {
                col_idx: 99, // Stale index; should resolve by name!
                col_name: "price".to_string(),
                operator: crate::db::ConditionOperator::GreaterThan,
                value: "1000".to_string(),
                values: None,
            }],
        });

        // Add a simple mapping: Banjo => "Standard Banjo"
        app.norm.pipeline.push(crate::db::TransformationStep::SimpleMap {
            column: "category".to_string(),
            original: "Banjo".to_string(),
            normalized: "Standard Banjo".to_string(),
        });

        app.save_csv().unwrap();

        let mut rdr = csv::Reader::from_path(&norm_output).unwrap();
        let records: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 2);
        assert_eq!(&records[0][1], "Standard Banjo");
        assert_eq!(&records[1][1], "Pro Banjo");

        let _ = fs::remove_file(&norm_output);
        let _ = fs::remove_file(temp_path.with_extension("normie.json"));
    }

    #[test]
    fn test_goldtone_products_real_data() {
        let path = PathBuf::from("goldtone_products.csv");
        if path.exists() {
            let app = load_normalizer(path).unwrap();
            assert_eq!(app.delimiter, Delimiter::Comma);
            assert!(app.db.headers.len() > 10);
            assert!(app.db.num_rows > 100);
        }
    }
}

