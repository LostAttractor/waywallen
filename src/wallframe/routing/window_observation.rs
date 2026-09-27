use crate::settings::WindowExclusions;

pub const APPLICATION_ID: u32 = 1;
pub const TITLE: u32 = 2;
pub const APPLICATION_ID_PATTERN: u32 = 4;
pub const TITLE_PATTERN: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub generation: u64,
    pub exclusions: WindowExclusions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_and_partial_support() {
        let rules = WindowExclusions {
            application_ids: vec!["cat".into()],
            titles: vec!["title".into()],
            ..Default::default()
        };
        let mut legacy = State::default();
        assert!(legacy.reconcile(&rules).is_none());
        assert_eq!(legacy.unsupported, 3);
        assert!(legacy.accepts_legacy());
        assert!(legacy.accept(0).is_err());
        let mut partial = State {
            capabilities: Some(APPLICATION_ID),
            ..Default::default()
        };
        let config = partial.reconcile(&rules).unwrap();
        assert_eq!(config.generation, 1);
        assert!(config.exclusions.titles.is_empty());
        assert_eq!(partial.unsupported, TITLE);
        assert!(!partial.accepts_legacy());
        assert_eq!(partial.accept(0), Ok(false));
        assert_eq!(partial.accept(1), Ok(true));
        assert!(partial.accept(2).is_err());
        assert!(partial.reconcile(&rules).is_none());
    }

    #[test]
    fn generation_replacement_and_reconnect() {
        let mut state = State {
            capabilities: Some(3),
            ..Default::default()
        };
        assert_eq!(
            state
                .reconcile(&WindowExclusions::default())
                .unwrap()
                .generation,
            0
        );
        assert!(state.accepts_legacy());
        assert_eq!(state.accept(0), Ok(true));
        assert!(!state.accepts_legacy());
        let rules = WindowExclusions {
            application_ids: vec!["cat".into()],
            ..Default::default()
        };
        assert_eq!(state.reconcile(&rules).unwrap().generation, 1);
        assert_eq!(state.applied, Some(0));
        assert_eq!(state.accept(0), Ok(false));
        assert_eq!(state.accept(1), Ok(true));
        assert_eq!(
            state
                .reconcile(&WindowExclusions::default())
                .unwrap()
                .generation,
            2
        );
        assert_eq!(state.accept(1), Ok(false));
        let mut reconnected = State {
            capabilities: Some(3),
            ..Default::default()
        };
        assert_eq!(
            reconnected
                .reconcile(&WindowExclusions::default())
                .unwrap()
                .generation,
            0
        );
    }

    #[test]
    fn patterns_require_both_identity_and_pattern_capabilities() {
        let mut rules = WindowExclusions {
            application_ids: vec!["cat*".into()],
            application_id_patterns: vec!["cat*".into()],
            title_patterns: vec!["clock*".into()],
            ..Default::default()
        };
        for (caps, unsupported, ids, titles) in [
            (None, 13, false, false),
            (Some(3), 12, false, false),
            (Some(7), 8, true, false),
            (Some(15), 0, true, true),
            (Some(12), 13, false, false),
        ] {
            let mut state = State {
                capabilities: caps,
                ..Default::default()
            };
            let config = state.reconcile(&rules);
            assert_eq!(state.unsupported, unsupported);
            if let Some(config) = config {
                assert_eq!(!config.exclusions.application_id_patterns.is_empty(), ids);
                assert_eq!(!config.exclusions.title_patterns.is_empty(), titles);
            }
        }
        let mut state = State {
            capabilities: Some(15),
            ..Default::default()
        };
        assert_eq!(state.reconcile(&rules).unwrap().generation, 1);
        assert_eq!(state.accept(1), Ok(true));
        rules.title_patterns = vec!["time*".into()];
        assert_eq!(state.reconcile(&rules).unwrap().generation, 2);
        assert_eq!(state.accept(1), Ok(false));
        assert_eq!(state.accept(2), Ok(true));
    }
}

#[derive(Debug, Default)]
pub struct State {
    pub capabilities: Option<u32>,
    pub desired: Option<Config>,
    pub applied: Option<u64>,
    pub unsupported: u32,
}

impl State {
    pub fn reconcile(&mut self, rules: &WindowExclusions) -> Option<Config> {
        let caps = self.capabilities.unwrap_or(0);
        self.unsupported = if caps & APPLICATION_ID == 0 && !rules.application_ids.is_empty() {
            APPLICATION_ID
        } else {
            0
        } | if caps & TITLE == 0 && !rules.titles.is_empty() {
            TITLE
        } else {
            0
        } | if caps & (APPLICATION_ID | APPLICATION_ID_PATTERN)
            != (APPLICATION_ID | APPLICATION_ID_PATTERN)
            && !rules.application_id_patterns.is_empty()
        {
            APPLICATION_ID_PATTERN
        } else {
            0
        } | if caps & (TITLE | TITLE_PATTERN) != (TITLE | TITLE_PATTERN)
            && !rules.title_patterns.is_empty()
        {
            TITLE_PATTERN
        } else {
            0
        };
        self.capabilities?;
        let exclusions = WindowExclusions {
            application_ids: if caps & APPLICATION_ID != 0 {
                rules.application_ids.clone()
            } else {
                Vec::new()
            },
            titles: if caps & TITLE != 0 {
                rules.titles.clone()
            } else {
                Vec::new()
            },
            application_id_patterns: if caps & (APPLICATION_ID | APPLICATION_ID_PATTERN)
                == (APPLICATION_ID | APPLICATION_ID_PATTERN)
            {
                rules.application_id_patterns.clone()
            } else {
                Vec::new()
            },
            title_patterns: if caps & (TITLE | TITLE_PATTERN) == (TITLE | TITLE_PATTERN) {
                rules.title_patterns.clone()
            } else {
                Vec::new()
            },
        };
        if self
            .desired
            .as_ref()
            .is_some_and(|config| config.exclusions == exclusions)
        {
            return None;
        }
        let generation =
            self.desired
                .as_ref()
                .map_or(u64::from(!exclusions.is_empty()), |config| {
                    config
                        .generation
                        .checked_add(1)
                        .expect("window observation generation exhausted")
                });
        let config = Config {
            generation,
            exclusions,
        };
        self.desired = Some(config.clone());
        Some(config)
    }

    pub fn accepts_legacy(&self) -> bool {
        self.capabilities.is_none()
            || (self.applied.is_none()
                && self
                    .desired
                    .as_ref()
                    .is_some_and(|config| config.generation == 0))
    }

    pub fn accept(&mut self, generation: u64) -> Result<bool, &'static str> {
        let desired = self
            .desired
            .as_ref()
            .ok_or("window observation was not negotiated")?;
        if generation > desired.generation {
            return Err("future window observation generation");
        }
        if generation < desired.generation {
            return Ok(false);
        }
        self.applied = Some(generation);
        Ok(true)
    }
}
