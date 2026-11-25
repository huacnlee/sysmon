//! Rates from the selected volume's cumulative disk counters. A device change,
//! missing counters or reset needs a fresh baseline, never a fabricated spike.
pub fn rates(
    previous: Option<&(String, u64, u64)>,
    current: Option<&(String, u64, u64)>,
    elapsed: f64,
) -> Option<(u64, u64)> {
    let (previous, current) = (previous?, current?);
    if previous.0 != current.0 || !elapsed.is_finite() || elapsed <= 0. {
        return None;
    }
    Some((
        (current.1.checked_sub(previous.1)? as f64 / elapsed) as u64,
        (current.2.checked_sub(previous.2)? as f64 / elapsed) as u64,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_missing_baseline_and_disk_changes_never_spike() {
        let first = ("disk0".into(), 1000, 4000);
        let second = ("disk0".into(), 5000, 6000);
        assert_eq!(rates(Some(&first), Some(&second), 2.), Some((2000, 1000)));
        assert_eq!(rates(Some(&second), Some(&second), 2.), Some((0, 0)));
        assert_eq!(rates(None, Some(&second), 2.), None);
        assert_eq!(rates(Some(&second), Some(&first), 2.), None);
        assert_eq!(
            rates(Some(&first), Some(&("disk1".into(), 5000, 6000)), 2.),
            None
        );
        assert_eq!(rates(Some(&first), Some(&second), 0.), None);
    }
}
