use crate::drive::RootTree;
use crate::fee_pools::epochs::tree_key_constants;
use crate::fee_pools::epochs::EpochPool;

impl EpochPool {
    pub fn get_proposers_path(&self) -> [&[u8]; 3] {
        [
            Into::<&[u8; 1]>::into(RootTree::Pools),
            &self.key,
            tree_key_constants::KEY_PROPOSERS.as_slice(),
        ]
    }

    pub fn get_proposers_vec_path(&self) -> Vec<Vec<u8>> {
        vec![
            vec![RootTree::Pools as u8],
            self.key.to_vec(),
            tree_key_constants::KEY_PROPOSERS.to_vec(),
        ]
    }

    pub fn get_path(&self) -> [&[u8]; 2] {
        [Into::<&[u8; 1]>::into(RootTree::Pools), &self.key]
    }

    pub fn get_vec_path(&self) -> Vec<Vec<u8>> {
        vec![vec![RootTree::Pools as u8], self.key.to_vec()]
    }
}