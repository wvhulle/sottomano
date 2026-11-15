#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyListings<Listing> {
    pub listings: Vec<Listing>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyDetails<Detail> {
    pub details: Vec<Detail>,
}
