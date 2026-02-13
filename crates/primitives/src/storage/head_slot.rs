use reth::rpc::types::beacon::events::HeadEvent;

/// The head slot of the latest block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadSlot(u64);

impl HeadSlot {
    /// Ensures the proposal slot is the next slot.
    pub fn is_next_slot(&self, proposal_slot: u64) -> bool {
        self.0 + 1 == proposal_slot
    }

    /// Returns true if the event is the first slot of a new epoch.
    /// This is used to refresh the proposer duties for the next epoch for slots: 1, 33, 65, 97, ...
    pub fn is_first_slot_of_new_epoch(&self, slots_per_epoch: u64) -> bool {
        self.0 % slots_per_epoch == 1
    }
    /// Returns the epoch number of the event from the slot number.
    pub fn epoch(&self, slots_per_epoch: u64) -> u64 {
        self.0 / slots_per_epoch
    }
}

impl From<HeadEvent> for HeadSlot {
    fn from(event: HeadEvent) -> Self {
        Self(event.slot)
    }
}

impl From<u64> for HeadSlot {
    fn from(slot: u64) -> Self {
        Self(slot)
    }
}

impl AsRef<u64> for HeadSlot {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_first_slot_of_new_epoch_true() {
        let slots_per_epoch = 32;
        let slot: HeadSlot = HeadSlot::from(33);
        assert!(slot.is_first_slot_of_new_epoch(slots_per_epoch));
    }

    #[test]
    fn test_is_first_slot_of_new_epoch_false() {
        let slots_per_epoch = 32;
        let slot: HeadSlot = HeadSlot::from(34); // Not the first slot of an epoch
        assert!(!slot.is_first_slot_of_new_epoch(slots_per_epoch));
    }

    #[test]
    fn test_epoch_calculation() {
        let slots_per_epoch = 32;
        let tests = vec![
            (32, 1), // Last slot of the first epoch
            (33, 1), // First slot of the second epoch
            (64, 2), // Last slot of the second epoch
            (65, 2), // First slot of the third epoch
        ];
        for (slot, expected_epoch) in tests {
            let slot = HeadSlot::from(slot);
            assert_eq!(slot.epoch(slots_per_epoch), expected_epoch);
        }
    }
}
