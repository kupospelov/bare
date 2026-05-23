use crate::render::Range;

pub fn arrays<T: PartialEq>(old: &[T], new: &[T], height: i32) -> Option<Range> {
    let min = old.len().min(new.len());
    let max = old.len().max(new.len());
    let mut dirty: Option<Range> = if min != max {
        Some(Range::new(min as i32 * height, max as i32 * height))
    } else {
        None
    };

    for i in 0..min {
        if old[i] != new[i] {
            let range = Range::new(i as i32 * height, (i as i32 + 1) * height);
            dirty = Some(match dirty {
                Some(d) => d.union(range),
                None => range,
            });
        }
    }

    dirty
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i32 = 20;

    fn assert_none(actual: Option<Range>) {
        assert!(
            actual.is_none(),
            "expected None, got {:?}",
            actual.map(|r| (r.start, r.end))
        );
    }

    fn assert_range(actual: Option<Range>, start: i32, end: i32) {
        let r = actual.expect("expected Some(Range), got None");
        assert_eq!((r.start, r.end), (start, end));
    }

    #[test]
    fn empty_to_empty() {
        assert_none(arrays::<i32>(&[], &[], H));
    }

    #[test]
    fn no_changes() {
        let old = vec![1, 2, 3];
        let new = vec![1, 2, 3];
        assert_none(arrays(&old, &new, H));
    }

    #[test]
    fn grow_from_empty() {
        let old: Vec<i32> = vec![];
        let new = vec![1, 2, 3];
        assert_range(arrays(&old, &new, H), 0, 3 * H);
    }

    #[test]
    fn shrink_to_one() {
        let old = vec![1, 2, 3];
        let new = vec![1];
        assert_range(arrays(&old, &new, H), H, 3 * H);
    }

    #[test]
    fn middle_item_changed() {
        let old = vec![1, 2, 3];
        let new = vec![1, 9, 3];
        assert_range(arrays(&old, &new, H), H, 2 * H);
    }

    #[test]
    fn grow_plus_prefix_change() {
        let old = vec![1, 2];
        let new = vec![9, 2, 3];
        assert_range(arrays(&old, &new, H), 0, 3 * H);
    }
}
