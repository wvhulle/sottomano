#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ControllerModel {
    DualSense,
    #[default]
    XboxSeries,
    SwitchPro,
    SteamController,
}

impl ControllerModel {
    pub const ALL: [ControllerModel; 4] = [
        ControllerModel::DualSense,
        ControllerModel::XboxSeries,
        ControllerModel::SwitchPro,
        ControllerModel::SteamController,
    ];
}
