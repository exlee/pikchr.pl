// This file is part of pikchr.pl.
//
// pikchr.pl is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// pikchr.pl is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with pikchr.pl. If not, see <https://www.gnu.org/licenses/>.

use std::collections::BTreeMap;

use crate::constants;

#[derive(Clone, Debug)]
pub struct PrologModules {
    pub available_modules: BTreeMap<&'static str, &'static str>,
    pub enabled_modules: Vec<&'static str>,
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
            .map(|(_, v)| v)
            .copied()
            .collect::<Vec<_>>()
            .join("\n\n")
    }
    pub fn disable(&mut self, module: &str) -> &Self {
        let new_enabled: Vec<&str> = self
            .enabled_modules
            .iter()
            .cloned()
            .filter(|i| *i != module)
            .collect();
        self.enabled_modules = new_enabled;
        self
    }

    #[allow(unused)]
    pub fn enable(&mut self, module: &str) -> &Self {
        if let Some((&static_key, _)) = self.available_modules.get_key_value(module) {
            if self.enabled_modules.contains(&static_key) {
                return self;
            }
            self.enabled_modules.push(static_key);
        }
        self
    }
}
