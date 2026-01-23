use std::collections::{BTreeMap, HashMap};

use crate::constants;

#[derive(Clone, Debug)]
pub struct PrologModules {
    available_modules: BTreeMap<&'static str, &'static str>,
    enabled_modules: Vec<&'static str>,
}

impl PrologModules {
    pub fn new() -> Self {
        let available_modules: BTreeMap<&'static str, &'static str> =
            constants::PROLOG_MODULES.iter().cloned().collect();
        let enabled_modules = available_modules.keys().copied().collect();
        Self {
            available_modules,
            enabled_modules,
        }
    }
    pub fn to_merged_string(&self) -> String {
        self.available_modules

            .iter()
            .filter(|(k, _)| self.enabled_modules.contains(k))
            .map(|(_,v)| v)
            .copied()
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
