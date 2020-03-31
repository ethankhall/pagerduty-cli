use std::sync::Mutex;
use lazy_static::lazy_static;

use clap::{clap_app, crate_version, ArgMatches};
use dotenv::dotenv;
use flexi_logger::{LevelFilter, LogSpecBuilder, Logger};

mod progress;
mod v2;
mod output;

lazy_static! {
    static ref DEBUG_LEVEL: Mutex<i32> = Mutex::new(0);
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    dotenv().ok();

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
            (about: "List who is Oncall")
            (@arg filter: -f --filter +takes_value "Only show Escilation Policies that contain the string")
        )
        (@subcommand export =>
            (name: "export")
            (about: "Export escilation policy to disk")
            (@arg dest: -o --output +takes_value default_value("-") "Where to save the output. Use `-` for stdout.")
            (@arg format: -f --filter +takes_value possible_value[tfstate] "Only show Escilation Policies that contain the string")
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
        ("who-is-oncall", Some(arg_matches)) => { who_is_oncall(pagerduty_client, &arg_matches).await; }
        ("export", Some(arg_matches)) => { export_escilation_policies(pagerduty_client, &arg_matches).await; }
        _ => unreachable!(),
    };

    Ok(())
}

async fn export_escilation_policies(client: v2::PagerDutyClient, args: &ArgMatches<'_>) {
    let policies = client.fetch_policies_for_account().await;
    let mut tf_state = output::export::TfStateExportData::default();

    for policy in policies {
        tf_state.add_escalation_policy(policy);
    }

    let dest = args.value_of("dest").unwrap();
    let output = serde_json::to_string_pretty(&tf_state).unwrap();
    output::write_file(dest, &output).ok();
}

async fn who_is_oncall(client: v2::PagerDutyClient, args: &ArgMatches<'_>) {
    let mut policies = client.fetch_policies_for_account().await;
    policies.sort();

    let tree = output::tree::TreePrinter::default();
    let filter = args.value_of("filter");

    for policy in policies {

        if let Some(filter) = filter {
            if !policy.policy_name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        let root = tree.add_line(format!("Escilation Policy - {}", policy.policy_name));
        let oncalls = root.add_line("Oncalls".into());
        for group in policy.oncall_groups {
            oncalls.add_line(format!("Level {} - {}", group.depth, group.users.iter().map(|user| user.name.clone()).collect::<Vec<String>>().join(", ")));
        }
    }

    tree.print();
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
