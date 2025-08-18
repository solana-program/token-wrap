use solana_pubkey::Pubkey;

pub mod entrypoint;
pub mod processor;

pub const ID: Pubkey = Pubkey::new_from_array([3u8; 32]); // success case
pub const NO_RETURN: Pubkey = Pubkey::new_from_array([4u8; 32]);
