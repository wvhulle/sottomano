use uom::si::f64::Information;

use super::{
    AssignedCpu, Chiplet, ChipletPlacement, CoreRole, LastLevelCache, LogicalCpu, SharedCache,
    highest_peak_clock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheSizeRank {
    Largest,
    Smaller,
}

pub(super) fn roles_by_cache_domain(cpus: &[LogicalCpu]) -> Vec<AssignedCpu<'_>> {
    let Some(domains) = CacheDomain::group_every_cpu(cpus) else {
        return cpus
            .iter()
            .map(|cpu| AssignedCpu {
                role: CoreRole::AllCores,
                cpu,
                chiplet: ChipletPlacement::OffChiplet,
            })
            .collect();
    };
    let [first, rest @ ..] = domains.as_slice() else {
        return Vec::new();
    };
    if rest.is_empty() {
        return first.assigned(CoreRole::AllCores, 0);
    }
    let largest_size = rest
        .iter()
        .map(|domain| domain.cache.size)
        .fold(first.cache.size, Information::max);
    let layout = if domains
        .iter()
        .all(|domain| domain.cache.size == largest_size)
    {
        CacheDomainLayout::EqualComplexes
    } else {
        let peak_clock_by_cache_rank = |cache_rank: CacheSizeRank| {
            highest_peak_clock(
                domains
                    .iter()
                    .filter(|domain| {
                        let domain_rank = if domain.cache.size == largest_size {
                            CacheSizeRank::Largest
                        } else {
                            CacheSizeRank::Smaller
                        };
                        domain_rank == cache_rank
                    })
                    .flat_map(|domain| domain.members.iter().copied()),
            )
        };
        if peak_clock_by_cache_rank(CacheSizeRank::Largest).is_some_and(|largest| {
            peak_clock_by_cache_rank(CacheSizeRank::Smaller)
                .is_some_and(|smaller| largest > smaller)
        }) {
            CacheDomainLayout::FullCoresBesideCompact { largest_size }
        } else {
            CacheDomainLayout::StackedCacheBesideHighClock { largest_size }
        }
    };
    domains
        .iter()
        .enumerate()
        .flat_map(|(position, domain)| domain.assigned(layout.role(position, domain), position))
        .collect()
}

struct CacheDomain<'cpus> {
    cache: &'cpus SharedCache,
    members: Vec<&'cpus LogicalCpu>,
}

impl<'cpus> CacheDomain<'cpus> {
    fn group_every_cpu(cpus: &'cpus [LogicalCpu]) -> Option<Vec<Self>> {
        cpus.iter()
            .try_fold(Vec::<Self>::new(), |mut domains, cpu| {
                let LastLevelCache::Shared(cache) = &cpu.last_level_cache else {
                    return None;
                };
                match domains.iter_mut().find(|domain| domain.cache == cache) {
                    Some(domain) => domain.members.push(cpu),
                    None => domains.push(CacheDomain {
                        cache,
                        members: vec![cpu],
                    }),
                }
                Some(domains)
            })
    }

    fn assigned(&self, role: CoreRole, position: usize) -> Vec<AssignedCpu<'cpus>> {
        let chiplet = Chiplet {
            position,
            shared_cache: self.cache.size,
        };
        self.members
            .iter()
            .map(|cpu| AssignedCpu {
                role,
                cpu,
                chiplet: ChipletPlacement::OnChiplet(chiplet),
            })
            .collect()
    }
}

enum CacheDomainLayout {
    EqualComplexes,
    StackedCacheBesideHighClock { largest_size: Information },
    FullCoresBesideCompact { largest_size: Information },
}

impl CacheDomainLayout {
    fn role(&self, position: usize, domain: &CacheDomain) -> CoreRole {
        match self {
            CacheDomainLayout::EqualComplexes => CoreRole::CoreComplex {
                ordinal: position + 1,
            },
            CacheDomainLayout::StackedCacheBesideHighClock { largest_size } => {
                if domain.cache.size == *largest_size {
                    CoreRole::StackedCacheCores
                } else {
                    CoreRole::HighClockCores
                }
            }
            CacheDomainLayout::FullCoresBesideCompact { largest_size } => {
                if domain.cache.size == *largest_size {
                    CoreRole::PerformanceCores
                } else {
                    CoreRole::CompactCores
                }
            }
        }
    }
}
