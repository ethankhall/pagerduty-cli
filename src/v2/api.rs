use log::{error, info, warn};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;
use crate::progress::{ProgressBarHelper, ProgressBarType};

const PAGE_SIZE: u32 = 100;

#[derive(Clone, Debug, Deserialize)]
pub struct UserModel {
    pub name: String,
    pub id: String,
    #[serde(rename = "self")]
    pub self_ref: String,
    pub html_url: String,
    pub email: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceModel {
    pub name: String,
    pub id: String,
    pub escalation_policy: ModelReference,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EscalationPolicyModel {
    pub id: String,
    pub description: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelReference {
    id: String,
}

pub(crate) struct PagerDutyApi {
    client: Client,
    auth_token: String,
}

#[derive(Debug, Deserialize)]
pub struct PagerDutyResponseWrapper {
    #[serde(flatten)]
    pub obj: PagerDutyObjects,
    pub limit: u32,
    pub offset: u32,
    pub more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OnCallModel {
    pub escalation_policy: ModelReference,
    pub escalation_level: u8,
    pub user: ModelReference,
}

#[derive(Debug, Deserialize)]
pub enum PagerDutyObjects {
    #[serde(rename = "escalation_policies")]
    EscalationPolicies(Vec<EscalationPolicyModel>),
    #[serde(rename = "oncalls")]
    Oncalls(Vec<OnCallModel>),
    #[serde(rename = "users")]
    Users(Vec<UserModel>),
    #[serde(rename = "services")]
    Services(Vec<ServiceModel>),
}

struct PagerDutyState {
    users: BTreeMap<String, UserModel>,
    services: BTreeMap<String, ServiceModel>,
    escalation_policies: BTreeMap<String, EscalationPolicyModel>,
    oncalls: Vec<OnCallModel>,
}

impl Default for PagerDutyState {
    fn default() -> Self {
        PagerDutyState {
            users: BTreeMap::default(),
            services: BTreeMap::default(),
            escalation_policies: BTreeMap::default(),
            oncalls: Vec::default(),
        }
    }
}

impl PagerDutyState {
    fn make_escalation_policies(&self) -> Vec<super::EscalationPolicy> {
        let mut return_policies = Vec::new();

        for esc_model in self.escalation_policies.values() {
            let mut users = Vec::new();
            let mut services = Vec::new();

            let esc_id = esc_model.id.clone();
            let mut oncall_for_esc = Vec::new();

            for oncall in self.oncalls.iter() {
                if oncall.escalation_policy.id == esc_id {
                    oncall_for_esc.push(oncall.clone());
                }
            }

            let max_depth = oncall_for_esc
                .iter()
                .map(|x| x.escalation_level)
                .max()
                .unwrap_or(1);

            for depth in 1..=max_depth {
                let mut users_for_depth = Vec::new();
                for oncall in oncall_for_esc.iter() {
                    if oncall.escalation_level == depth {
                        if let Some(user) = self.users.get(&oncall.user.id) {
                            users_for_depth.push(super::PagerDutyUser {
                                id: user.id.clone(),
                                name: user.name.clone(),
                                email: user.email.clone(),
                            });
                        }
                    }
                }

                users.push(super::PagerDutyUserGroups {
                    users: users_for_depth,
                    depth,
                });
            }

            for service in self.services.values() {
                if service.escalation_policy.id == esc_id {
                    services.push(service.name.clone());
                }
            }

            return_policies.push(super::EscalationPolicy {
                id: esc_id,
                description: esc_model.description.clone(),
                policy_name: esc_model.name.clone(),
                oncall_groups: users,
                services,
            });
        }

        return_policies
    }

    fn add_policy(&mut self, policy: EscalationPolicyModel) {
        self.escalation_policies.insert(policy.id.clone(), policy);
    }

    fn add_oncall(&mut self, oncall: OnCallModel) {
        self.oncalls.push(oncall);
    }

    fn add_user(&mut self, user: UserModel) {
        self.users.insert(user.id.clone(), user);
    }

    fn add_service(&mut self, service: ServiceModel) {
        self.services.insert(service.id.clone(), service);
    }
}

impl PagerDutyApi {
    pub(crate) fn new(auth_token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Should be able to make client");

        PagerDutyApi { client, auth_token }
    }

    pub(crate) async fn get_escalation_policies(&self) -> Vec<super::EscalationPolicy> {
        let mut state = PagerDutyState::default();

        self.fetch_policies_for_account(&mut state).await;
        self.fetch_oncalls_for_account(&mut state).await;
        self.fetch_users_for_account(&mut state).await;
        self.fetch_services_for_account(&mut state).await;

        state.make_escalation_policies()
    }

    async fn fetch_services_for_account(&self, state: &mut PagerDutyState) {
        let some_response = self
            .make_api_call("Services", "https://api.pagerduty.com/services", &[])
            .await;

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Services(services) = obj {
                        for service in services {
                            state.add_service(service);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
    }

    async fn fetch_users_for_account(&self, state: &mut PagerDutyState) {
        let some_response = self
            .make_api_call("Users", "https://api.pagerduty.com/users", &[])
            .await;

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Users(users) = obj {
                        for user in users {
                            state.add_user(user);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
    }

    async fn fetch_oncalls_for_account(&self, state: &mut PagerDutyState) {
        let some_response = self
            .make_api_call("Oncalls", "https://api.pagerduty.com/oncalls", &["targets"])
            .await;

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Oncalls(oncalls) = obj {
                        for oncall in oncalls {
                            state.add_oncall(oncall);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
    }

    async fn fetch_policies_for_account(&self, state: &mut PagerDutyState) {
        let some_response = self
            .make_api_call(
                "Escalation Policies",
                "https://api.pagerduty.com/escalation_policies",
                &["targets"],
            )
            .await;

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::EscalationPolicies(policies) = obj {
                        for policy in policies {
                            state.add_policy(policy);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
    }

    async fn make_api_call(&self, name: &str, url: &str, includes: &[&str]) -> Option<Vec<PagerDutyObjects>> {
        let mut poll_queue: Vec<u32> = vec![0];
        let mut response_array = Vec::new();

        let pb = ProgressBarHelper::new(ProgressBarType::UnsizedProgressBar("{prefix:.bold.dim} {spinner:.green} {pos:>2} {wide_msg}"));
        pb.inc_with_message(&format!("Pages of {}", name));

        while !poll_queue.is_empty() {
            let offset = poll_queue.pop().unwrap();

            let resp = self
                .client
                .get(url)
                .query(&[
                    ("include[]", includes.join(",")),
                    ("sort_by", "name".into()),
                    ("limit", format!("{}", PAGE_SIZE)),
                    ("offset", format!("{}", offset)),
                ])
                .header("Accept", "application/vnd.pagerduty+json;version=2")
                .header("Authorization", format!("Token token={}", self.auth_token))
                .send();

            let resp = match resp.await {
                Ok(body) => body,
                Err(e) => {
                    error!("Request to PagerDuty Failed: {}", e);
                    return None;
                }
            };

            let text_body = match resp.text().await {
                Ok(body) => body,
                Err(e) => {
                    error!("Unable to get text from body: {}", e);
                    return None;
                }
            };

            let resp: PagerDutyResponseWrapper = match serde_json::from_str(&text_body) {
                Ok(body) => body,
                Err(e) => {
                    error!("Unable to parse output from PagerDuty: {}", e);
                    info!("Message: {}\n\n", text_body);
                    return None;
                }
            };

            response_array.push(resp.obj);

            if resp.more {
                pb.inc();
                poll_queue.push(offset + PAGE_SIZE);
            }
        }

        pb.done();

        print!("{}\r", termion::clear::CurrentLine);

        Some(response_array)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn do_parse<F>(path: &str, validate: F)
    where
        F: Fn(PagerDutyObjects) -> bool,
    {
        use std::fs::read_to_string;
        use std::path::PathBuf;

        let mut ep = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ep.push(format!("resources/test/{}", path));

        let test_contents = read_to_string(ep).unwrap();

        let wrapper = match serde_json::from_str::<PagerDutyResponseWrapper>(&test_contents) {
            Ok(v) => v,
            Err(e) => {
                panic!("Unable to parse JSON: {}", e);
            }
        };

        assert!(validate(wrapper.obj));
    }

    #[test]
    fn validate_escalation_policy() {
        do_parse("escalation_policy.json", |i| {
            if let PagerDutyObjects::EscalationPolicies(_) = i {
                true
            } else {
                false
            }
        });
    }

    #[test]
    fn validate_oncalls() {
        do_parse("oncalls.json", |i| {
            if let PagerDutyObjects::Oncalls(_) = i {
                true
            } else {
                false
            }
        });
    }

    #[test]
    fn validate_users() {
        do_parse("users.json", |i| {
            if let PagerDutyObjects::Users(_) = i {
                true
            } else {
                false
            }
        });
    }

    #[test]
    fn validate_services() {
        do_parse("services.json", |i| {
            if let PagerDutyObjects::Services(_) = i {
                true
            } else {
                false
            }
        });
    }
}
