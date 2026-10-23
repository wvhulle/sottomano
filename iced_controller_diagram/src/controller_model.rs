#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ControllerModel {
    DualSense,
    #[default]
    XboxSeries,
    SwitchPro,
}

impl ControllerModel {
    pub const ALL: [ControllerModel; 3] = [
        ControllerModel::DualSense,
        ControllerModel::XboxSeries,
        ControllerModel::SwitchPro,
    ];
}
