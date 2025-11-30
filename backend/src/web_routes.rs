use crate::api_state::AppState;
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::{context, Template};

/// GET / - Redirect to dashboard
#[get("/")]
pub fn index() -> Redirect {
    Redirect::to("/dashboard")
}

/// GET /dashboard - Dashboard page
#[get("/dashboard")]
pub fn dashboard(_state: &State<AppState>) -> Template {
    Template::render(
        "dashboard",
        context! {
            title: "Dashboard",
            active_tab: "dashboard",
        },
    )
}

/// GET /configuration - Configuration page
#[get("/configuration")]
pub fn configuration(_state: &State<AppState>) -> Template {
    Template::render(
        "configuration",
        context! {
            title: "Configuration",
            active_tab: "configuration",
        },
    )
}

/// GET /logs - Logs page
#[get("/logs")]
pub fn logs(_state: &State<AppState>) -> Template {
    Template::render(
        "logs",
        context! {
            title: "Logs",
            active_tab: "logs",
        },
    )
}

/// GET /help - Help page
#[get("/help")]
pub fn help() -> Template {
    Template::render(
        "help",
        context! {
            title: "Help",
            active_tab: "help",
        },
    )
}
