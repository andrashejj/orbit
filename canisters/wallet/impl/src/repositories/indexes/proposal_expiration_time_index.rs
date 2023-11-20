use crate::{
    core::{with_memory_manager, Memory, PROPOSAL_EXPIRATION_TIME_INDEX_MEMORY_ID},
    models::indexes::proposal_expiration_time_index::{
        ProposalExpirationTimeIndex, ProposalExpirationTimeIndexCriteria,
    },
};
use ic_canister_core::{repository::IndexRepository, types::UUID};
use ic_stable_structures::{memory_manager::VirtualMemory, StableBTreeMap};
use std::{cell::RefCell, collections::HashSet};

thread_local! {
  static DB: RefCell<StableBTreeMap<ProposalExpirationTimeIndex, (), VirtualMemory<Memory>>> = with_memory_manager(|memory_manager| {
    RefCell::new(
      StableBTreeMap::init(memory_manager.get(PROPOSAL_EXPIRATION_TIME_INDEX_MEMORY_ID))
    )
  })
}

#[derive(Default, Debug)]
pub struct ProposalExpirationTimeIndexRepository {}

impl IndexRepository<ProposalExpirationTimeIndex, UUID> for ProposalExpirationTimeIndexRepository {
    type FindByCriteria = ProposalExpirationTimeIndexCriteria;

    fn exists(&self, index: &ProposalExpirationTimeIndex) -> bool {
        DB.with(|m| m.borrow().get(index).is_some())
    }

    fn insert(&self, index: ProposalExpirationTimeIndex) {
        DB.with(|m| m.borrow_mut().insert(index, ()));
    }

    fn remove(&self, index: &ProposalExpirationTimeIndex) -> bool {
        DB.with(|m| m.borrow_mut().remove(index).is_some())
    }

    fn find_by_criteria(&self, criteria: Self::FindByCriteria) -> HashSet<UUID> {
        DB.with(|db| {
            let start_key = ProposalExpirationTimeIndex {
                expiration_dt: criteria.from_dt.to_owned().unwrap_or(u64::MIN),
                proposal_id: [std::u8::MIN; 16],
            };
            let end_key = ProposalExpirationTimeIndex {
                expiration_dt: criteria.to_dt.to_owned().unwrap_or(u64::MAX),
                proposal_id: [std::u8::MAX; 16],
            };

            db.borrow()
                .range(start_key..=end_key)
                .map(|(index, _)| index.proposal_id)
                .collect::<HashSet<UUID>>()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_crud() {
        let repository = ProposalExpirationTimeIndexRepository::default();
        let index = ProposalExpirationTimeIndex {
            expiration_dt: 10,
            proposal_id: [1; 16],
        };

        assert!(!repository.exists(&index));

        repository.insert(index.clone());

        assert!(repository.exists(&index));
        assert!(repository.remove(&index));
        assert!(!repository.exists(&index));
    }

    #[test]
    fn test_find_by_criteria() {
        let repository = ProposalExpirationTimeIndexRepository::default();
        let index = ProposalExpirationTimeIndex {
            expiration_dt: 10,
            proposal_id: [1; 16],
        };

        repository.insert(index.clone());
        repository.insert(ProposalExpirationTimeIndex {
            expiration_dt: 11,
            proposal_id: [2; 16],
        });

        let criteria = ProposalExpirationTimeIndexCriteria {
            from_dt: None,
            to_dt: Some(10),
        };

        let result = repository.find_by_criteria(criteria);

        assert_eq!(result.len(), 1);
        assert!(result.contains(&index.proposal_id));
    }
}