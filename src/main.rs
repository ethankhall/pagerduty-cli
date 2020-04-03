use lazy_static::lazy_static;
use std::sync::Mutex;

use clap::{clap_app, crate_version, ArgMatches};
use dotenv::dotenv;
use flexi_logger::{LevelFilter, LogSpecBuilder, Logger};
use regex::Regex;

mod output;
mod progress;
mod v2;

use crate::v2::EscalationPolicy;

lazy_static! {
    static ref DEBUG_LEVEL: Mutex<i32> = Mutex::new(0);
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    dotenv().ok();

    let is_number = |arg: String| match arg.parse::<u8>() {
        Ok(_) => Ok(()),
        Err(_) => Err(format!("`{}` is not a number (0-255)", arg)),
    };

    let matches = clap_app!(("pagerduty-cli") =>
        (version: crate_version!())
        (about: "PagerDuty CLI")
        (@setting SubcommandRequiredElseHelp)
        (@setting ColorAuto)
        (@setting VersionlessSubcommands)
        (@arg API_TOKEN: -a --("api-token") +global +takes_value env("PAGERDUTY_TOKEN") "A PagerDuty API Token to valid for READ access")
        (@group logging =>
            (@arg debug: -v --verbose +global +multiple "Increasing verbosity")
            (@arg warn: -w --warn +global "Only display warning messages")
            (@arg quite: -q --quite +global "Only error output will be displayed")
        )
        (@subcommand who =>
            (name: "who-is-oncall")
            (alias: "who")
            (alias: "oncall")
            (about: "List who is Oncall")
            (@arg include: -i --include +takes_value +multiple "Regex that when matches will include the policy. Regex syntax: https://docs.rs/regex/1.3.6/regex/#syntax")
            (@arg exclude: -x --exclude +takes_value +multiple "Regex that when matches will exclude the policy. Include takes precedence.  Regex syntax: https://docs.rs/regex/1.3.6/regex/#syntax")
            (@arg format: -f --format +takes_value default_value("tree") possible_value[tree json csv] "Format the Escalation oncalls should be exported.")
            (@arg depth: --depth +takes_value {is_number} "How far down the Escalation Policy should be printed?")
        )
        (@subcommand export =>
            (name: "export")
            (about: "Export escalation policy to disk")
            (@arg dest: -o --output +takes_value default_value("-") "Where to save the output. Use `-` for stdout.")
            (@arg format: -f --filter +takes_value possible_value[tfstate] "Only show Escalation Policies that contain the string.")
        )
    ).get_matches();

    let level_filter = match (
        matches.is_present("quite"),
        matches.is_present("warn"),
        matches.occurrences_of("debug"),
    ) {
        (true, _, _) => LevelFilter::Error,
        (false, true, _) => LevelFilter::Warn,
        (false, false, 0) => LevelFilter::Info,
        (false, false, 1) => LevelFilter::Debug,
        (false, false, _) => LevelFilter::Trace,
    };

    let mut builder = LogSpecBuilder::new(); // default is LevelFilter::Off
    builder.default(level_filter);

    Logger::with(builder.build())
        .format(custom_log_format)
        .start()
        .unwrap();

    let pagerduty_client = v2::PagerDutyClient::new(matches.value_of("API_TOKEN").unwrap());

    match matches.subcommand() {
        ("who-is-oncall", Some(arg_matches)) => {
            who_is_oncall(pagerduty_client, &arg_matches).await;
        }
        ("export", Some(arg_matches)) => {
            export_escilation_policies(pagerduty_client, &arg_matches).await;
        }
        _ => unreachable!(),
    };

    Ok(())
}

async fn export_escilation_policies(client: v2::PagerDutyClient, args: &ArgMatches<'_>) {
    let policies = client.fetch_policies_for_account().await;
    let mut tf_state = output::tfstate::TfStateExportData::default();

    for policy in policies {
        tf_state.add_escalation_policy(policy);
    }

    let dest = args.value_of("dest").unwrap();
    let output = serde_json::to_string_pretty(&tf_state).unwrap();
    output::write_file(dest, &output).ok();
}

async fn who_is_oncall(client: v2::PagerDutyClient, args: &ArgMatches<'_>) {
    let include_vec: Vec<Regex> = args
        .values_of("include")
        .unwrap_or_default()
        .map(|i| Regex::new(i).unwrap_or_else(|_| panic!("`{}` to be valid regex", i)))
        .collect();

    let exclude_vec: Vec<Regex> = args
        .values_of("exclude")
        .unwrap_or_default()
        .map(|i| Regex::new(i).unwrap_or_else(|_| panic!("`{}` to be valid regex", i)))
        .collect();

    let max_depth = args
        .value_of("depth")
        .map(|s| s.parse::<u8>().unwrap())
        .unwrap_or(255);

    let mut policies = Vec::new();
    for policy in client.fetch_policies_for_account().await {
        if policy_should_be_included(&include_vec, &exclude_vec, &policy) {
            policies.push(policy);
        }
    }
    policies.sort();

    let usergroup_filter = |usergroup: &crate::v2::PagerDutyUserGroups| {
        if usergroup.depth > max_depth {
            return false;
        }
        true
    };

    let output = match args.value_of("format").unwrap() {
        "tree" => output::build_tree_output(policies, usergroup_filter),
        "json" => output::build_json_output(policies, usergroup_filter),
        "csv" => output::build_csv_output(policies, usergroup_filter),
        _ => unreachable!(),
    };

    println!("{}", output);
}

enum PolicyMatch {
    Yes,
    No,
    NotProvided,
}

fn policy_should_be_included(
    includes_vec: &[Regex],
    excludes_vec: &[Regex],
    policy: &EscalationPolicy,
) -> bool {
    match does_policy_match(includes_vec, &policy) {
        PolicyMatch::Yes => return true,
        PolicyMatch::No => return false,
        PolicyMatch::NotProvided => {}
    };

    match does_policy_match(excludes_vec, &policy) {
        PolicyMatch::Yes => false,
        PolicyMatch::No => true,
        PolicyMatch::NotProvided => true,
    }
}

fn does_policy_match(inputs: &[Regex], policy: &EscalationPolicy) -> PolicyMatch {
    if !inputs.is_empty() {
        for exp in inputs {
            if exp.is_match(&policy.policy_name) {
                return PolicyMatch::Yes;
            }
        }
        return PolicyMatch::No;
    }

    PolicyMatch::NotProvided
}

fn custom_log_format(
    w: &mut dyn std::io::Write,
    now: &mut flexi_logger::DeferredNow,
    record: &flexi_logger::Record,
) -> Result<(), std::io::Error> {
    use flexi_logger::style;

    let level = record.level();
    write!(
        w,
        "[{}] {} [{}:{}] {}",
        style(level, now.now().format("%Y-%m-%d %H:%M:%S%.6f %:z")),
        style(level, level),
        record.module_path().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        style(level, &record.args())
    )
}
