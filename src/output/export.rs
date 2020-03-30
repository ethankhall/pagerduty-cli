use std::collections::BTreeMap;

use serde::Serialize;

use crate::v2::EscalationPolicy;

#[derive(Debug, Serialize)]
pub struct TfStateExportData {
    escalation_policies: BTreeMap<String, String>,
    duplicates: Vec<String>,
}

impl std::default::Default for TfStateExportData {
    fn default() -> Self {
        TfStateExportData { escalation_policies: Default::default(), duplicates: Default::default() }
    }
}

impl TfStateExportData {
    pub fn add_escalation_policy(&mut self, policy: EscalationPolicy) {
        if let Some(value) = self.escalation_policies.get(&policy.policy_name) {
            if value != &policy.id {
                eprintln!("Warning! Duplicate policy with name {} found!", policy.policy_name);
                self.escalation_policies.remove(&policy.policy_name);
                self.duplicates.push(policy.policy_name);
                return;
            }
        }

        self.escalation_policies.insert(policy.policy_name, policy.id);
    }
}