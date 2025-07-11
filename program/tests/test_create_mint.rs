use {
    crate::helpers::{
        common::{TokenProgram, DEFAULT_MINT_DECIMALS},
        create_mint_builder::CreateMintBuilder,
        extensions::MintExtension,
        mint_builder::MintBuilder,
    },
    mollusk_svm::result::Check,
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_pod::primitives::{PodBool, PodU64},
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint, BaseStateWithExtensions,
            PodStateWithExtensions,
        },
        pod::{PodCOption, PodMint},
        state::Mint,
    },
    spl_token_wrap::{
        error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority,
        state::Backpointer,
    },
    test_case::test_case,
};

pub mod helpers;

#[test]
fn test_idempotency_false_with_existing_account() {
    let account_with_data = Account {
        data: vec![1; 10],
        ..Account::default()
    };

    // Test case 1: mint already exists
    CreateMintBuilder::default()
        .wrapped_mint_account(account_with_data.clone())
        .check(Check::err(ProgramError::AccountAlreadyInitialized))
        .execute();

    // Test case 2: backpointer already exists
    CreateMintBuilder::default()
        .backpointer_account(account_with_data)
        .check(Check::err(ProgramError::AccountAlreadyInitialized))
        .execute();
}

#[test]
fn test_idempotency_true_with_existing_valid_account() {
    // Simulating existing data on mint or backpointer
    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: spl_token_2022::id(),
        ..Account::default()
    };
    let backpointer_account_with_data = Account {
        owner: spl_token_wrap::id(),
        ..Account::default()
    };

    // idempotent: true causes these to return successfully
    CreateMintBuilder::default()
        .idempotent()
        .wrapped_mint_account(mint_account_with_data)
        .backpointer_account(backpointer_account_with_data)
        .execute();
}

#[test]
fn test_idempotency_true_with_existing_invalid_accounts() {
    // Incorrectly wrapped mint account owner

    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: Pubkey::new_unique(), // Wrong owner
        ..Account::default()
    };

    CreateMintBuilder::default()
        .idempotent()
        .wrapped_mint_account(mint_account_with_data)
        .check(Check::err(TokenWrapError::InvalidWrappedMintOwner.into()))
        .execute();

    // Incorrect owner on wrapped backpointer account

    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: spl_token_2022::id(),
        ..Account::default()
    };

    let backpointer_account_with_data = Account {
        owner: Pubkey::new_unique(), // Wrong owner
        data: vec![1; 10],
        ..Account::default()
    };

    CreateMintBuilder::default()
        .idempotent()
        .wrapped_mint_account(mint_account_with_data)
        .backpointer_account(backpointer_account_with_data)
        .check(Check::err(TokenWrapError::InvalidBackpointerOwner.into()))
        .execute();
}

#[test]
fn test_create_mint_insufficient_funds() {
    // Calculate minimum rent for Mint account
    let rent = Rent::default(); // Using default rent for test
    let space = Mint::get_packed_len();
    let mint_rent_required = rent.minimum_balance(space);

    // Create wrapped_mint_account with insufficient lamports
    let insufficient_lamports = mint_rent_required - 1; // Less than required rent
    let wrapped_mint_account_insufficent_funds = Account {
        lamports: insufficient_lamports,
        ..Account::default()
    };

    CreateMintBuilder::default()
        .wrapped_mint_account(wrapped_mint_account_insufficent_funds)
        .check(Check::err(ProgramError::AccountNotRentExempt))
        .execute();
}

#[test]
fn test_create_mint_backpointer_insufficient_funds() {
    // Calculate minimum rent for Backpointer account
    let rent = Rent::default(); // Using default rent for test
    let backpointer_space = std::mem::size_of::<Backpointer>();
    let backpointer_rent_required = rent.minimum_balance(backpointer_space);

    // Create wrapped_backpointer_account with insufficient lamports
    let insufficient_lamports = backpointer_rent_required - 1; // Less than required rent
    let wrapped_backpointer_account_insufficent_funds = Account {
        lamports: insufficient_lamports,
        ..Account::default()
    };

    CreateMintBuilder::default()
        .backpointer_account(wrapped_backpointer_account_insufficent_funds)
        .check(Check::err(ProgramError::AccountNotRentExempt))
        .execute();
}

#[test]
fn test_improperly_derived_addresses_fail() {
    // Incorrectly derived wrapped mint address

    let incorrect_wrapped_mint_addr = Pubkey::new_unique();
    CreateMintBuilder::default()
        .wrapped_mint_addr(incorrect_wrapped_mint_addr)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();

    // Incorrectly derived backpointer address

    let incorrect_backpointer = Pubkey::new_unique();
    CreateMintBuilder::default()
        .backpointer_addr(incorrect_backpointer)
        .check(Check::err(TokenWrapError::BackpointerMismatch.into()))
        .execute();

    // Incorrect token program address passed

    let incorrect_token_program = Pubkey::new_unique();
    CreateMintBuilder::default()
        .token_program_addr(incorrect_token_program)
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_invalid_unwrapped_mint_owner() {
    // Create fake unwrapped‑mint account owned by an arbitrary program.
    let bogus_mint = Account {
        lamports: 100_000_000,
        owner: Pubkey::new_unique(), // ← not spl‑token or spl‑token‑2022
        data: vec![0; spl_token::state::Mint::LEN],
        ..Account::default()
    };

    CreateMintBuilder::default()
        .unwrapped_mint_account(bogus_mint)
        .check(Check::err(ProgramError::InvalidAccountOwner))
        .execute();
}

#[test]
fn test_successful_spl_token_to_spl_token_2022() {
    let freeze_authority = Pubkey::new_unique();
    let result = CreateMintBuilder::default()
        .unwrapped_token_program(TokenProgram::SplToken)
        .wrapped_token_program(TokenProgram::SplToken2022)
        .freeze_authority(freeze_authority)
        .execute();

    // Assert state of resulting wrapped mint account

    assert_eq!(result.wrapped_mint.account.owner, spl_token_2022::id());
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();

    assert_eq!(wrapped_mint_state.base.decimals, DEFAULT_MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&result.wrapped_mint.key);
    assert_eq!(
        wrapped_mint_state.base.mint_authority,
        PodCOption::some(expected_mint_authority),
    );
    assert_eq!(wrapped_mint_state.base.supply, PodU64::from(0));
    assert_eq!(
        wrapped_mint_state.base.is_initialized,
        PodBool::from_bool(true)
    );
    assert_eq!(
        wrapped_mint_state.base.freeze_authority,
        PodCOption::some(freeze_authority)
    );

    // Verify confidential transfer extension is present and is the only extension
    assert!(wrapped_mint_state
        .get_extension::<ConfidentialTransferMint>()
        .is_ok());
    assert_eq!(wrapped_mint_state.get_extension_types().unwrap().len(), 1);

    // Assert state of resulting backpointer account

    assert_eq!(
        result.wrapped_backpointer.account.owner,
        spl_token_wrap::id()
    );
    let backpointer =
        bytemuck::from_bytes::<Backpointer>(&result.wrapped_backpointer.account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, result.unwrapped_mint.key);
}

#[test]
fn test_successful_spl_token_2022_to_spl_token() {
    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let freeze_authority = Pubkey::new_unique();

    let result = CreateMintBuilder::default()
        .unwrapped_mint_addr(unwrapped_mint_address)
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_mint_addr(wrapped_mint_address)
        .wrapped_token_program(TokenProgram::SplToken)
        .freeze_authority(freeze_authority)
        .execute();

    // Assert state of resulting wrapped mint account

    assert_eq!(result.wrapped_mint.account.owner, spl_token::id());

    let wrapped_mint_data =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data)
            .unwrap()
            .base;

    assert_eq!(wrapped_mint_data.decimals, DEFAULT_MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    assert_eq!(
        wrapped_mint_data
            .mint_authority
            .ok_or(ProgramError::InvalidAccountData)
            .unwrap(),
        expected_mint_authority,
    );
    assert_eq!(wrapped_mint_data.supply, PodU64::from(0));
    assert_eq!(wrapped_mint_data.is_initialized, PodBool::from_bool(true));
    assert_eq!(
        wrapped_mint_data.freeze_authority,
        PodCOption::some(freeze_authority)
    );

    // Assert state of resulting backpointer account

    assert_eq!(
        result.wrapped_backpointer.account.owner,
        spl_token_wrap::id()
    );

    let backpointer =
        bytemuck::from_bytes::<Backpointer>(&result.wrapped_backpointer.account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint_address);
}

#[test_case(MintExtension::ConfidentialTransfer)]
#[test_case(MintExtension::TransferHook)]
#[test_case(MintExtension::TransferFeeConfig)]
#[test_case(MintExtension::MintCloseAuthority(Pubkey::new_unique()))]
fn test_create_mint_from_extended_mint(extension: MintExtension) {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(extension)
        .build();

    let result = CreateMintBuilder::default()
        .unwrapped_mint_account(unwrapped_mint.account)
        .unwrapped_mint_addr(unwrapped_mint.key)
        .unwrapped_token_program(TokenProgram::SplToken2022)
        .wrapped_token_program(TokenProgram::SplToken)
        .execute();

    assert_eq!(result.wrapped_mint.account.owner, spl_token::id());
    let wrapped_mint_data =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data)
            .unwrap()
            .base;

    assert_eq!(wrapped_mint_data.decimals, DEFAULT_MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&result.wrapped_mint.key);
    assert_eq!(
        wrapped_mint_data
            .mint_authority
            .ok_or(ProgramError::InvalidAccountData)
            .unwrap(),
        expected_mint_authority,
    );
    assert_eq!(wrapped_mint_data.supply, PodU64::from(0));
    assert_eq!(wrapped_mint_data.is_initialized, PodBool::from_bool(true));
    assert_eq!(wrapped_mint_data.freeze_authority, PodCOption::none());

    assert_eq!(
        result.wrapped_backpointer.account.owner,
        spl_token_wrap::id()
    );
    let backpointer =
        bytemuck::from_bytes::<Backpointer>(&result.wrapped_backpointer.account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint.key);
}
