//! The tasks a user means to finish in a session, and the rules for changing
//! them. Notes and drop reasons need a few real words (at least 2 different
//! ones from the ENABLE word list): friction, not proof.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_TASKS: usize = 20;
pub const MAX_TASK_LEN: usize = 120;
pub const MAX_NOTE_LEN: usize = 200;
pub const MIN_NOTE_WORDS: usize = 3;
pub const MIN_REAL_WORDS: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskState {
    Open,
    Done {
        note: String,
    },
    /// Deleted mid-session. Kept for the stats; can't come back.
    Dropped {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: u32,
    pub text: String,
    pub state: TaskState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TaskError {
    #[error("that task no longer exists")]
    NotFound,
    #[error("a session can have at most 20 tasks")]
    TooMany,
    #[error("a task needs 1 to 120 characters")]
    InvalidText,
    #[error("write at least 3 words (up to 200 characters)")]
    NoteTooShort,
    #[error("use at least 2 different real words")]
    NotRealWords,
    #[error("that task was dropped; add it again instead")]
    Dropped,
}

/// Words that contain a letter or digit, so "- . !" doesn't count.
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

fn clean_text(text: &str) -> Result<String, TaskError> {
    let t = text.trim();
    if t.is_empty() || t.chars().count() > MAX_TASK_LEN {
        return Err(TaskError::InvalidText);
    }
    Ok(t.to_string())
}

/// The ENABLE word list (public domain), one lowercase word per line.
const WORD_LIST: &str = include_str!("../data/enable1.txt");

fn dictionary() -> &'static HashSet<&'static str> {
    static DICT: OnceLock<HashSet<&'static str>> = OnceLock::new();
    DICT.get_or_init(|| WORD_LIST.lines().map(str::trim).filter(|w| !w.is_empty()).collect())
}

/// Lowercased, outer non-letters stripped, apostrophes removed: "Didn't," -> "didnt".
fn normalize(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphabetic())
        .chars()
        .filter(|&c| c != '\'' && c != '\u{2019}')
        .flat_map(char::to_lowercase)
        .collect()
}

/// How many different words in `s` are in the word list.
pub fn real_word_count(s: &str) -> usize {
    let dict = dictionary();
    s.split_whitespace().map(normalize).filter(|w| dict.contains(w.as_str())).collect::<HashSet<_>>().len()
}

/// Live feedback for the note dialog: the same rules `tick` and `drop_task` apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCheck {
    pub words: usize,
    pub real_words: usize,
    pub ok: bool,
}

pub fn check_note(note: &str) -> NoteCheck {
    let n = note.trim();
    NoteCheck { words: word_count(n), real_words: real_word_count(n), ok: clean_note(n).is_ok() }
}

fn clean_note(note: &str) -> Result<String, TaskError> {
    let n = note.trim();
    if word_count(n) < MIN_NOTE_WORDS || n.chars().count() > MAX_NOTE_LEN {
        return Err(TaskError::NoteTooShort);
    }
    if real_word_count(n) < MIN_REAL_WORDS {
        return Err(TaskError::NotRealWords);
    }
    Ok(n.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskList(Vec<Task>);

impl TaskList {
    pub fn new(texts: &[String]) -> Result<Self, TaskError> {
        let mut list = Self::default();
        for t in texts {
            list.add(t)?;
        }
        Ok(list)
    }

    pub fn tasks(&self) -> &[Task] {
        &self.0
    }

    pub fn add(&mut self, text: &str) -> Result<u32, TaskError> {
        if self.0.len() >= MAX_TASKS {
            return Err(TaskError::TooMany);
        }
        let text = clean_text(text)?;
        let id = self.0.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        self.0.push(Task { id, text, state: TaskState::Open });
        Ok(id)
    }

    /// A live (not dropped) task.
    fn live_mut(&mut self, id: u32) -> Result<&mut Task, TaskError> {
        let task = self.0.iter_mut().find(|t| t.id == id).ok_or(TaskError::NotFound)?;
        if matches!(task.state, TaskState::Dropped { .. }) {
            return Err(TaskError::Dropped);
        }
        Ok(task)
    }

    pub fn edit(&mut self, id: u32, text: &str) -> Result<(), TaskError> {
        let text = clean_text(text)?;
        let task = self.live_mut(id)?;
        if task.text != text {
            task.text = text;
            task.state = TaskState::Open;
        }
        Ok(())
    }

    pub fn tick(&mut self, id: u32, note: &str) -> Result<(), TaskError> {
        let note = clean_note(note)?;
        self.live_mut(id)?.state = TaskState::Done { note };
        Ok(())
    }

    pub fn untick(&mut self, id: u32) -> Result<(), TaskError> {
        self.live_mut(id)?.state = TaskState::Open;
        Ok(())
    }

    pub fn drop_task(&mut self, id: u32, reason: &str) -> Result<(), TaskError> {
        let reason = clean_note(reason)?;
        self.live_mut(id)?.state = TaskState::Dropped { reason };
        Ok(())
    }

    /// At least one task is left, and every task that's left is done.
    pub fn all_done(&self) -> bool {
        let mut live = self.0.iter().filter(|t| !matches!(t.state, TaskState::Dropped { .. })).peekable();
        live.peek().is_some() && live.all(|t| matches!(t.state, TaskState::Done { .. }))
    }

    pub fn open_texts(&self) -> Vec<String> {
        self.0.iter().filter(|t| t.state == TaskState::Open).map(|t| t.text.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(texts: &[&str]) -> TaskList {
        TaskList::new(&texts.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn word_count_needs_letters_or_digits() {
        assert_eq!(word_count("wrote the intro"), 3);
        assert_eq!(word_count("  done  "), 1);
        assert_eq!(word_count("a - b . c"), 3);
        assert_eq!(word_count("- . ! ?"), 0);
    }

    #[test]
    fn new_trims_and_validates() {
        let l = list(&["  write intro ", "fix chart"]);
        assert_eq!(l.tasks()[0].text, "write intro");
        assert_eq!(l.tasks()[1].id, 2);
        assert_eq!(TaskList::new(&["   ".into()]).unwrap_err(), TaskError::InvalidText);
        assert_eq!(TaskList::new(&["x".repeat(121)]).unwrap_err(), TaskError::InvalidText);
        assert!(TaskList::new(&["x".repeat(120)]).is_ok());
    }

    #[test]
    fn at_most_twenty_tasks_counting_dropped() {
        let mut l = list(&[]);
        for i in 0..20 {
            l.add(&format!("task {i}")).unwrap();
        }
        assert_eq!(l.add("one more").unwrap_err(), TaskError::TooMany);
        l.drop_task(1, "not needed any more").unwrap();
        assert_eq!(l.add("one more").unwrap_err(), TaskError::TooMany);
    }

    #[test]
    fn tick_needs_three_words_and_at_most_200_chars() {
        let mut l = list(&["write intro"]);
        assert_eq!(l.tick(1, "done").unwrap_err(), TaskError::NoteTooShort);
        assert_eq!(l.tick(1, "did it").unwrap_err(), TaskError::NoteTooShort);
        assert_eq!(l.tick(1, &format!("a b {}", "c".repeat(200))).unwrap_err(), TaskError::NoteTooShort);
        l.tick(1, " wrote the intro ").unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Done { note: "wrote the intro".into() });
    }

    #[test]
    fn notes_need_two_different_real_words() {
        assert_eq!(clean_note("wrote the intro").unwrap(), "wrote the intro");
        assert_eq!(clean_note("asjasas sasaasa sasasa").unwrap_err(), TaskError::NotRealWords);
        assert_eq!(clean_note("the the the").unwrap_err(), TaskError::NotRealWords);
        assert!(clean_note("wrote teh intro").is_ok());
        assert!(clean_note("Fixed the CHART!").is_ok());
        assert!(clean_note("didn't finish it").is_ok());
        assert_eq!(clean_note("done").unwrap_err(), TaskError::NoteTooShort);
        // Length is checked before real words.
        assert_eq!(clean_note("asjasas sasaasa").unwrap_err(), TaskError::NoteTooShort);
    }

    #[test]
    fn tick_and_drop_reject_gibberish() {
        let mut l = list(&["write intro", "fix chart"]);
        assert_eq!(l.tick(1, "asjasas sasaasa sasasa").unwrap_err(), TaskError::NotRealWords);
        assert_eq!(l.drop_task(2, "asjasas sasaasa sasasa").unwrap_err(), TaskError::NotRealWords);
        assert_eq!(l.tasks()[1].state, TaskState::Open);
    }

    #[test]
    fn check_note_counts_words_and_distinct_real_words() {
        assert_eq!(check_note("wrote teh intro"), NoteCheck { words: 3, real_words: 2, ok: true });
        assert_eq!(check_note("the the the"), NoteCheck { words: 3, real_words: 1, ok: false });
        assert_eq!(check_note("asjasas sasaasa sasasa"), NoteCheck { words: 3, real_words: 0, ok: false });
        assert_eq!(check_note("  done  "), NoteCheck { words: 1, real_words: 1, ok: false });
        assert_eq!(check_note(""), NoteCheck { words: 0, real_words: 0, ok: false });
        let long = format!("wrote the intro {}", "x".repeat(200));
        assert!(!check_note(&long).ok);
    }

    #[test]
    fn check_note_serializes_camel_case() {
        let json = serde_json::to_string(&check_note("wrote the intro")).unwrap();
        assert_eq!(json, r#"{"words":3,"realWords":3,"ok":true}"#);
    }

    #[test]
    fn untick_clears_the_note() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        l.untick(1).unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Open);
    }

    #[test]
    fn editing_a_ticked_task_unticks_it_unless_text_is_unchanged() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        l.edit(1, "write intro").unwrap();
        assert!(matches!(l.tasks()[0].state, TaskState::Done { .. }));
        l.edit(1, "write intro and outro").unwrap();
        assert_eq!(l.tasks()[0].state, TaskState::Open);
        assert_eq!(l.tasks()[0].text, "write intro and outro");
    }

    #[test]
    fn drop_needs_a_reason_and_is_final() {
        let mut l = list(&["write intro", "fix chart"]);
        assert_eq!(l.drop_task(2, "nope").unwrap_err(), TaskError::NoteTooShort);
        l.drop_task(2, "chart is not needed").unwrap();
        assert_eq!(l.tick(2, "fixed the chart").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.edit(2, "fix chart again").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.untick(2).unwrap_err(), TaskError::Dropped);
        assert_eq!(l.drop_task(2, "dropping it twice now").unwrap_err(), TaskError::Dropped);
        assert_eq!(l.tick(9, "no such task here").unwrap_err(), TaskError::NotFound);
    }

    #[test]
    fn all_done_ignores_dropped_but_needs_one_remaining() {
        let mut l = list(&["write intro", "fix chart"]);
        assert!(!l.all_done());
        l.tick(1, "wrote the intro").unwrap();
        assert!(!l.all_done());
        l.drop_task(2, "chart is not needed").unwrap();
        assert!(l.all_done());

        let mut gone = list(&["write intro"]);
        gone.drop_task(1, "changed my mind entirely").unwrap();
        assert!(!gone.all_done());
        assert!(!list(&[]).all_done());
    }

    #[test]
    fn open_texts_lists_only_open_tasks() {
        let mut l = list(&["a task", "b task", "c task"]);
        l.tick(1, "finished task a").unwrap();
        l.drop_task(3, "task c not needed").unwrap();
        assert_eq!(l.open_texts(), vec!["b task".to_string()]);
    }

    #[test]
    fn serializes_as_a_plain_array() {
        let mut l = list(&["write intro"]);
        l.tick(1, "wrote the intro").unwrap();
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(json, r#"[{"id":1,"text":"write intro","state":{"type":"done","note":"wrote the intro"}}]"#);
        assert_eq!(serde_json::from_str::<TaskList>(&json).unwrap(), l);
    }
}
