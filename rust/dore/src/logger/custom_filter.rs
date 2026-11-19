use log::LevelFilter;
use log4rs::filter::Response;

#[derive(Debug, Clone)]
pub(crate) struct CustomLogLevelFilter {
    app_crates: Vec<String>,
    regular_level: LevelFilter,
}

impl CustomLogLevelFilter {
    pub fn new(app_crates: impl IntoIterator<Item = impl AsRef<str>>, regular_level: LevelFilter) -> Self {
        Self {
            app_crates: app_crates.into_iter().map(|c| c.as_ref().to_owned()).collect(),
            regular_level,
        }
    }
}

impl log4rs::filter::Filter for CustomLogLevelFilter {
    fn filter(&self, record: &log::Record) -> log4rs::filter::Response {
        let target = record.target();
        for crate_ in &self.app_crates {
            if target.starts_with(crate_) { // NOTE: Split target by '::' and match exact name with HashSet?
                return Response::Accept;
            }
        }

        if record.level() <= self.regular_level {
            Response::Accept
        }
        else {
            Response::Reject
        }
    }
}
