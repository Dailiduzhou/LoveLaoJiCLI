//! Bounded 80×24+ layout: opening candidate pool, current output, assembly, source.

use std::sync::OnceLock;

use super::{
    highlight::{self, Palette},
    playback::{Playback, PlaybackState},
    progress,
};
use crate::{
    corpus,
    material::{Assembly, Material},
    pipeline::Generation,
    source::Source,
    trace::{Stage, StageRole},
};
use cli_common::Language;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;
const MAX_TEXT_CHARS: usize =
    crate::material::MAX_MATERIAL_BYTES + crate::material::MAX_MATERIAL_PIECES;

pub fn draw(
    frame: &mut Frame,
    generation: &Generation,
    source: &Source,
    playback: &Playback,
    language: Language,
    colors: bool,
) {
    let area = frame.area();
    let palette = Palette::new(colors);
    if area.width < MIN_WIDTH
        || area.height < MIN_HEIGHT
        || matches!(
            playback.state(),
            PlaybackState::TooSmall | PlaybackState::Suspended | PlaybackState::Background
        )
    {
        draw_small(frame, playback, language);
        return;
    }

    let [title, data, preview, code, help] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length((area.height / 4).clamp(5, 12)),
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let stage = playback
        .stage_index()
        .and_then(|index| generation.trace.stages.get(index));
    let material = displayed_material(generation, playback, language);
    let clipped = draw_data(frame, data, material.text)
        | draw_preview(frame, preview, generation, stage, playback, language);
    let [heading, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(title);
    let [identity, meter] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(progress::WIDTH)])
            .areas(heading);
    draw_title(
        frame,
        [identity, status],
        stage,
        playback,
        language,
        material,
        clipped,
    );
    progress::draw(frame, meter, playback, colors);
    draw_code(frame, code, generation, source, playback, language, palette);
    draw_help(frame, help, playback, language);
}

#[derive(Clone, Copy)]
struct DisplayedMaterial<'a> {
    text: Option<&'a Material>,
    candidate_pool: bool,
}

fn displayed_material<'a>(
    generation: &'a Generation,
    playback: &Playback,
    language: Language,
) -> DisplayedMaterial<'a> {
    let index = playback.stage_index();
    let stage = index.and_then(|index| generation.trace.stages.get(index));
    // The pool is opening context, not a replacement for a later empty output.
    // Detect the leading non-text prefix without assuming operation IDs/counts.
    let opening = !playback.is_completed()
        && stage.is_some()
        && generation
            .trace
            .stages
            .iter()
            .take(index.unwrap_or(0) + 1)
            .all(|stage| stage.material.pieces().is_empty() && stage.material.omitted() == 0);
    DisplayedMaterial {
        text: if opening {
            Some(candidate_pool(language))
        } else {
            stage.map(|stage| &stage.material)
        },
        candidate_pool: opening,
    }
}

fn candidate_pool(language: Language) -> &'static Material {
    static ENGLISH: OnceLock<Material> = OnceLock::new();
    static CHINESE: OnceLock<Material> = OnceLock::new();
    let cache = match language {
        Language::English => &ENGLISH,
        Language::Chinese => &CHINESE,
    };
    cache.get_or_init(|| {
        // Read the actual locale corpus in declaration order, keeping duplicates.
        // No seed, chosen frame, later observation or final sentence is consulted.
        let mut material = Material::default();
        for frame in corpus::frames(language) {
            for slot in frame.slots {
                for atom in slot
                    .before
                    .iter()
                    .chain(slot.variants.iter().flat_map(|variant| variant.iter()))
                {
                    material.atom(*atom, language, Assembly::None);
                }
            }
            for atom in frame.suffix {
                material.atom(*atom, language, Assembly::None);
            }
        }
        material
    })
}

fn state_text(state: PlaybackState, language: Language) -> &'static str {
    match state {
        PlaybackState::Playing => language.text("Playing", "播放中"),
        PlaybackState::Paused => language.text("Paused", "已暂停"),
        PlaybackState::TooSmall => language.text("Paused: too small", "窗口过小，已暂停"),
        PlaybackState::Suspended => language.text("Suspended", "已挂起"),
        PlaybackState::Background => language.text("Background", "后台"),
        PlaybackState::Completed => language.text("Completed", "已完成"),
        PlaybackState::Cancelled => language.text("Cancelled", "已取消"),
        PlaybackState::Failed => language.text("Failed", "失败"),
    }
}

fn draw_title(
    frame: &mut Frame,
    [identity, status]: [Rect; 2],
    stage: Option<&Stage>,
    playback: &Playback,
    language: Language,
    material: DisplayedMaterial<'_>,
    clipped: bool,
) {
    let position = playback.stage_index().map_or_else(
        || "0/0".into(),
        |index| format!("{}/{}", index + 1, playback.stage_count()),
    );
    let operation = stage.map_or("", |stage| stage.operation.name());
    let transition = if stage.is_some_and(|stage| stage.role == StageRole::Transition) {
        language.text(" · Transition", " · 展示转换")
    } else {
        ""
    };
    let material_label = if material.candidate_pool {
        language.text("Candidate pool", "候选原料")
    } else if material.text.is_none_or(|text| text.pieces().is_empty()) {
        language.text("No text", "暂无文本")
    } else {
        language.text("Current output", "当前输出")
    };
    let notice = if clipped {
        language.text(" · Clipped", " · 已裁剪")
    } else {
        ""
    };
    frame.render_widget(
        Paragraph::new(format!("encourage · {position} {operation}"))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        identity,
    );
    let title = format!(
        "{material_label}{notice} · {:.1}/{}s · {}{transition}",
        playback.elapsed().as_secs_f64(),
        playback.total().as_secs(),
        state_text(playback.state(), language),
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        status,
    );
}

fn draw_data(frame: &mut Frame, area: Rect, material: Option<&Material>) -> bool {
    // No title, types, diagnostic wrappers, counters or omissions inside here.
    let block = Block::bordered();
    let inner = block.inner(area);
    let raw = material.map_or_else(String::new, Material::raw);
    let (lines, clipped) = wrap_text(&raw, inner.width as usize, inner.height as usize);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    clipped || material.is_some_and(|material| material.omitted() != 0)
}

fn draw_preview(
    frame: &mut Frame,
    area: Rect,
    generation: &Generation,
    stage: Option<&Stage>,
    playback: &Playback,
    language: Language,
) -> bool {
    let title = if playback.is_completed() {
        language.text("Final sentence", "最终句子")
    } else {
        language.text("Current assembly preview", "当前拼接预览")
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    let preview = if playback.is_completed() {
        generation.sentence.clone()
    } else {
        stage.map_or_else(String::new, |stage| stage.material.preview(language))
    };
    let (lines, clipped) = wrap_text(&preview, inner.width as usize, inner.height as usize);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    clipped
}

fn draw_code(
    frame: &mut Frame,
    area: Rect,
    generation: &Generation,
    source: &Source,
    playback: &Playback,
    language: Language,
    palette: Palette,
) {
    let excerpt = source.omitted_before() || source.omitted_after();
    let title = match (language, excerpt) {
        (Language::English, true) => "Rust iterator chain (excerpt)",
        (Language::English, false) => "Rust iterator chain",
        (Language::Chinese, true) => "Rust 迭代器链（节选）",
        (Language::Chinese, false) => "Rust 迭代器链",
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    let height = inner.height as usize;
    let range = playback.stage_index().and_then(|index| {
        let stage = generation.trace.stages.get(index)?;
        source
            .range(stage.id)
            .filter(|range| *range == stage.source)
    });
    let cursor = range.map_or(0, |range| range.start);
    let start = scroll_start(source.lines(), cursor, height);
    let mut lines = Vec::with_capacity(height.min(source.lines().len()));
    for source_line in source.lines().iter().skip(start).take(height) {
        let active = range.is_some_and(|range| range.contains(source_line.number));
        let marker = if active { ">" } else { " " };
        let number_style = if active {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let mut spans = vec![Span::styled(
            format!("{marker} {:>4}  ", source_line.number),
            number_style,
        )];
        spans.extend(highlight::line(&source_line.text, palette).spans);
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn scroll_start(lines: &[crate::source::SourceLine], cursor: usize, height: usize) -> usize {
    if height == 0 || lines.len() <= height {
        return 0;
    }
    let cursor_index = lines
        .iter()
        .position(|line| line.number >= cursor)
        .unwrap_or(lines.len() - 1);
    cursor_index
        .saturating_sub(height / 2)
        .min(lines.len() - height)
}

fn draw_help(frame: &mut Frame, area: Rect, playback: &Playback, language: Language) {
    let text = if playback.is_completed() {
        language.text(
            "Enter restore and print the sentence    q / Ctrl+C cancel",
            "Enter 恢复原屏幕并输出句子    q / Ctrl+C 取消",
        )
    } else {
        language.text(
            "Space pause    Enter jump to result    q quit",
            "Space 暂停    Enter 跳到结果    q 退出",
        )
    };
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
}

fn draw_small(frame: &mut Frame, playback: &Playback, language: Language) {
    let area = frame.area();
    let text = if playback.is_completed() {
        language.text(
            "Need 80×24 to show the result\nEnter: restore and print · q cancel",
            "显示结果需要 80×24\nEnter：恢复并输出 · q 取消",
        )
    } else {
        language.text(
            "Need 80×24; replay paused\nEnlarge to continue · q quit",
            "需要 80×24；回放已暂停\n放大后继续 · q 退出",
        )
    };
    // No border is required at tiny or zero-sized viewports.
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), area);
}

/// Escape controls and wrap by terminal cells, without inserting ellipses or
/// labels into the material. All clipping is reported separately in the title.
fn wrap_text(value: &str, width: usize, height: usize) -> (Vec<Line<'static>>, bool) {
    if width == 0 || height == 0 {
        return (Vec::new(), !value.is_empty());
    }
    let mut source = value.chars();
    let mut escaped = String::new();
    for character in source.by_ref().take(MAX_TEXT_CHARS) {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    let truncated = source.next().is_some();
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut cells = 0;
    for character in escaped.chars() {
        let next = Line::from(character.to_string()).width();
        if next > width {
            return (lines, true);
        }
        if cells + next > width {
            lines.push(Line::from(std::mem::take(&mut line)));
            if lines.len() == height {
                return (lines, true);
            }
            cells = 0;
        }
        line.push(character);
        cells += next;
    }
    if !line.is_empty() {
        lines.push(Line::from(line));
    }
    (lines, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        source::Source,
        trace::{OperationKind, SourceRange, Stage, Trace, ValuePreview},
    };
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use std::time::Duration;

    fn fixture(stage_count: usize, language: Language) -> (Generation, Source, Playback) {
        let mut code = String::from("// header\n// encourage:display:start\n");
        code.push_str("fn generate(seed: u64, language: Language) {\n");
        let mut stages = Vec::new();
        for index in 0..stage_count {
            code.push_str(&format!("    // encourage:stage {}:start\n", index + 1));
            code.push_str(&format!(
                "    .map(|value| value + {index}) // stage {}\n",
                index + 1
            ));
            code.push_str(&format!("    // encourage:stage {}:end\n", index + 1));
            let source_line = code.lines().count() - 1;
            let mut stage = Stage::new(
                (index + 1) as u16,
                OperationKind::Map,
                SourceRange::new(source_line, source_line),
                "u64",
            );
            stage.summary.observed_inputs = Some(20);
            stage.summary.observed_outputs = Some(18);
            stage.summary.rejected = Some(2);
            stage.summary.observed_events = 20;
            for item in 0..20 {
                stage.summary.push_preview(
                    ValuePreview::new(item, format!("Word(Atom({item}))"))
                        .with_parents([item.saturating_sub(1)]),
                );
                stage.material.push(
                    &format!("原料片段{item}{}", "内容".repeat(8)),
                    true,
                    Assembly::Words,
                );
            }
            stages.push(stage);
        }
        code.push_str("}\n// encourage:display:end\n// footer\n");
        let source = Source::parse(&code).unwrap();
        let generation = Generation::from_trace(
            0x2a,
            language.text("One small step is enough.", "先迈出一小步。"),
            Trace { stages },
        );
        source.validate_trace(&generation.trace).unwrap();
        let playback = Playback::new(stage_count);
        (generation, source, playback)
    }

    fn render(
        width: u16,
        height: u16,
        generation: &Generation,
        source: &Source,
        playback: &Playback,
        language: Language,
        colors: bool,
    ) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, generation, source, playback, language, colors))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn text(buffer: &Buffer) -> String {
        buffer.content.iter().map(|cell| cell.symbol()).collect()
    }

    fn compact(buffer: &Buffer) -> String {
        text(buffer)
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    fn above_code(buffer: &Buffer) -> String {
        let rows = 2 + (buffer.area.height / 4).clamp(5, 12) + 4;
        buffer
            .content
            .iter()
            .take(rows as usize * buffer.area.width as usize)
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn renders_only_raw_material_and_reports_clipping_outside_the_data() {
        for language in [Language::English, Language::Chinese] {
            let (generation, source, playback) = fixture(34, language);
            let buffer = render(80, 24, &generation, &source, &playback, language, true);
            let text = text(&buffer);
            assert!(text.contains("map"));
            assert!(text.contains("u64"), "real source remains visible");
            assert!(
                compact(&buffer).contains(language.text("Clipped", "已裁剪")),
                "{text}"
            );
            assert!(text.contains("1/34"));
            assert!(!text.contains("Word(Atom("));
            assert!(!text.contains("events") && !text.contains("⟦") && !text.contains("…"));
            assert!(!text.contains("Final sentence"));
            assert!(!text.contains(&generation.sentence), "no final-result leak");
        }
    }

    #[test]
    fn top_right_progress_and_three_hearts_do_not_hide_material_status() {
        for language in [Language::English, Language::Chinese] {
            let generation = crate::pipeline::generate(0, language).unwrap();
            let source = Source::embedded().unwrap();
            let playback = Playback::new(generation.trace.stages.len());
            for (width, height) in [(80, 24), (100, 30)] {
                let buffer = render(
                    width,
                    height,
                    &generation,
                    &source,
                    &playback,
                    language,
                    true,
                );
                let meter: String = (width - progress::WIDTH..width)
                    .map(|x| buffer[(x, 0)].symbol())
                    .collect();
                assert!(meter.contains("0%"));
                assert_eq!(
                    meter
                        .chars()
                        .filter(|c| matches!(c, '♥' | '❤' | '💗'))
                        .count(),
                    3
                );
                let status: String = (0..width).map(|x| buffer[(x, 1)].symbol()).collect();
                let dense: String = status.chars().filter(|c| !c.is_whitespace()).collect();
                assert!(dense.contains(language.text("Candidatepool", "候选原料")));
                assert!(dense.contains(language.text("Playing", "播放中")));
                assert!(dense.contains("0.0/15s"));
                assert_eq!(
                    buffer[(0, 2)].symbol(),
                    "┌",
                    "material stays below the header"
                );
            }
        }
    }

    #[test]
    fn current_source_line_scrolls_into_view_and_completion_waits() {
        let (generation, source, mut playback) = fixture(34, Language::English);
        playback.advance(Duration::from_millis(11_100));
        let buffer = render(
            100,
            30,
            &generation,
            &source,
            &playback,
            Language::English,
            true,
        );
        assert!(text(&buffer).contains("> "));
        assert!(text(&buffer).contains("stage 26"));

        playback.key(super::super::playback::Key::Enter);
        let completed = render(
            100,
            30,
            &generation,
            &source,
            &playback,
            Language::English,
            true,
        );
        let text = text(&completed);
        assert!(text.contains("Final sentence"));
        assert!(text.contains("One small step is enough."));
        assert!(text.contains("Enter restore and print"));
        assert_eq!(
            playback.key(super::super::playback::Key::Space),
            super::super::playback::Action::Continue
        );
        assert!(playback.is_completed());
    }

    #[test]
    fn real_34_stage_generation_renders_without_fixture_metadata() {
        for language in [Language::English, Language::Chinese] {
            let generation = crate::pipeline::generate(0, language).unwrap();
            let source = Source::embedded().unwrap();
            source.validate_trace(&generation.trace).unwrap();
            let mut playback = Playback::new(generation.trace.stages.len());
            let buffer = render(100, 30, &generation, &source, &playback, language, true);
            let initial_text = text(&buffer);
            assert!(initial_text.contains("1/34"), "{initial_text}");
            let upper = above_code(&buffer);
            let dense_upper: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
            assert!(!upper.contains("01.theme"));
            assert!(dense_upper.contains(language.text("Candidatepool", "候选原料")));
            assert!(dense_upper.contains(language.text("enjoyawarmsimplehearty", "好好记得安心")));
            assert!(generation.trace.stages[0].material.pieces().is_empty());
            assert!(dense_upper.contains(language.text("Currentassemblypreview", "当前拼接预览")));
            assert!(!compact(&buffer).contains(&generation.sentence.replace(' ', "")));
            assert!(initial_text.contains(".map(|first|"));

            playback.advance(Duration::from_secs(8));
            let transition = render(100, 30, &generation, &source, &playback, language, true);
            let transition_text = compact(&transition);
            assert!(transition_text.contains("19/34"));
            assert!(
                transition_text.contains(language.text("Transition", "展示转换")),
                "{transition_text}"
            );

            playback.key(super::super::playback::Key::Enter);
            let completed = render(100, 30, &generation, &source, &playback, language, true);
            let expected = generation
                .sentence
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>();
            assert!(compact(&completed).contains(&expected));
        }
    }

    #[test]
    fn opening_pool_uses_real_locale_corpus_order_duplicates_and_bounds() {
        for language in [Language::English, Language::Chinese] {
            let material = candidate_pool(language);
            let expected_prefix = language.text(
                "enjoy a warm simple hearty meal and take a quiet short break . have some",
                "好好 记得 安心 吃 饭 ， 慢慢 用心 照顾 自己 。 先 记得",
            );
            assert!(material.raw().starts_with(expected_prefix));
            assert!(
                material
                    .pieces()
                    .iter()
                    .filter(|piece| piece.text == language.text("a", "记得"))
                    .count()
                    > 1
            );
            assert!(material.pieces().len() <= crate::material::MAX_MATERIAL_PIECES);
            assert!(
                material
                    .pieces()
                    .iter()
                    .map(|piece| piece.text.len())
                    .sum::<usize>()
                    <= crate::material::MAX_MATERIAL_BYTES
            );
            assert_eq!(
                material.omitted(),
                0,
                "current corpus fits the snapshot budget"
            );
            assert_eq!(
                material.preview(language),
                "",
                "alternatives are not an assembly"
            );
            assert!(
                std::ptr::eq(material, candidate_pool(language)),
                "reuse the bounded cache"
            );
            if language == Language::English {
                assert!(!material.raw().contains('好'));
            } else {
                assert!(!material.raw().contains("enjoy"));
            }
        }
    }

    #[test]
    fn opening_pool_is_seed_independent_and_switches_to_exact_current_output() {
        for language in [Language::English, Language::Chinese] {
            for seed in [0, 42, u64::MAX] {
                let generation = crate::pipeline::generate(seed, language).unwrap();
                let before = generation.clone();
                let source = Source::embedded().unwrap();
                let first_text = generation
                    .trace
                    .stages
                    .iter()
                    .position(|stage| {
                        !stage.material.pieces().is_empty() || stage.material.omitted() != 0
                    })
                    .unwrap();
                let mut playback = Playback::new(generation.trace.stages.len());
                for index in 0..generation.trace.stages.len() {
                    if index != 0 {
                        playback.advance(Duration::from_nanos(
                            15_000_000_000 / generation.trace.stages.len() as u64 + 1,
                        ));
                    }
                    assert_eq!(playback.stage_index(), Some(index));
                    let view = displayed_material(&generation, &playback, language);
                    assert_eq!(view.candidate_pool, index < first_text);
                    if index < first_text {
                        assert!(std::ptr::eq(view.text.unwrap(), candidate_pool(language)));
                        let buffer =
                            render(80, 24, &generation, &source, &playback, language, false);
                        let upper = above_code(&buffer);
                        assert!(
                            compact(&buffer).contains(language.text("Candidatepool", "候选原料"))
                        );
                        assert!(compact(&buffer)
                            .contains(language.text("enjoyawarmsimplehearty", "好好记得安心")));
                        assert!(
                            !upper.contains("Word(")
                                && !upper.contains("Atom")
                                && !upper.contains("Ok(")
                        );
                        // The pool never fills the real assembly preview.
                        let preview_y = 2 + (buffer.area.height / 4).clamp(5, 12);
                        for y in preview_y + 1..preview_y + 3 {
                            for x in 1..buffer.area.width - 1 {
                                assert_eq!(buffer[(x, y)].symbol(), " ");
                            }
                        }
                    } else {
                        assert!(std::ptr::eq(
                            view.text.unwrap(),
                            &generation.trace.stages[index].material
                        ));
                        if index == first_text {
                            let buffer =
                                render(80, 24, &generation, &source, &playback, language, false);
                            assert!(compact(&buffer)
                                .contains(language.text("Currentoutput", "当前输出")));
                            assert!(!compact(&buffer)
                                .contains(language.text("Candidatepool", "候选原料")));
                        }
                    }
                }
                assert_eq!(
                    generation, before,
                    "drawing must not change the v2 observations"
                );
            }
        }
    }

    #[test]
    fn later_empty_outputs_and_empty_traces_do_not_reuse_the_opening_pool() {
        for language in [Language::English, Language::Chinese] {
            let (mut generation, source, mut playback) = fixture(3, language);
            generation.trace.stages[0].material = Material::default();
            generation.trace.stages[2].material = Material::default();
            assert!(displayed_material(&generation, &playback, language).candidate_pool);
            playback.advance(Duration::from_secs(5));
            assert!(!displayed_material(&generation, &playback, language).candidate_pool);
            playback.advance(Duration::from_secs(5));
            let view = displayed_material(&generation, &playback, language);
            assert!(!view.candidate_pool);
            assert!(view.text.unwrap().pieces().is_empty());
            let buffer = render(80, 24, &generation, &source, &playback, language, false);
            assert!(compact(&buffer).contains(language.text("Notext", "暂无文本")));
            assert!(!text(&buffer).contains(language.text("enjoy a warm", "好好 记得")));

            let empty = Generation::from_trace(0, "", Trace::default());
            let empty_view = displayed_material(&empty, &Playback::new(0), language);
            assert!(!empty_view.candidate_pool);
            assert!(empty_view.text.is_none());
        }
    }

    #[test]
    fn assembly_preview_tracks_real_partial_and_edited_material_without_early_reveal() {
        let language = Language::English;
        let seed = crate::rng::seed_from_argv(&["42"]).unwrap();
        let generation = crate::pipeline::generate(seed, language).unwrap();
        let source = Source::embedded().unwrap();
        for index in [0, 15, 16, 17, 22, 23, 25, 26, 30, 32] {
            let mut playback = Playback::new(34);
            playback.advance(Duration::from_millis(index * 15_000 / 34 + 10));
            assert_eq!(playback.stage_index(), Some(index as usize));
            let buffer = render(120, 30, &generation, &source, &playback, language, false);
            let upper = above_code(&buffer);
            let dense: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
            let expected = generation.trace.stages[index as usize]
                .material
                .preview(language)
                .replace(' ', "");
            assert!(dense.contains(&expected), "stage {}: {upper}", index + 1);
            assert!(!upper.contains("Word(") && !upper.contains("Atom") && !upper.contains("Ok("));
            assert!(!upper.contains("Final sentence"));
            if index < 32 {
                assert!(
                    !dense.contains(&generation.sentence.replace(' ', "")),
                    "stage {} revealed final output too early",
                    index + 1
                );
            }
        }
    }

    #[test]
    fn no_color_changes_only_styles_and_tiny_sizes_stay_bounded() {
        let (generation, source, playback) = fixture(2, Language::Chinese);
        let colored = render(
            80,
            24,
            &generation,
            &source,
            &playback,
            Language::Chinese,
            true,
        );
        let plain = render(
            80,
            24,
            &generation,
            &source,
            &playback,
            Language::Chinese,
            false,
        );
        assert_eq!(text(&colored), text(&plain));
        assert!(plain
            .content
            .iter()
            .all(|cell| cell.fg == ratatui::style::Color::Reset));

        for (width, height) in [(0, 0), (1, 1), (20, 3), (79, 24), (80, 23)] {
            let buffer = render(
                width,
                height,
                &generation,
                &source,
                &playback,
                Language::Chinese,
                false,
            );
            assert_eq!(buffer.area.width, width);
            assert_eq!(buffer.area.height, height);
        }
    }

    #[test]
    fn wrapping_escapes_controls_without_ellipses_or_split_utf8() {
        let show = |text, width, height| {
            let (lines, clipped) = wrap_text(text, width, height);
            (
                lines
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>(),
                clipped,
            )
        };
        assert_eq!(show("你好世界", 5, 1), (vec!["你好".to_owned()], true));
        assert_eq!(
            show("你好世界", 5, 2),
            (vec!["你好".to_owned(), "世界".to_owned()], false)
        );
        assert_eq!(show("exact", 5, 1), (vec!["exact".to_owned()], false));
        assert_eq!(show("beyond", 5, 1), (vec!["beyon".to_owned()], true));
        assert_eq!(
            show("a\n\u{1b}[31mb", 80, 1),
            (vec!["a\\n\\u{1b}[31mb".to_owned()], false)
        );
        assert!(show(&"界".repeat(1000), 12, 1).1);
    }
}
