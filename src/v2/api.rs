use crate::progress::{ProgressBarHelper, ProgressBarType};
use log::{error, info, warn};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;

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

fn make_escalation_policies(
    source_escalation_policies: Vec<EscalationPolicyModel>,
    source_users: Vec<UserModel>,
    source_oncalls: Vec<OnCallModel>,
    source_services: Vec<ServiceModel>,
) -> Vec<super::EscalationPolicy> {
    let mut return_policies = Vec::new();

    let mut users_map = BTreeMap::new();
    for user in source_users {
        users_map.insert(user.id.clone(), user);
    }

    for esc_model in source_escalation_policies {
        let mut users = Vec::new();
        let mut services = Vec::new();

        let esc_id = esc_model.id.clone();
        let mut oncall_for_esc = Vec::new();

        for oncall in source_oncalls.iter() {
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
                    if let Some(user) = users_map.get(&oncall.user.id) {
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

        for service in &source_services {
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

impl PagerDutyApi {
    pub(crate) fn new(auth_token: String) -> Self {
        PagerDutyApi { auth_token }
    }

    pub(crate) async fn get_escalation_policies(&self) -> Vec<super::EscalationPolicy> {
        let pb = ProgressBarHelper::new(ProgressBarType::SizedProgressBar(
            0,
            "{prefix:.bold.dim} {spinner:.green} {pos:>2}/{len:>2} Fetching data from PagerDuty",
        ));

        let api_resolver = ApiResolver::new(&self.auth_token, &pb);

        let (policies, oncalls, users, services) = tokio::join!(
            self.fetch_policies_for_account(&api_resolver),
            self.fetch_oncalls_for_account(&api_resolver),
            self.fetch_users_for_account(&api_resolver),
            self.fetch_services_for_account(&api_resolver)
        );

        pb.done();

        make_escalation_policies(policies, users, oncalls, services)
    }

    async fn fetch_services_for_account(
        &self,
        api_resolver: &ApiResolver<'_>,
    ) -> Vec<ServiceModel> {
        let some_response = api_resolver
            .make_api_call("https://api.pagerduty.com/services", &[])
            .await;

        let mut outputs: Vec<ServiceModel> = Vec::new();

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Services(services) = obj {
                        for service in services {
                            outputs.push(service);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
        outputs
    }

    async fn fetch_users_for_account(&self, api_resolver: &ApiResolver<'_>) -> Vec<UserModel> {
        let some_response = api_resolver
            .make_api_call("https://api.pagerduty.com/users", &[])
            .await;

        let mut outputs: Vec<UserModel> = Vec::new();

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Users(users) = obj {
                        for user in users {
                            outputs.push(user);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
        outputs
    }

    async fn fetch_oncalls_for_account(&self, api_resolver: &ApiResolver<'_>) -> Vec<OnCallModel> {
        let some_response = api_resolver
            .make_api_call("https://api.pagerduty.com/oncalls", &["targets"])
            .await;

        let mut outputs: Vec<OnCallModel> = Vec::new();

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::Oncalls(oncalls) = obj {
                        for oncall in oncalls {
                            outputs.push(oncall);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }
        outputs
    }

    async fn fetch_policies_for_account(
        &self,
        api_resolver: &ApiResolver<'_>,
    ) -> Vec<EscalationPolicyModel> {
        let some_response = api_resolver
            .make_api_call(
                "https://api.pagerduty.com/escalation_policies",
                &["targets"],
            )
            .await;

        let mut outputs: Vec<EscalationPolicyModel> = Vec::new();

        match some_response {
            Some(objs) => {
                for obj in objs {
                    if let PagerDutyObjects::EscalationPolicies(policies) = obj {
                        for policy in policies {
                            outputs.push(policy);
                        }
                    }
                }
            }
            None => {
                warn!("Unable to get policies. Skipping.");
            }
        }

        outputs
    }
}

struct ApiResolver<'a> {
    pb: &'a ProgressBarHelper,
    client: Client,
    auth_token: &'a str,
}

impl<'a> ApiResolver<'a> {
    pub(crate) fn new(auth_token: &'a str, pb: &'a ProgressBarHelper) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Should be able to make client");

        ApiResolver {
            pb,
            client,
            auth_token,
        }
    }

    async fn make_api_call(&self, url: &str, includes: &[&str]) -> Option<Vec<PagerDutyObjects>> {
        let mut poll_queue: Vec<u32> = vec![0];
        let mut response_array = Vec::new();

        while !poll_queue.is_empty() {
            self.pb.inc_length();
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
                poll_queue.push(offset + PAGE_SIZE);
            }
            self.pb.inc();
        }

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
