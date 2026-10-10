use crate::{Annotation, Document, Rect};

#[derive(Clone, PartialEq)]
struct Snapshot {
    annotations: Vec<Annotation>,
    crop: Option<Rect>,
}

impl Snapshot {
    fn capture(document: &Document) -> Self {
        Self {
            annotations: document.annotations.clone(),
            crop: document.crop,
        }
    }

    fn restore(&self, document: &mut Document) {
        document.annotations.clone_from(&self.annotations);
        document.crop = self.crop;
    }
}

/// Undo/redo snapshots of editable state only. Call `commit` after a complete
/// gesture or text session; screenshot pixels are never copied or restored.
pub struct History {
    steps: Vec<Snapshot>,
    current: usize,
}

impl History {
    pub fn new(document: &Document) -> Self {
        Self {
            steps: vec![Snapshot::capture(document)],
            current: 0,
        }
    }

    /// Records a changed state and discards redo. A no-op preserves redo.
    pub fn commit(&mut self, document: &Document) {
        let snapshot = Snapshot::capture(document);
        if snapshot == self.steps[self.current] {
            return;
        }
        self.steps.truncate(self.current + 1);
        self.steps.push(snapshot);
        self.current += 1;
    }

    pub fn undo(&mut self, document: &mut Document) -> bool {
        if self.current == 0 {
            return false;
        }
        self.current -= 1;
        self.steps[self.current].restore(document);
        true
    }

    pub fn redo(&mut self, document: &mut Document) -> bool {
        if self.current + 1 == self.steps.len() {
            return false;
        }
        self.current += 1;
        self.steps[self.current].restore(document);
        true
    }
}
