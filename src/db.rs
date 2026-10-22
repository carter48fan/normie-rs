use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    rc::Rc,
};

pub struct ColumnData {
    pub values: Vec<Rc<str>>,
    pub numeric_values: Vec<Option<f64>>,
}

pub struct Database {
    pub headers: Vec<String>,
    pub columns: Vec<ColumnData>,
    pub unique_values: Vec<Vec<(String, String)>>,
    pub row_registry: Vec<HashMap<Rc<str>, Vec<usize>>>,
    pub num_rows: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    InList,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Condition {
    pub col_idx: usize,
    pub col_name: String,
    pub operator: ConditionOperator,
    pub value: String,
    #[serde(default)]
    pub values: Option<Vec<String>>,
}

impl Condition {
    /// Dynamically resolve column index by header name to guard against reordered/stale indices.
    pub fn resolve_col_idx(&self, headers: &[String]) -> Option<usize> {
        headers.iter().position(|h| h == &self.col_name)
    }

    /// Evaluates if an actual cell value satisfies this condition.
    /// If actual_num / target_num are pre-computed, uses numeric comparison for inequality operators.
    pub fn matches(&self, actual: &str, actual_num: Option<f64>, target_num: Option<f64>) -> bool {
        match self.operator {
            ConditionOperator::Equals => actual == self.value,
            ConditionOperator::NotEquals => actual != self.value,
            ConditionOperator::Contains => actual.contains(&self.value),
            ConditionOperator::NotContains => !actual.contains(&self.value),
            ConditionOperator::GreaterThan => {
                if let (Some(a), Some(t)) = (actual_num, target_num) {
                    a > t
                } else {
                    actual > &self.value
                }
            }
            ConditionOperator::LessThan => {
                if let (Some(a), Some(t)) = (actual_num, target_num) {
                    a < t
                } else {
                    actual < &self.value
                }
            }
            ConditionOperator::GreaterOrEqual => {
                if let (Some(a), Some(t)) = (actual_num, target_num) {
                    a >= t
                } else {
                    actual >= &self.value
                }
            }
            ConditionOperator::LessOrEqual => {
                if let (Some(a), Some(t)) = (actual_num, target_num) {
                    a <= t
                } else {
                    actual <= &self.value
                }
            }
            ConditionOperator::InList => {
                if let Some(ref vals) = self.values {
                    vals.iter().any(|v| v == actual)
                } else {
                    true
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn matches_str(&self, actual: &str) -> bool {
        let actual_num = actual.trim().parse::<f64>().ok();
        let target_num = self.value.trim().parse::<f64>().ok();
        self.matches(actual, actual_num, target_num)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TransformationStep {
    SimpleMap {
        column: String,
        original: String,
        normalized: String,
    },
    ConditionalMap {
        column: String,
        original: String,
        normalized: String,
        conditions: Vec<Condition>,
    },
}

impl TransformationStep {
    pub fn column(&self) -> &str {
        match self {
            Self::SimpleMap { column, .. } => column,
            Self::ConditionalMap { column, .. } => column,
        }
    }

    pub fn original(&self) -> &str {
        match self {
            Self::SimpleMap { original, .. } => original,
            Self::ConditionalMap { original, .. } => original,
        }
    }

    pub fn normalized(&self) -> &str {
        match self {
            Self::SimpleMap { normalized, .. } => normalized,
            Self::ConditionalMap { normalized, .. } => normalized,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct NormalizationState {
    pub pipeline: Vec<TransformationStep>,
}

impl NormalizationState {
    pub fn load(csv_path: &Path) -> Self {
        let mapping_path = csv_path.with_extension("normie.json");
        let content = match fs::read_to_string(&mapping_path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        // Try parsing the new format first
        if let Ok(state) = serde_json::from_str::<NormalizationState>(&content) {
            return state;
        }

        // Fallback to legacy HashMap format: Column Name -> (Original Value -> Normalized Value)
        if let Ok(legacy) = serde_json::from_str::<HashMap<String, HashMap<String, String>>>(&content) {
            let mut pipeline = Vec::new();
            for (col_name, mappings) in legacy {
                for (original, normalized) in mappings {
                    pipeline.push(TransformationStep::SimpleMap {
                        column: col_name.clone(),
                        original,
                        normalized,
                    });
                }
            }
            return NormalizationState { pipeline };
        }

        Self::default()
    }

    pub fn save(&self, csv_path: &Path) -> Result<()> {
        let mapping_path = csv_path.with_extension("normie.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(mapping_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_condition_matching() {
        let cond_eq = Condition {
            col_idx: 0,
            col_name: "status".to_string(),
            operator: ConditionOperator::Equals,
            value: "active".to_string(),
            values: None,
        };
        assert!(cond_eq.matches_str("active"));
        assert!(!cond_eq.matches_str("inactive"));

        let cond_gt = Condition {
            col_idx: 1,
            col_name: "price".to_string(),
            operator: ConditionOperator::GreaterThan,
            value: "100.50".to_string(),
            values: None,
        };
        assert!(cond_gt.matches_str("150.00"));
        assert!(!cond_gt.matches_str("99.99"));
        assert!(!cond_gt.matches_str("100.50"));

        let cond_in = Condition {
            col_idx: 2,
            col_name: "color".to_string(),
            operator: ConditionOperator::InList,
            value: "<2 values>".to_string(),
            values: Some(vec!["red".to_string(), "blue".to_string()]),
        };
        assert!(cond_in.matches_str("red"));
        assert!(cond_in.matches_str("blue"));
        assert!(!cond_in.matches_str("green"));
    }

    #[test]
    fn test_condition_resolve_col_idx() {
        let cond = Condition {
            col_idx: 99, // Stale index
            col_name: "category".to_string(),
            operator: ConditionOperator::Equals,
            value: "banjo".to_string(),
            values: None,
        };
        let headers = vec!["id".to_string(), "title".to_string(), "category".to_string()];
        assert_eq!(cond.resolve_col_idx(&headers), Some(2));

        let headers_missing = vec!["id".to_string(), "title".to_string()];
        assert_eq!(cond.resolve_col_idx(&headers_missing), None);
    }

    #[test]
    fn test_legacy_format_loading() {
        let legacy_json = r#"{
            "flange": {
                "One Piece": "One-Piece Die Cast"
            }
        }"#;
        let legacy: HashMap<String, HashMap<String, String>> = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.get("flange").unwrap().get("One Piece").unwrap(), "One-Piece Die Cast");
    }
}

