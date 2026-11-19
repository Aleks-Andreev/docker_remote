use log::LevelFilter;
use log4rs::Config;
use log4rs::config::{Appender, Root};
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::append::console::ConsoleAppender;
use log4rs::filter::threshold::ThresholdFilter;

mod custom_roller;
use custom_roller::CustomFileRoller;

mod custom_trigger;
use custom_trigger::CustomTrigger;

mod custom_filter;
use custom_filter::CustomLogLevelFilter;

pub(crate) fn setup_logger(verbose: u8) {
    let trigger = CustomTrigger::new(20, 10 * 1024);

    let mut log_path = dirs::data_local_dir().expect("Local dir should exist");
    log_path.push("dore.logs");
    log_path.push("app");
    let log_path = log_path.to_str().expect("Path should be UTF8");

    let roller = CustomFileRoller::new(log_path);
    let def_file_name = roller.get_default_file_name();

    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let file_message_format = "{d(%Y-%m-%d %H:%M:%S.%3f)} [{T}] {l}: '{t}' {m}{n}";
    let log_file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(file_message_format)))
        .build(def_file_name, Box::new(policy))
        .unwrap();

    let cli_message_format = PatternEncoder::new("{l}: {m}{n}");
    let console = ConsoleAppender::builder()
        .encoder(Box::new(cli_message_format))        
        .build();

    let default_log_level = match verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };

    let log_filter = CustomLogLevelFilter::new(["dore"], default_log_level);

    let config = Config::builder()
        .appender(Appender::builder().filter(Box::new(ThresholdFilter::new(LevelFilter::Warn))).build("cli", Box::new(console)))
        .appender(Appender::builder().filter(Box::new(log_filter)).build("file", Box::new(log_file)))
        .build(Root::builder()
            .appender("file")
            .appender("cli")
            .build(log::LevelFilter::Debug)
        )
        .expect("Config should be valid");

    log4rs::init_config(config).expect("This should be the single call to init_logger");
}


