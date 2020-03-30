extern crate log;

use serde::Deserialize;
use std::cmp::Ordering;

mod api;

use api::*;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct PagerDutyUserGroups {
    pub users: Vec<PagerDutyUser>,
    pub depth: u8,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct EscalationPolicy {
    pub id: String,
    pub description: Option<String>,
    pub policy_name: String,
    pub oncall_groups: Vec<PagerDutyUserGroups>,
    pub services: Vec<String>,
}

impl EscalationPolicy {
    #[cfg(test)]
    fn new(id: &str, policy_name: &str) -> Self {
        let formatted_name = format!("oncall-{}", policy_name);
        EscalationPolicy {
            id: id.to_string(),
            description: None,
            policy_name: policy_name.to_string(),
            oncall_groups: vec![PagerDutyUserGroups {
                users: vec![PagerDutyUser {
                    id: formatted_name.clone(),
                    name: formatted_name.clone(),
                    email: formatted_name,
                }],
                depth: 1,
            }],
            services: vec![],
        }
    }
}

#[test]
fn when_different_do_not_match() {
    assert_ne!(
        EscalationPolicy::new("abc123", "policy-1"),
        EscalationPolicy::new("def456", "policy-2")
    );
}

impl PartialOrd for EscalationPolicy {
    fn partial_cmp(&self, other: &EscalationPolicy) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EscalationPolicy {
    fn cmp(&self, other: &EscalationPolicy) -> Ordering {
        self.policy_name.cmp(&other.policy_name)
    }
}

#[derive(Debug, Deserialize, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub struct PagerDutyUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

pub struct PagerDutyClient {
    api: PagerDutyApi,
}

impl PagerDutyClient {
    pub fn new(auth_token: &str) -> Self {
        let api = PagerDutyApi::new(auth_token.into());

        PagerDutyClient { api }
    }

    pub async fn fetch_policies_for_account(&self) -> Vec<EscalationPolicy> {
        self.api.get_escalation_policies().await
    }
}

#[test]
fn order_will_be_alpha() {
    let policy1 = EscalationPolicy::new("ABC123", "Connect");
    let policy2 = EscalationPolicy::new("DEF456", "Go");

    let mut vec = vec![policy1.clone(), policy2.clone()];
    vec.sort();
    assert_eq!("Connect", vec[0].policy_name);
    assert_eq!("Go", vec[1].policy_name);

    let mut vec = vec![policy2, policy1];
    vec.sort();
    assert_eq!("Connect", vec[0].policy_name);
    assert_eq!("Go", vec[1].policy_name);
}
