//! Pure arithmetic of `juice_lock`, no Anchor types: the unlock time of a
//! tier and the fee bounds. Every timestamp sum is checked; `None` means the
//! caller returns `MathOverflow` (or the value is refused).

/// Forever: `unlock_ts` is this and `unlock` refuses the lock.
pub const FOREVER_TS: i64 = i64::MAX;

/// The unlock time of a lock (or an extension) made at `now` with `tier`:
/// `now + durations[tier]`, `FOREVER_TS` for the Forever tier, `None` for a
/// tier out of range or an overflowing sum.
pub fn unlock_ts(now: i64, tier: u8, durations: &[i64], forever_tier: u8) -> Option<i64> {
    if tier == forever_tier {
        return Some(FOREVER_TS);
    }
    let d = *durations.get(tier as usize)?;
    now.checked_add(d)
}

/// A fee is 0 (a promotion) or within `[rent_min, max]`, so the transfer to
/// an empty treasury can't fail on rent (boost finding 8).
pub fn fee_ok(fee: u64, rent_min: u64, max: u64) -> bool {
    fee <= max && (fee == 0 || fee >= rent_min)
}

/// What a vault received: its balance after minus before.
pub fn received(before: u64, after: u64) -> Option<u64> {
    after.checked_sub(before)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: [i64; 5] = [604_800, 2_592_000, 7_776_000, 15_552_000, 31_536_000];

    #[test]
    fn unlock_times() {
        for (t, d) in D.iter().enumerate() {
            assert_eq!(unlock_ts(1_000, t as u8, &D, 5), Some(1_000 + d));
        }
        assert_eq!(unlock_ts(1_000, 5, &D, 5), Some(i64::MAX));
        assert_eq!(unlock_ts(i64::MAX - 1, 5, &D, 5), Some(i64::MAX));
        assert_eq!(unlock_ts(1_000, 6, &D, 5), None);
        assert_eq!(unlock_ts(1_000, 255, &D, 5), None);
        assert_eq!(unlock_ts(i64::MAX - D[0] + 1, 0, &D, 5), None);
        assert_eq!(unlock_ts(i64::MAX - D[0], 0, &D, 5), Some(i64::MAX));
    }

    #[test]
    fn fee_bounds() {
        assert!(fee_ok(0, 890_880, 50_000_000));
        assert!(!fee_ok(1, 890_880, 50_000_000));
        assert!(!fee_ok(890_879, 890_880, 50_000_000));
        assert!(fee_ok(890_880, 890_880, 50_000_000));
        assert!(fee_ok(50_000_000, 890_880, 50_000_000));
        assert!(!fee_ok(50_000_001, 890_880, 50_000_000));
        assert!(!fee_ok(u64::MAX, 890_880, 50_000_000));
    }

    #[test]
    fn received_is_checked() {
        assert_eq!(received(5, 9), Some(4));
        assert_eq!(received(9, 5), None);
    }
}
