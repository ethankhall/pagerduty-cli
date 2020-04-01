pub mod tfstate;
pub mod tree;

use json::object;
use std::fs;

pub fn build_csv_output<P>(policies: Vec<crate::v2::EscalationPolicy>, mut filter: P) -> String
where
    P: FnMut(&crate::v2::PagerDutyUserGroups) -> bool,
{
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);

    wtr.write_record(&[
        "Escalation Policy ID",
        "Escalation Policy",
        "depth",
        "name",
        "email",
    ])
    .expect("To be able to write header");

    for policy in policies {
        for group in policy.oncall_groups {
            if filter(&group) {
                for user in group.users {
                    wtr.write_record(&[
                        policy.id.clone(),
                        policy.policy_name.clone(),
                        group.depth.to_string(),
                        user.name,
                        user.email,
                    ])
                    .expect("to be able to write row");
                }
            }
        }
    }

    String::from_utf8(wtr.into_inner().expect("to be able to get vec"))
        .expect("To be able to serialize CSV")
}

pub fn build_json_output<P>(policies: Vec<crate::v2::EscalationPolicy>, mut filter: P) -> String
where
    P: FnMut(&crate::v2::PagerDutyUserGroups) -> bool,
{
    let mut outputs = Vec::new();

    for policy in policies {
        for group in policy.oncall_groups {
            if filter(&group) {
                for user in group.users {
                    outputs.push(object! {
                        id: policy.id.clone(),
                        escalationPolicy: policy.policy_name.clone(),
                        depth: group.depth,
                        userName: user.name,
                        userEmail: user.email
                    });
                }
            }
        }
    }

    json::stringify_pretty(outputs, 2)
}

pub fn build_tree_output<P>(policies: Vec<crate::v2::EscalationPolicy>, mut filter: P) -> String
where
    P: FnMut(&crate::v2::PagerDutyUserGroups) -> bool,
{
    let tree = tree::TreePrinter::default();

    for policy in policies {
        let root = tree.add_line(format!("Escilation Policy - {}", policy.policy_name));
        let oncalls = root.add_line("Oncalls".into());
        for group in policy.oncall_groups {
            if filter(&group) {
                oncalls.add_line(format!(
                    "Level {} - {}",
                    group.depth,
                    group
                        .users
                        .iter()
                        .map(|user| user.to_display())
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }
        }
    }

    tree.to_string()
}

pub fn write_file(path: &str, contents: &str) -> std::io::Result<()> {
    if path == "-" {
        println!("{}", contents);
        Ok(())
    } else {
        fs::write(path, contents)
    }
}
