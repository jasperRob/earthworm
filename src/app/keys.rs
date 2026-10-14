use std::collections::HashMap;

use crossterm::event::KeyEvent;

use crate::action::Action;

/// Buffers keys between presses so multi-key bindings (e.g. `<g><g>`) can be matched
#[derive(Debug, Default)]
pub(super) struct KeySequence {
    keys: Vec<KeyEvent>,
}

impl KeySequence {
    /// Feeds a key press, returning the bound action once a binding is complete
    pub(super) fn feed(
        &mut self,
        key: KeyEvent,
        keymap: &HashMap<Vec<KeyEvent>, Action>,
    ) -> Option<Action> {
        // Single key bindings take priority and abandon any pending sequence
        if let Some(action) = keymap.get(&vec![key]) {
            self.keys.clear();
            return Some(action.clone());
        }

        self.keys.push(key);
        if let Some(action) = keymap.get(&self.keys) {
            self.keys.clear();
            return Some(action.clone());
        }

        let is_prefix = keymap
            .keys()
            .any(|k| k.starts_with(&self.keys) && k.len() > self.keys.len());
        if !is_prefix {
            self.keys.clear();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crossterm::event::KeyEvent;

    use super::KeySequence;
    use crate::{action::Action, config::parse_key_sequence};

    fn keys(raw: &str) -> Vec<KeyEvent> {
        parse_key_sequence(raw).unwrap()
    }

    fn keymap(bindings: &[(&str, Action)]) -> HashMap<Vec<KeyEvent>, Action> {
        bindings
            .iter()
            .map(|(raw, action)| (keys(raw), action.clone()))
            .collect()
    }

    fn default_keymap() -> HashMap<Vec<KeyEvent>, Action> {
        keymap(&[
            ("<j>", Action::CmdSelectNext),
            ("<g><g>", Action::CmdJumpTop),
            ("<Space><p>", Action::CmdAddProject),
            ("<Space><s>", Action::CmdAddSession),
        ])
    }

    /// Feeds each key in turn, collecting the action produced by each press
    fn feed_all(
        sequence: &mut KeySequence,
        raw: &str,
        keymap: &HashMap<Vec<KeyEvent>, Action>,
    ) -> Vec<Option<Action>> {
        keys(raw)
            .into_iter()
            .map(|key| sequence.feed(key, keymap))
            .collect()
    }

    #[test]
    fn test_single_key_binding() {
        let mut sequence = KeySequence::default();
        let actions = feed_all(&mut sequence, "<j>", &default_keymap());
        assert_eq!(actions, vec![Some(Action::CmdSelectNext)]);
        assert!(sequence.keys.is_empty());
    }

    #[test]
    fn test_two_key_sequence() {
        let mut sequence = KeySequence::default();
        let actions = feed_all(&mut sequence, "<g><g>", &default_keymap());
        assert_eq!(actions, vec![None, Some(Action::CmdJumpTop)]);
        assert!(sequence.keys.is_empty());
    }

    #[test]
    fn test_prefix_then_completion() {
        let mut sequence = KeySequence::default();
        let keymap = default_keymap();
        let actions = feed_all(&mut sequence, "<Space>", &keymap);
        assert_eq!(actions, vec![None]);
        assert_eq!(sequence.keys, keys("<Space>"));

        let actions = feed_all(&mut sequence, "<p>", &keymap);
        assert_eq!(actions, vec![Some(Action::CmdAddProject)]);
        assert!(sequence.keys.is_empty());
    }

    #[test]
    fn test_prefix_then_unbound_key_resets() {
        let mut sequence = KeySequence::default();
        let keymap = default_keymap();
        let actions = feed_all(&mut sequence, "<Space><x>", &keymap);
        assert_eq!(actions, vec![None, None]);
        assert!(sequence.keys.is_empty());

        // The abandoned prefix doesn't leak into the next sequence
        let actions = feed_all(&mut sequence, "<p>", &keymap);
        assert_eq!(actions, vec![None]);
    }

    #[test]
    fn test_unbound_key_with_empty_buffer_is_noop() {
        let mut sequence = KeySequence::default();
        let actions = feed_all(&mut sequence, "<x>", &default_keymap());
        assert_eq!(actions, vec![None]);
        assert!(sequence.keys.is_empty());
    }

    #[test]
    fn test_prefix_then_single_key_binding_fires() {
        let mut sequence = KeySequence::default();
        let actions = feed_all(&mut sequence, "<Space><j>", &default_keymap());
        assert_eq!(actions, vec![None, Some(Action::CmdSelectNext)]);
        assert!(sequence.keys.is_empty());
    }

    #[test]
    fn test_single_key_binding_shadows_sequence() {
        // A single key binding wins over any sequence it starts or continues
        let mut keymap = default_keymap();
        keymap.insert(keys("<g>"), Action::CmdJumpBottom);
        keymap.insert(keys("<Space><j>"), Action::CmdAttach);
        let mut sequence = KeySequence::default();

        let actions = feed_all(&mut sequence, "<g><g>", &keymap);
        assert_eq!(
            actions,
            vec![Some(Action::CmdJumpBottom), Some(Action::CmdJumpBottom)]
        );

        let actions = feed_all(&mut sequence, "<Space><j>", &keymap);
        assert_eq!(actions, vec![None, Some(Action::CmdSelectNext)]);
    }
}
