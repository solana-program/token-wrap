//! Token Wrap program
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod entrypoint;
pub mod error;
pub mod instruction;
pub mod metadata;
pub mod metaplex;
pub mod mint_customizer;
pub mod processor;
pub mod state;

use {
    solana_pubkey::Pubkey,
    spl_associated_token_account_client::address::get_associated_token_address_with_program_id,
};

solana_pubkey::declare_id!("TwRapQCDhWkZRrDaHfZGuHxkZ91gHDRkyuzNqeU5MgR");

const WRAPPED_MINT_SEED: &[u8] = br"mint";

pub(crate) fn get_wrapped_mint_address_with_seed(
    unwrapped_mint: &Pubkey,
    wrapped_token_program_id: &Pubkey,
) -> (Pubkey, u8) {
    get_wrapped_mint_address_with_seed_for_program(unwrapped_mint, wrapped_token_program_id, &id())
}

pub(crate) fn get_wrapped_mint_address_with_seed_for_program(
    unwrapped_mint: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &get_wrapped_mint_seeds(unwrapped_mint, wrapped_token_program_id),
        program_id,
    )
}

pub(crate) fn get_wrapped_mint_seeds<'a>(
    unwrapped_mint: &'a Pubkey,
    wrapped_token_program_id: &'a Pubkey,
) -> [&'a [u8]; 3] {
    [
        WRAPPED_MINT_SEED,
        unwrapped_mint.as_ref(),
        wrapped_token_program_id.as_ref(),
    ]
}

pub(crate) fn get_wrapped_mint_signer_seeds<'a>(
    unwrapped_mint: &'a Pubkey,
    wrapped_token_program_id: &'a Pubkey,
    bump_seed: &'a [u8],
) -> [&'a [u8]; 4] {
    [
        WRAPPED_MINT_SEED,
        unwrapped_mint.as_ref(),
        wrapped_token_program_id.as_ref(),
        bump_seed,
    ]
}

/// Derive the SPL Token wrapped mint address associated with an unwrapped mint
pub fn get_wrapped_mint_address(
    unwrapped_mint: &Pubkey,
    wrapped_token_program_id: &Pubkey,
) -> Pubkey {
    get_wrapped_mint_address_for_program(unwrapped_mint, wrapped_token_program_id, &id())
}

/// Derive the SPL Token wrapped mint address associated with an unwrapped mint
/// for a specific Token Wrap program deployment.
pub fn get_wrapped_mint_address_for_program(
    unwrapped_mint: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    get_wrapped_mint_address_with_seed_for_program(
        unwrapped_mint,
        wrapped_token_program_id,
        program_id,
    )
    .0
}

const WRAPPED_MINT_AUTHORITY_SEED: &[u8] = br"authority";

pub(crate) fn get_wrapped_mint_authority_seeds(wrapped_mint: &Pubkey) -> [&[u8]; 2] {
    [WRAPPED_MINT_AUTHORITY_SEED, wrapped_mint.as_ref()]
}

pub(crate) fn get_wrapped_mint_authority_signer_seeds<'a>(
    wrapped_mint: &'a Pubkey,
    bump_seed: &'a [u8],
) -> [&'a [u8]; 3] {
    [
        WRAPPED_MINT_AUTHORITY_SEED,
        wrapped_mint.as_ref(),
        bump_seed,
    ]
}

pub(crate) fn get_wrapped_mint_authority_with_seed(wrapped_mint: &Pubkey) -> (Pubkey, u8) {
    get_wrapped_mint_authority_with_seed_for_program(wrapped_mint, &id())
}

pub(crate) fn get_wrapped_mint_authority_with_seed_for_program(
    wrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&get_wrapped_mint_authority_seeds(wrapped_mint), program_id)
}

/// Derive the SPL Token wrapped mint authority address
pub fn get_wrapped_mint_authority(wrapped_mint: &Pubkey) -> Pubkey {
    get_wrapped_mint_authority_for_program(wrapped_mint, &id())
}

/// Derive the SPL Token wrapped mint authority address for a specific Token
/// Wrap program deployment
pub fn get_wrapped_mint_authority_for_program(
    wrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    get_wrapped_mint_authority_with_seed_for_program(wrapped_mint, program_id).0
}

const WRAPPED_MINT_BACKPOINTER_SEED: &[u8] = br"backpointer";

pub(crate) fn get_wrapped_mint_backpointer_address_seeds(wrapped_mint: &Pubkey) -> [&[u8]; 2] {
    [WRAPPED_MINT_BACKPOINTER_SEED, wrapped_mint.as_ref()]
}

pub(crate) fn get_wrapped_mint_backpointer_address_signer_seeds<'a>(
    wrapped_mint: &'a Pubkey,
    bump_seed: &'a [u8],
) -> [&'a [u8]; 3] {
    [
        WRAPPED_MINT_BACKPOINTER_SEED,
        wrapped_mint.as_ref(),
        bump_seed,
    ]
}

pub(crate) fn get_wrapped_mint_backpointer_address_with_seed(
    wrapped_mint: &Pubkey,
) -> (Pubkey, u8) {
    get_wrapped_mint_backpointer_address_with_seed_for_program(wrapped_mint, &id())
}

pub(crate) fn get_wrapped_mint_backpointer_address_with_seed_for_program(
    wrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &get_wrapped_mint_backpointer_address_seeds(wrapped_mint),
        program_id,
    )
}

/// Derive the SPL Token wrapped mint backpointer address
pub fn get_wrapped_mint_backpointer_address(wrapped_mint: &Pubkey) -> Pubkey {
    get_wrapped_mint_backpointer_address_for_program(wrapped_mint, &id())
}

/// Derive the SPL Token wrapped mint backpointer address for a specific Token
/// Wrap program deployment.
pub fn get_wrapped_mint_backpointer_address_for_program(
    wrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    get_wrapped_mint_backpointer_address_with_seed_for_program(wrapped_mint, program_id).0
}

/// Derive the escrow `ATA` that backs a given wrapped mint.
pub fn get_escrow_address(
    unwrapped_mint: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    wrapped_token_program_id: &Pubkey,
) -> Pubkey {
    get_escrow_address_for_program(
        unwrapped_mint,
        unwrapped_token_program_id,
        wrapped_token_program_id,
        &id(),
    )
}

/// Derive the escrow `ATA` for a specific Token Wrap program deployment.
pub fn get_escrow_address_for_program(
    unwrapped_mint: &Pubkey,
    unwrapped_token_program_id: &Pubkey,
    wrapped_token_program_id: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    let wrapped_mint =
        get_wrapped_mint_address_for_program(unwrapped_mint, wrapped_token_program_id, program_id);
    let mint_authority = get_wrapped_mint_authority_for_program(&wrapped_mint, program_id);

    get_associated_token_address_with_program_id(
        &mint_authority,
        unwrapped_mint,
        unwrapped_token_program_id,
    )
}

const CANONICAL_POINTER_SEED: &[u8] = br"canonical_pointer";

/// Derives the canonical pointer address and bump seed for a specific
/// Token Wrap program deployment.
pub(crate) fn get_canonical_pointer_address_with_seed_for_program(
    unwrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CANONICAL_POINTER_SEED, unwrapped_mint.as_ref()],
        program_id,
    )
}

pub(crate) fn get_canonical_pointer_address_signer_seeds<'a>(
    unwrapped_mint: &'a Pubkey,
    bump_seed: &'a [u8],
) -> [&'a [u8]; 3] {
    [CANONICAL_POINTER_SEED, unwrapped_mint.as_ref(), bump_seed]
}

/// Derives the canonical pointer address and bump seed.
pub(crate) fn get_canonical_pointer_address_with_seed(unwrapped_mint: &Pubkey) -> (Pubkey, u8) {
    get_canonical_pointer_address_with_seed_for_program(unwrapped_mint, &id())
}

/// Derives the canonical pointer address for an unwrapped mint.
pub fn get_canonical_pointer_address(unwrapped_mint: &Pubkey) -> Pubkey {
    get_canonical_pointer_address_for_program(unwrapped_mint, &id())
}

/// Derives the canonical pointer address for an unwrapped mint for a specific
/// Token Wrap program deployment.
pub fn get_canonical_pointer_address_for_program(
    unwrapped_mint: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    get_canonical_pointer_address_with_seed_for_program(unwrapped_mint, program_id).0
}
