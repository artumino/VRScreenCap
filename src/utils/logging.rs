use log::LevelFilter;
use log4rs::{
    append::{
        console::ConsoleAppender,
        rolling_file::{
            policy::compound::{
                roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
            },
            RollingFileAppender,
        },
    },
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
    Config,
};

pub fn setup_logging() -> anyhow::Result<()> {
    let trigger_size = 30_000_000_u64;
    let trigger = Box::new(SizeTrigger::new(trigger_size));

    let roller_pattern = "logs/output_{}.log";
    let roller_count = 5;
    let roller_base = 1;
    let roller = Box::new(
        FixedWindowRoller::builder()
            .base(roller_base)
            .build(roller_pattern, roller_count)
            .unwrap(),
    );

    let compound_policy = Box::new(CompoundPolicy::new(trigger, roller));

    let logfile = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {f}:{L} — {m}{n}",
        )))
        .build("logs/output.log", compound_policy)?;

    let stdout = ConsoleAppender::builder().build();

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .logger(
            Logger::builder()
                .appender("logfile")
                .build("panic", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appender("logfile")
                .build("vr-screen-cap", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appender("logfile")
                .build("vr_screen_cap_core", LevelFilter::Info),
        )
        .logger(
            Logger::builder()
                .appender("logfile")
                .build("wgpu", LevelFilter::Warn),
        )
        .logger(
            Logger::builder()
                .appender("logfile")
                .build("wgpu-hal", LevelFilter::Warn),
        )
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;
    log_panics::init();

    Ok(())
}
