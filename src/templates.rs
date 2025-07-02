use askama::Template;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {}

#[derive(Template)]
#[template(path = "keys.html")]
pub struct KeysTemplate {}

#[derive(Template)]
#[template(path = "monitor.html")]
pub struct MonitorTemplate {}
