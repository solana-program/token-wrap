//! Program state

use {
    bytemuck::{Pod, Zeroable},
    solana_pubkey::Pubkey,
};

/// Backpointer
///
/// Since the backpointer account address is derived from the wrapped mint, it
/// allows clients to easily work with wrapped tokens.
///
/// Try to fetch the account at `get_wrapped_mint_backpointer_address`.
///  * if it doesn't exist, then the token is not wrapped
///  * if it exists, read the data in the account as the unwrapped mint address
///
/// With this info, clients can easily unwrap tokens, even if they don't know
/// the origin.
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct Backpointer {
    /// Address that the wrapped mint is wrapping
    pub unwrapped_mint: Pubkey,
}

/// An on-chain pointer to a canonical token-wrap program deployment.
///
/// The authority of an unwrapped mint can create this account to signal which
/// deployment of the token-wrap program is the "official" one for their mint.
/// This guides users and apps especially when custom forks of the
/// program exist.
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct CanonicalDeploymentPointer {
    /// The program ID of the canonical token-wrap deployment as determined by
    /// the unwrapped mint authority.
    pub program_id: Pubkey,
}
