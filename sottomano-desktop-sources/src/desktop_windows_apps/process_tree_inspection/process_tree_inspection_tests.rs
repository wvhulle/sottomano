use uom::si::ratio::percent;

use super::*;

fn leaf(pid: i32, parent: i32) -> Process {
    Process {
        pid,
        parent,
        name: format!("proc{pid}"),
        memory: ByteSize::b(0),
        cpu: Ratio::new::<ratio>(0.0),
        threads: 1,
        uptime: Duration::ZERO,
        children: Vec::new(),
    }
}

fn grown(pid: i32, leaves: impl IntoIterator<Item = Process>) -> Process {
    let mut brood = by_parent(leaves);
    let root = brood
        .values()
        .flatten()
        .find(|node| node.pid == pid)
        .cloned()
        .unwrap_or_else(|| leaf(pid, 0));
    graft(root, &mut brood)
}

fn shape(node: &Process) -> Vec<i32> {
    let mut pids = vec![node.pid];
    pids.extend(node.children.iter().flat_map(shape));
    pids
}

#[test]
fn chain_nests_by_parent() {
    let root = grown(1, [leaf(1, 0), leaf(2, 1), leaf(3, 2), leaf(4, 1)]);
    assert_eq!(shape(&root), vec![1, 2, 3, 4]);
    assert_eq!(root.count(), 4);
}

#[test]
fn pruning_keeps_only_branches_holding_a_kept_process() {
    let kitty = grown(
        1,
        [
            leaf(1, 0),
            leaf(2, 1),
            leaf(3, 2),
            leaf(4, 1),
            leaf(5, 4),
            leaf(6, 1),
        ],
    );
    assert_eq!(shape(&kitty.pruned_to(&[2, 5])), vec![1, 2, 3, 4, 5]);
    assert_eq!(shape(&kitty.pruned_to(&[])), vec![1]);
}

#[test]
fn pruning_reaches_through_an_intermediate_fork_server() {
    let firefox = grown(
        1,
        [
            leaf(1, 0),
            leaf(2, 1),
            leaf(3, 2),
            leaf(4, 2),
            leaf(5, 4),
            leaf(6, 1),
        ],
    );
    assert_eq!(shape(&firefox.pruned_to(&[4])), vec![1, 2, 4, 5]);
}

#[test]
fn absent_parent_still_yields_its_child() {
    assert_eq!(shape(&grown(7, [leaf(7, 5), leaf(8, 7)])), vec![7, 8]);
}

#[test]
fn one_saturated_core_is_a_whole_share() {
    assert_eq!(
        CpuCapacity::one_core().busy_fraction(50, Duration::from_millis(500)),
        Ratio::new::<ratio>(1.0)
    );
}

#[test]
fn four_saturated_cores_exceed_a_whole_share() {
    assert_eq!(
        CpuCapacity::one_core().busy_fraction(200, Duration::from_millis(500)),
        Ratio::new::<percent>(400.0)
    );
}

#[test]
fn no_elapsed_time_is_idle() {
    assert_eq!(
        CpuCapacity::one_core().busy_fraction(50, Duration::ZERO),
        Ratio::new::<ratio>(0.0)
    );
}

#[test]
fn cpu_total_sums_descendants() {
    let mut root = grown(1, [leaf(1, 0), leaf(2, 1), leaf(3, 1)]);
    root.children[0].cpu = Ratio::new::<percent>(40.0);
    root.children[1].cpu = Ratio::new::<percent>(60.0);
    assert_eq!(root.cpu_total(), Ratio::new::<ratio>(1.0));
}

#[test]
fn self_parented_process_does_not_recurse() {
    assert_eq!(shape(&grown(1, [leaf(1, 1), leaf(2, 1)])), vec![1, 2]);
}
