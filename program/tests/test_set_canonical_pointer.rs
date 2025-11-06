use {
    crate::helpers::{
        common::{KeyedAccount, TokenProgram},
        mint_builder::MintBuilder,
        set_canonical_pointer_builder::SetCanonicalPointerBuilder,
    },
    bytemuck,
    mollusk_svm::result::Check,
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_token_wrap::{get_canonical_pointer_address, state::CanonicalDeploymentPointer},
};

pub mod helpers;

#[test]
fn test_fail_missing_authority_signature() {
    SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(Pubkey::new_unique())
        .authority_not_signer()
        .check(Check::err(ProgramError::MissingRequiredSignature))
        .execute();
}

#[test]
fn test_fail_mint_has_no_authority() {
    let mint_without_authority = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .no_mint_authority()
        .build();

    SetCanonicalPointerBuilder::new()
        .unwrapped_mint(mint_without_authority)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_fail_incorrect_authority() {
    let correct_authority = Pubkey::new_unique();
    let incorrect_authority = Pubkey::new_unique();
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_authority(correct_authority)
        .build();

    SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(incorrect_authority)
        .unwrapped_mint(mint)
        .check(Check::err(ProgramError::IncorrectAuthority))
        .execute();
}

#[test]
fn test_fail_incorrect_pointer_address() {
    let authority = Pubkey::new_unique();
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_authority(authority)
        .build();
    let incorrect_pointer = KeyedAccount {
        key: Pubkey::new_unique(), // Not the derived PDA
        account: Account::default(),
    };
    SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(authority)
        .unwrapped_mint(mint)
        .canonical_pointer(incorrect_pointer)
        .check(Check::err(ProgramError::InvalidArgument))
        .execute();
}

#[test]
fn test_fail_insufficient_funds_for_new_pointer() {
    let authority = Pubkey::new_unique();
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_authority(authority)
        .build();

    let pointer_address = get_canonical_pointer_address(&mint.key);
    let pointer_account_not_rent_exempt = KeyedAccount {
        key: pointer_address,
        account: Account {
            lamports: Rent::default()
                .minimum_balance(std::mem::size_of::<CanonicalDeploymentPointer>())
                - 1,
            ..Default::default()
        },
    };

    SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(authority)
        .unwrapped_mint(mint)
        .canonical_pointer(pointer_account_not_rent_exempt)
        .check(Check::err(ProgramError::AccountNotRentExempt))
        .execute();
}

#[test]
fn test_success_create_new_pointer() {
    let authority = Pubkey::new_unique();
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_authority(authority)
        .build();
    let new_program_id = Pubkey::new_unique();
    let pointer_address = get_canonical_pointer_address(&mint.key);
    let pointer_account_uninitialized = KeyedAccount {
        key: pointer_address,
        account: Account {
            lamports: Rent::default()
                .minimum_balance(std::mem::size_of::<CanonicalDeploymentPointer>()),
            ..Default::default()
        },
    };

    let result = SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(authority)
        .unwrapped_mint(mint)
        .canonical_pointer(pointer_account_uninitialized)
        .new_program_id(new_program_id)
        .execute();

    // Check account state
    assert_eq!(result.canonical_pointer.account.owner, spl_token_wrap::id());
    let pointer_data =
        bytemuck::from_bytes::<CanonicalDeploymentPointer>(&result.canonical_pointer.account.data);
    assert_eq!(pointer_data.program_id, new_program_id);
}

#[test]
fn test_success_update_existing_pointer() {
    let authority = Pubkey::new_unique();
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_authority(authority)
        .build();
    let old_program_id = Pubkey::new_unique();
    let new_program_id = Pubkey::new_unique();

    let pointer_address = get_canonical_pointer_address(&mint.key);
    let pointer_account_initialized = KeyedAccount {
        key: pointer_address,
        account: Account {
            lamports: Rent::default()
                .minimum_balance(std::mem::size_of::<CanonicalDeploymentPointer>()),
            owner: spl_token_wrap::id(),
            data: bytemuck::bytes_of(&CanonicalDeploymentPointer {
                program_id: old_program_id,
            })
            .to_vec(),
            ..Default::default()
        },
    };

    let result = SetCanonicalPointerBuilder::new()
        .unwrapped_mint_authority(authority)
        .unwrapped_mint(mint)
        .canonical_pointer(pointer_account_initialized)
        .new_program_id(new_program_id)
        .execute();

    // Check account state
    assert_eq!(result.canonical_pointer.account.owner, spl_token_wrap::id());
    let pointer_data =
        bytemuck::from_bytes::<CanonicalDeploymentPointer>(&result.canonical_pointer.account.data);
    assert_eq!(pointer_data.program_id, new_program_id);
}
