//! Compile-time embedded pipeline source and stable stage-to-line mapping.

use crate::trace::{OperationKind, SourceRange, StageId, StageRole, Trace};
use std::{collections::BTreeMap, fmt};

const DISPLAY_START: &str = "// encourage:display:start";
const DISPLAY_END: &str = "// encourage:display:end";
const STAGE_PREFIX: &str = "// encourage:stage ";
const OPERATION_PREFIX: &str = "// op";

#[derive(Clone, Debug)]
pub struct SourceLine {
    pub number: usize,
    pub text: String,
}

#[derive(Clone, Copy, Debug)]
struct SourceStage {
    range: SourceRange,
    operation: Option<OperationKind>,
    role: Option<StageRole>,
}

#[derive(Clone, Debug)]
pub struct Source {
    lines: Vec<SourceLine>,
    stages: BTreeMap<StageId, SourceStage>,
    total_lines: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError(pub String);

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SourceError {}

impl Source {
    pub fn embedded() -> Result<Self, SourceError> {
        // Execution and display deliberately share this exact file.
        Self::parse(include_str!("pipeline.rs"))
    }

    /// Embed a selected real pipeline, including the current isolated P0
    /// prototype. This does not read source from the filesystem at runtime.
    pub fn from_embedded(text: &'static str) -> Result<Self, SourceError> {
        Self::parse(text)
    }

    pub fn lines(&self) -> &[SourceLine] {
        &self.lines
    }

    pub fn range(&self, id: StageId) -> Option<SourceRange> {
        self.stages.get(&id).map(|stage| stage.range)
    }

    pub fn operation(&self, id: StageId) -> Option<OperationKind> {
        self.stages.get(&id).and_then(|stage| stage.operation)
    }

    pub fn role(&self, id: StageId) -> Option<StageRole> {
        self.stages.get(&id).and_then(|stage| stage.role)
    }

    pub fn omitted_before(&self) -> bool {
        self.lines.first().is_some_and(|line| line.number > 1)
    }

    pub fn omitted_after(&self) -> bool {
        self.lines
            .last()
            .is_some_and(|line| line.number < self.total_lines)
    }

    pub fn validate_trace(&self, trace: &Trace) -> Result<(), SourceError> {
        if self.stages.len() != trace.stages.len() {
            return Err(SourceError(format!(
                "source has {} stages but trace has {}",
                self.stages.len(),
                trace.stages.len()
            )));
        }
        let mut previous = None;
        for stage in &trace.stages {
            if previous.is_some_and(|id| id >= stage.id) {
                return Err(SourceError(
                    "trace stage ids are not strictly increasing".into(),
                ));
            }
            previous = Some(stage.id);
            match self.stages.get(&stage.id) {
                Some(source)
                    if source.range == stage.source
                        && source.operation.is_none_or(|kind| kind == stage.operation)
                        && source.role.is_none_or(|role| role == stage.role) => {}
                _ => {
                    return Err(SourceError(format!(
                        "stage {} has no matching source metadata",
                        stage.id
                    )))
                }
            }
        }
        Ok(())
    }

    pub(crate) fn parse(text: &str) -> Result<Self, SourceError> {
        let all: Vec<_> = text.lines().collect();
        let start = unique_marker(&all, DISPLAY_START)?;
        let end = unique_marker(&all, DISPLAY_END)?;
        if end <= start {
            return Err(SourceError("display markers are out of order".into()));
        }

        let mut lines = Vec::new();
        let mut stages = BTreeMap::new();
        let mut open: Option<(StageId, usize)> = None;
        let mut pending_operation: Option<(StageId, OperationKind, StageRole)> = None;
        for (index, text) in all.iter().enumerate().take(end).skip(start + 1) {
            let text = *text;
            let trimmed = text.trim();
            if let Some(marker) = trimmed.strip_prefix(STAGE_PREFIX) {
                if pending_operation.is_some() {
                    return Err(SourceError("operation marker has no source line".into()));
                }
                let (id, edge) = parse_stage_marker(marker)?;
                match edge {
                    Edge::Start => {
                        ensure_next_stage(&stages, id)?;
                        if open.is_some() {
                            return Err(SourceError(format!("invalid stage {id} start")));
                        }
                        open = Some((id, index + 2));
                    }
                    Edge::End => {
                        let (opened, first) = open
                            .take()
                            .ok_or_else(|| SourceError(format!("stage {id} has no start")))?;
                        if opened != id || first > index {
                            return Err(SourceError(format!("invalid stage {id} range")));
                        }
                        stages.insert(
                            id,
                            SourceStage {
                                range: SourceRange::new(first, index),
                                operation: None,
                                role: None,
                            },
                        );
                    }
                }
            } else {
                if trimmed.starts_with("// encourage:") {
                    return Err(SourceError("unknown encourage source marker".into()));
                }
                if open.is_some() && trimmed.starts_with(OPERATION_PREFIX) {
                    return Err(SourceError("nested operation marker".into()));
                }
                if let Some(marker) = trimmed.strip_prefix(OPERATION_PREFIX) {
                    if open.is_some() || pending_operation.is_some() {
                        return Err(SourceError("nested operation marker".into()));
                    }
                    let (id, operation, role) = parse_operation_marker(marker)?;
                    ensure_next_stage(&stages, id)?;
                    pending_operation = Some((id, operation, role));
                } else if let Some((id, operation, role)) = pending_operation.take() {
                    if trimmed.is_empty() {
                        return Err(SourceError(format!("stage {id} has no adapter")));
                    }
                    stages.insert(
                        id,
                        SourceStage {
                            range: SourceRange::new(index + 1, index + 1),
                            operation: Some(operation),
                            role: Some(role),
                        },
                    );
                }
                lines.push(SourceLine {
                    number: index + 1,
                    text: text.to_owned(),
                });
            }
        }
        if open.is_some() || pending_operation.is_some() {
            return Err(SourceError("unterminated stage marker".into()));
        }
        Ok(Self {
            lines,
            stages,
            total_lines: all.len(),
        })
    }
}

fn ensure_next_stage(
    stages: &BTreeMap<StageId, SourceStage>,
    id: StageId,
) -> Result<(), SourceError> {
    if stages.contains_key(&id) || stages.keys().next_back().is_some_and(|last| *last >= id) {
        Err(SourceError(format!("invalid or out-of-order stage {id}")))
    } else {
        Ok(())
    }
}

fn unique_marker(lines: &[&str], marker: &str) -> Result<usize, SourceError> {
    let mut found = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == marker)
        .map(|(index, _)| index);
    let first = found
        .next()
        .ok_or_else(|| SourceError(format!("missing `{marker}`")))?;
    if found.next().is_some() {
        return Err(SourceError(format!("duplicate `{marker}`")));
    }
    Ok(first)
}

#[derive(Clone, Copy)]
enum Edge {
    Start,
    End,
}

fn parse_stage_marker(marker: &str) -> Result<(StageId, Edge), SourceError> {
    let (number, edge) = marker
        .split_once(':')
        .ok_or_else(|| SourceError("malformed stage marker".into()))?;
    let id = parse_stage_id(number)?;
    let edge = match edge {
        "start" => Edge::Start,
        "end" => Edge::End,
        _ => return Err(SourceError("invalid stage marker edge".into())),
    };
    Ok((id, edge))
}

fn parse_operation_marker(
    marker: &str,
) -> Result<(StageId, OperationKind, StageRole), SourceError> {
    let (number, operation) = marker
        .split_once(':')
        .ok_or_else(|| SourceError("malformed operation marker".into()))?;
    let id = parse_stage_id(number)?;
    let mut fields = operation.split_whitespace();
    let operation = fields
        .next()
        .and_then(OperationKind::from_name)
        .ok_or_else(|| SourceError("unknown operation kind".into()))?;
    let role = match fields.next() {
        None => StageRole::Content,
        Some("(TRANSITION)") => StageRole::Transition,
        _ => return Err(SourceError("unknown operation role".into())),
    };
    if fields.next().is_some() {
        return Err(SourceError("extra operation marker fields".into()));
    }
    Ok((id, operation, role))
}

fn parse_stage_id(number: &str) -> Result<StageId, SourceError> {
    number
        .parse::<u16>()
        .ok()
        .filter(|id| *id != 0)
        .map(StageId)
        .ok_or_else(|| SourceError("invalid stage id".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_source_is_the_current_real_chain() {
        let source = Source::embedded().unwrap();
        assert!(source.omitted_before());
        assert!(source.omitted_after());
        assert_eq!(source.stages.len(), crate::pipeline::OPERATIONS.len());
        assert_eq!(source.operation(StageId(14)), Some(OperationKind::Flatten));
        assert_eq!(source.role(StageId(19)), Some(StageRole::Transition));
        assert!(!source.lines.iter().any(|line| line.text.contains("todo!")));
    }

    #[test]
    fn explicit_markers_map_real_lines_and_reject_ambiguous_order() {
        let valid = "// encourage:display:start\nlet n = 1;\n// encourage:stage 1:start\nn.map(drop)\n// encourage:stage 1:end\n// encourage:display:end\n";
        let source = Source::parse(valid).unwrap();
        assert_eq!(source.range(StageId(1)), Some(SourceRange::new(4, 4)));
        assert_eq!(source.lines()[1].text, "n.map(drop)");

        for invalid in [
            "// encourage:display:start\n// encourage:stage 0:start\nx\n// encourage:stage 0:end\n// encourage:display:end",
            "// encourage:display:start\n// encourage:stage 2:start\nx\n// encourage:stage 2:end\n// encourage:stage 1:start\ny\n// encourage:stage 1:end\n// encourage:display:end",
            "// encourage:display:start\n// encourage:stage 1:start\n// encourage:stage 2:start\nx\n// encourage:stage 2:end\n// encourage:stage 1:end\n// encourage:display:end",
        ] {
            assert!(Source::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn current_operation_comments_supply_metadata_without_editing_the_chain() {
        let source = Source::parse(
            "// encourage:display:start\n// op01: map\n    .map(|n| n)\n// op02: flatten (TRANSITION)\n    .flatten()\n// encourage:display:end",
        )
        .unwrap();
        assert_eq!(source.operation(StageId(1)), Some(OperationKind::Map));
        assert_eq!(source.role(StageId(1)), Some(StageRole::Content));
        assert_eq!(source.range(StageId(1)), Some(SourceRange::new(3, 3)));
        assert_eq!(source.operation(StageId(2)), Some(OperationKind::Flatten));
        assert_eq!(source.role(StageId(2)), Some(StageRole::Transition));
        assert!(source.lines().iter().any(|line| line.text.contains("op01")));
    }
}
