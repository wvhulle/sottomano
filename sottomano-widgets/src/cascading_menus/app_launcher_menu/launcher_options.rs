use crate::group_indicators::{
    sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
    toggle_control::SwitchPosition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAppListing {
    Hidden,
    Listed,
}

impl OpenAppListing {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            OpenAppListing::Hidden => OpenAppListing::Listed,
            OpenAppListing::Listed => OpenAppListing::Hidden,
        }
    }
}

impl SingletonFlagState for OpenAppListing {
    const STORED_FLAG: SingletonFlag = SingletonFlag::FrequentAppsHideOpen;
    const STORED_AS_SET: Self = OpenAppListing::Hidden;
    const STORED_AS_CLEARED: Self = OpenAppListing::Listed;
}

impl From<OpenAppListing> for SwitchPosition {
    fn from(open_app_listing: OpenAppListing) -> Self {
        match open_app_listing {
            OpenAppListing::Hidden => SwitchPosition::On,
            OpenAppListing::Listed => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryAppOrder {
    ByUsage,
    Alphabetical,
}

impl CategoryAppOrder {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            CategoryAppOrder::ByUsage => CategoryAppOrder::Alphabetical,
            CategoryAppOrder::Alphabetical => CategoryAppOrder::ByUsage,
        }
    }
}

impl SingletonFlagState for CategoryAppOrder {
    const STORED_FLAG: SingletonFlag = SingletonFlag::CategoryAppsSortedByUsage;
    const STORED_AS_SET: Self = CategoryAppOrder::ByUsage;
    const STORED_AS_CLEARED: Self = CategoryAppOrder::Alphabetical;
}

impl From<CategoryAppOrder> for SwitchPosition {
    fn from(category_app_order: CategoryAppOrder) -> Self {
        match category_app_order {
            CategoryAppOrder::ByUsage => SwitchPosition::On,
            CategoryAppOrder::Alphabetical => SwitchPosition::Off,
        }
    }
}
