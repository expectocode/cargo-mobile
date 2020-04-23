use crate::opts;
use colored::Colorize as _;
use std::fmt::{Debug, Display};
use structopt::{
    clap::{self, AppSettings},
    StructOpt,
};

pub static SETTINGS: &'static [AppSettings] = &[
    AppSettings::ColoredHelp,
    AppSettings::DeriveDisplayOrder,
    AppSettings::SubcommandRequiredElseHelp,
    AppSettings::VersionlessSubcommands,
];

pub fn bin_name(name: &str) -> String {
    format!("cargo {}", name)
}

#[derive(Clone, Copy, Debug, StructOpt)]
pub struct GlobalFlags {
    #[structopt(
        short = "v",
        long = "verbose",
        about = "Make life louder",
        global = true,
        multiple = true,
        parse(from_occurrences = opts::NoiseLevel::from_occurrences),
    )]
    pub noise_level: opts::NoiseLevel,
    #[structopt(
        long = "non-interactive",
        about = "Go with the flow",
        global = true,
        parse(from_flag = opts::Interactivity::from_flag),
    )]
    pub interactivity: opts::Interactivity,
}

#[derive(Clone, Copy, Debug, StructOpt)]
pub struct Clobbering {
    #[structopt(
        long = "force",
        about = "Clobber files with no remorse",
        parse(from_flag = opts::Clobbering::from_flag),
    )]
    pub clobbering: opts::Clobbering,
}

#[derive(Clone, Copy, Debug, StructOpt)]
pub struct Profile {
    #[structopt(
        long = "release",
        about = "Build with release optimizations",
        parse(from_flag = opts::Profile::from_flag),
    )]
    pub profile: opts::Profile,
}

pub type TextWrapper = textwrap::Wrapper<'static, textwrap::NoHyphenation>;

#[derive(Clone, Copy, Debug)]
pub enum Label {
    Error,
    ActionRequest,
}

impl Label {
    pub fn color(&self) -> colored::Color {
        match self {
            Self::Error => colored::Color::BrightRed,
            Self::ActionRequest => colored::Color::BrightMagenta,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::ActionRequest => "action request",
        }
    }
}

#[derive(Debug)]
pub struct Report {
    label: Label,
    msg: String,
    details: String,
}

impl Report {
    pub fn new(label: Label, msg: impl Display, details: impl Display) -> Self {
        Self {
            label,
            msg: format!("{}", msg),
            details: format!("{}", details),
        }
    }

    pub fn error(msg: impl Display, details: impl Display) -> Self {
        Self::new(Label::Error, msg, details)
    }

    pub fn action_request(msg: impl Display, details: impl Display) -> Self {
        Self::new(Label::ActionRequest, msg, details)
    }

    pub fn render(&self, wrapper: &TextWrapper, interactivity: opts::Interactivity) -> String {
        fn render_head(wrapper: &TextWrapper, label: impl Display, msg: &str) -> String {
            wrapper.fill(&format!("{}: {}", label, msg))
        }

        fn render_full(wrapper: &TextWrapper, head: impl Display, details: &str) -> String {
            static INDENT: &'static str = "    ";
            let wrapper = wrapper
                .clone()
                .initial_indent(INDENT)
                .subsequent_indent(INDENT);
            let details = wrapper.fill(details);
            format!("{}\n{}", head, details)
        }

        if interactivity.full() && colored::control::SHOULD_COLORIZE.should_colorize() {
            render_full(
                wrapper,
                render_head(
                    wrapper,
                    self.label.as_str().color(self.label.color()),
                    &self.msg,
                )
                .bold(),
                &self.details,
            )
        } else {
            render_full(
                wrapper,
                render_head(wrapper, self.label.as_str(), &self.msg),
                &self.details,
            )
        }
    }
}

pub trait Reportable: Debug {
    fn report(&self) -> Report;
}

pub trait Exec: Debug + StructOpt {
    type Report: Reportable;

    fn global_flags(&self) -> GlobalFlags;

    fn exec(self, wrapper: &TextWrapper) -> Result<(), Self::Report>;
}

fn get_args(name: &str) -> Vec<String> {
    let mut args: Vec<String> = std::env::args().collect();
    // Running this as a cargo subcommand gives us our name as an argument,
    // so let's just discard that...
    if args.get(1).map(String::as_str) == Some(name) {
        args.remove(1);
    }
    args
}

fn init_logging(noise_level: opts::NoiseLevel) {
    use env_logger::{Builder, Env};
    let default_level = match noise_level {
        opts::NoiseLevel::Polite => "warn",
        opts::NoiseLevel::LoudAndProud => "cargo_mobile=info,bossy=info",
        opts::NoiseLevel::FranklyQuitePedantic => "info,bossy=debug",
    };
    let env = Env::default().default_filter_or(default_level);
    Builder::from_env(env).init();
}

#[derive(Debug)]
enum Exit {
    Report(Report, opts::Interactivity),
    Clap(clap::Error),
}

impl Exit {
    fn report(reportable: impl Reportable, interactivity: opts::Interactivity) -> Self {
        log::info!("exiting with {:#?}", reportable);
        Self::Report(reportable.report(), interactivity)
    }

    fn do_the_thing(self, wrapper: TextWrapper) -> ! {
        match self {
            Self::Report(report, interactivity) => {
                eprintln!("{}", report.render(&wrapper, interactivity));
                std::process::exit(1)
            }
            Self::Clap(err) => err.exit(),
        }
    }

    fn main(inner: impl FnOnce(&TextWrapper) -> Result<(), Self>) {
        let wrapper = TextWrapper::with_splitter(textwrap::termwidth(), textwrap::NoHyphenation);
        if let Err(exit) = inner(&wrapper) {
            exit.do_the_thing(wrapper)
        }
    }
}

pub fn exec<E: Exec>(name: &str) {
    Exit::main(|wrapper| {
        let input = E::from_iter_safe(get_args(name)).map_err(Exit::Clap)?;
        let GlobalFlags {
            noise_level,
            interactivity,
        } = input.global_flags();
        init_logging(noise_level);
        input
            .exec(wrapper)
            .map_err(|report| Exit::report(report, interactivity))
    })
}