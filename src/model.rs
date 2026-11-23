use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Up,
    Working,
    Booting,
    Flapping,
    Unsure,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Green,
    Yellow,
    Red,
}

impl Status {
    pub fn color(self) -> Color {
        match self {
            Status::Up => Color::Green,
            Status::Down => Color::Red,
            Status::Working | Status::Booting | Status::Flapping | Status::Unsure => Color::Yellow,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Status::Up => "up",
            Status::Working => "working",
            Status::Booting => "booting",
            Status::Flapping => "flapping",
            Status::Unsure => "unsure",
            Status::Down => "down",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Status::Up => 0,
            Status::Unsure => 1,
            Status::Working => 2,
            Status::Booting => 3,
            Status::Flapping => 4,
            Status::Down => 5,
        }
    }

    pub fn worst<I: IntoIterator<Item = Status>>(statuses: I) -> Option<Status> {
        statuses.into_iter().max_by_key(|s| s.rank())
    }
}

impl Color {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Color::Green => [0x33, 0xd1, 0x7a],
            Color::Yellow => [0xf6, 0xd3, 0x2d],
            Color::Red => [0xe0, 0x1b, 0x24],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Aggregate,
    PerService,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ButtonView {
    pub name: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemView {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    pub actions: Vec<ButtonView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceView {
    pub id: String,
    pub name: String,
    pub status: Status,
    pub color: Color,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<ButtonView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ItemView>,
    pub busy: bool,
    pub last_action: Option<ActionResult>,
    pub children: Vec<ServiceView>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    #[serde(default = "rollup_default", skip_serializing_if = "is_true")]
    pub rollup: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub one_click: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub layout: Layout,
    #[serde(default = "shell_panel_default")]
    pub shell_panel: bool,
    #[serde(default = "icon_size_default")]
    pub icon_size: u32,
    #[serde(default = "icon_size_min_default")]
    pub icon_size_min: u32,
    #[serde(default = "icon_size_max_default")]
    pub icon_size_max: u32,
    #[serde(default)]
    pub hide_healthy: bool,
    pub status: Status,
    pub color: Color,
    pub services: Vec<ServiceView>,
}

fn shell_panel_default() -> bool {
    true
}

fn icon_size_default() -> u32 {
    100
}

fn icon_size_min_default() -> u32 {
    crate::config::ICON_SIZE_MIN
}

fn icon_size_max_default() -> u32 {
    crate::config::ICON_SIZE_MAX
}

fn rollup_default() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

impl Snapshot {
    pub fn flat(&self) -> Vec<&ServiceView> {
        fn walk<'a>(views: &'a [ServiceView], out: &mut Vec<&'a ServiceView>) {
            for view in views {
                out.push(view);
                walk(&view.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.services, &mut out);
        out
    }
}
