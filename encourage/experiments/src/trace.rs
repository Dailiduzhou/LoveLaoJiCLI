use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct TraceConfig {
    pub enabled: bool,
    pub previews: usize,
    pub events: usize,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            previews: 16,
            events: 128,
        }
    }
}

impl TraceConfig {
    pub fn off() -> Self {
        Self {
            enabled: false,
            previews: 0,
            events: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub observed: usize,
    pub preview: Vec<String>,
}

impl Snapshot {
    pub fn omitted(&self) -> usize {
        self.observed - self.preview.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub stage: &'static str,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Trace {
    config: TraceConfig,
    pub stages: BTreeMap<&'static str, Snapshot>,
    pub events: Vec<Event>,
    pub observed_events: usize,
}

impl Trace {
    pub fn new(config: TraceConfig) -> Self {
        Self {
            config,
            stages: BTreeMap::new(),
            events: Vec::new(),
            observed_events: 0,
        }
    }

    pub fn record(&mut self, stage: &'static str, value: impl FnOnce() -> String) {
        if !self.config.enabled {
            return;
        }
        let snapshot = self.stages.entry(stage).or_default();
        snapshot.observed += 1;
        self.observed_events += 1;
        let want_preview = snapshot.preview.len() < self.config.previews;
        let want_event = self.events.len() < self.config.events;
        if want_preview || want_event {
            let value = value();
            if want_preview {
                snapshot.preview.push(value.clone());
            }
            if want_event {
                self.events.push(Event { stage, value });
            }
        }
    }
}
