//! Bounded, real-observation data consumed by the stage replay.

use std::fmt;

pub const MAX_PREVIEWS: usize = 16;
pub const MAX_DETAILS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(pub u16);

impl fmt::Display for StageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}", self.0)
    }
}

/// One-based, inclusive lines in `pipeline.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

impl SourceRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn contains(self, line: usize) -> bool {
        self.start <= line && line <= self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Enumerate,
    Zip,
    TakeWhile,
    Scan,
    Map,
    FlatMap,
    Flatten,
    Filter,
    FilterMap,
    Chain,
    Fold,
}

impl OperationKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Enumerate => "enumerate",
            Self::Zip => "zip",
            Self::TakeWhile => "take_while",
            Self::Scan => "scan",
            Self::Map => "map",
            Self::FlatMap => "flat_map",
            Self::Flatten => "flatten",
            Self::Filter => "filter",
            Self::FilterMap => "filter_map",
            Self::Chain => "chain",
            Self::Fold => "fold",
        }
    }
}

impl OperationKind {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "enumerate" => Self::Enumerate,
            "zip" => Self::Zip,
            "take_while" => Self::TakeWhile,
            "scan" => Self::Scan,
            "map" => Self::Map,
            "flat_map" => Self::FlatMap,
            "flatten" => Self::Flatten,
            "filter" => Self::Filter,
            "filter_map" => Self::FilterMap,
            "chain" => Self::Chain,
            "fold" => Self::Fold,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StageRole {
    #[default]
    Content,
    Transition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservedItemId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuePreview {
    pub id: Option<ObservedItemId>,
    pub parents: Vec<ObservedItemId>,
    pub value: String,
}

impl ValuePreview {
    pub fn new(id: u64, value: impl Into<String>) -> Self {
        Self {
            id: Some(ObservedItemId(id)),
            parents: Vec::new(),
            value: value.into(),
        }
    }

    pub fn unidentified(value: impl Into<String>) -> Self {
        Self {
            id: None,
            parents: Vec::new(),
            value: value.into(),
        }
    }

    pub fn with_parents(mut self, parents: impl IntoIterator<Item = u64>) -> Self {
        self.parents = parents.into_iter().map(ObservedItemId).collect();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageSummary {
    pub value_type: &'static str,
    pub observed_inputs: Option<usize>,
    pub observed_outputs: Option<usize>,
    pub rejected: Option<usize>,
    pub observed_events: usize,
    /// A real short-circuit/pairing note, supplied by the collector.
    pub termination: Option<String>,
    previews: Vec<ValuePreview>,
    observed_previews: usize,
    details: Vec<String>,
    observed_details: usize,
}

impl StageSummary {
    pub fn new(value_type: &'static str) -> Self {
        Self {
            value_type,
            observed_inputs: None,
            observed_outputs: None,
            rejected: None,
            observed_events: 0,
            termination: None,
            previews: Vec::new(),
            observed_previews: 0,
            details: Vec::new(),
            observed_details: 0,
        }
    }

    /// Record an observed value. Saturation affects storage, never the count.
    pub fn push_preview(&mut self, preview: ValuePreview) {
        self.observed_previews += 1;
        if self.previews.len() < MAX_PREVIEWS {
            self.previews.push(preview);
        }
    }

    /// Account for real observations omitted by an upstream preview cap.
    pub fn omit_previews(&mut self, count: usize) {
        self.observed_previews += count;
    }

    /// Record a real state/filter/expansion detail with the global detail cap.
    pub fn push_detail(&mut self, detail: impl Into<String>) {
        self.observed_details += 1;
        if self.details.len() < MAX_DETAILS {
            self.details.push(detail.into());
        }
    }

    pub fn previews(&self) -> &[ValuePreview] {
        &self.previews
    }

    pub fn omitted_previews(&self) -> usize {
        self.observed_previews - self.previews.len()
    }

    pub fn details(&self) -> &[String] {
        &self.details
    }

    pub fn omitted_details(&self) -> usize {
        self.observed_details - self.details.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
    pub id: StageId,
    pub operation: OperationKind,
    pub role: StageRole,
    pub source: SourceRange,
    pub summary: StageSummary,
    /// Real textual payloads of this stage's outputs, separate from diagnostics.
    pub material: crate::material::Material,
}

impl Stage {
    pub fn new(
        id: u16,
        operation: OperationKind,
        source: SourceRange,
        value_type: &'static str,
    ) -> Self {
        Self {
            id: StageId(id),
            operation,
            role: StageRole::Content,
            source,
            summary: StageSummary::new(value_type),
            material: crate::material::Material::default(),
        }
    }

    pub fn with_role(mut self, role: StageRole) -> Self {
        self.role = role;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    pub stages: Vec<Stage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_storage_is_bounded_without_losing_exact_counts() {
        let mut summary = StageSummary::new("u64");
        for index in 0..40 {
            summary.push_preview(ValuePreview::new(index, index.to_string()));
        }
        for index in 0..140 {
            summary.push_detail(format!("state {index}"));
        }
        assert_eq!(summary.previews().len(), MAX_PREVIEWS);
        assert_eq!(summary.omitted_previews(), 24);
        assert_eq!(summary.details().len(), MAX_DETAILS);
        assert_eq!(summary.omitted_details(), 12);
    }
}
