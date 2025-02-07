use {
    mollusk_svm::{result::Check, Mollusk},
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_pod::{
        optional_keys::OptionalNonZeroPubkey,
        primitives::{PodBool, PodU64},
    },
    spl_token_2022::{
        extension::{
            mint_close_authority::MintCloseAuthority, BaseStateWithExtensionsMut, ExtensionType,
            PodStateWithExtensions, PodStateWithExtensionsMut,
        },
        pod::{PodCOption, PodMint},
        state::Mint,
    },
    spl_token_wrap::{
        get_wrapped_mint_address, get_wrapped_mint_authority, get_wrapped_mint_backpointer_address,
        instruction::create_mint, state::Backpointer,
    },
    std::convert::TryFrom,
};

const MINT_DECIMALS: u8 = 12;
const MINT_SUPPLY: u64 = 500_000_000;
const FREEZE_AUTHORITY: Pubkey =
    Pubkey::from_str_const("11111115q4EpJaTXAZWpCg3J2zppWGSZ46KXozzo9");

fn setup_mint(owner: Pubkey, rent: &Rent) -> Account {
    let state = spl_token::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        freeze_authority: COption::Some(FREEZE_AUTHORITY),
        ..Default::default()
    };
    let mut data = vec![0u8; spl_token::state::Mint::LEN];
    state.pack_into_slice(&mut data);

    let lamports = rent.minimum_balance(data.len());

    Account {
        lamports,
        data,
        owner,
        ..Default::default()
    }
}

#[test]
fn test_idempotency_false_with_existing_account() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    // Simulating existing data on mint or backpointer
    let account_with_data = Account {
        data: vec![1; 10],
        ..Account::default()
    };

    // idempotent: false causes these to fail
    let accounts = &[
        (wrapped_mint_address, account_with_data.clone()), // mint already exists
        (wrapped_backpointer_address, Account::default()),
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::AccountAlreadyInitialized)],
    );

    let accounts = &[
        (wrapped_mint_address, Account::default()),
        (wrapped_backpointer_address, account_with_data), // backpointer already exists
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::AccountAlreadyInitialized)],
    );
}

#[test]
fn test_idempotency_true_with_existing_valid_account() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        true,
    );

    // Simulating existing data on mint or backpointer
    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: wrapped_token_program_id,
        ..Account::default()
    };
    let backpointer_account_with_data = Account {
        owner: program_id,
        ..Account::default()
    };

    // idempotent: true causes these to return successfully
    let accounts = &[
        (wrapped_mint_address, mint_account_with_data.clone()), // mint already exists
        (wrapped_backpointer_address, backpointer_account_with_data), // backpointer already exists
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(&instruction, accounts, &[Check::success()]);
}

#[test]
fn test_idempotency_true_with_existing_invalid_accounts() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    // Incorrectly wrapped mint account owner

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        true,
    );

    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: Pubkey::new_unique(), // Wrong owner
        ..Account::default()
    };
    let backpointer_account_with_data = Account {
        owner: program_id,
        ..Account::default()
    };

    let accounts = &[
        (wrapped_mint_address, mint_account_with_data.clone()),
        (wrapped_backpointer_address, backpointer_account_with_data),
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidAccountOwner)],
    );

    // Incorrect owner on wrapped backpointer account

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        true,
    );

    let mint_account_with_data = Account {
        data: vec![1; 10],
        owner: wrapped_token_program_id,
        ..Account::default()
    };
    let backpointer_account_with_data = Account {
        owner: Pubkey::new_unique(), // Wrong owner
        ..Account::default()
    };

    let accounts = &[
        (wrapped_mint_address, mint_account_with_data.clone()),
        (wrapped_backpointer_address, backpointer_account_with_data),
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidAccountOwner)],
    );
}

#[test]
fn test_create_mint_insufficient_funds() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

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

    let accounts = &[
        (wrapped_mint_address, wrapped_mint_account_insufficent_funds),
        (wrapped_backpointer_address, Account::default()),
        (unwrapped_mint_address, Account::default()),
        (solana_system_interface::program::id(), Account::default()),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::AccountNotRentExempt)],
    );
}

#[test]
fn test_create_mint_backpointer_insufficient_funds() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

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

    let rent = &mollusk.sysvars.rent;
    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            wrapped_backpointer_account_insufficent_funds,
        ),
        (unwrapped_mint_address, setup_mint(spl_token::id(), rent)),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InsufficientFunds)],
    );
}

#[test]
fn test_improperly_derived_addresses_fail() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let rent = &mollusk.sysvars.rent;
    let unwrapped_mint_account = setup_mint(spl_token::id(), rent);

    // Incorrectly derived wrapped mint address

    let incorrect_wrapped_mint_addr = Pubkey::new_unique();
    let instruction = create_mint(
        &program_id,
        &incorrect_wrapped_mint_addr,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let accounts = &[
        (
            incorrect_wrapped_mint_addr,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidAccountData)],
    );

    // Incorrectly derived backpointer address

    let incorrect_backpointer = Pubkey::new_unique();
    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &incorrect_backpointer,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            incorrect_backpointer,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidSeeds)],
    );

    // Incorrect token program address passed

    let incorrect_token_program = Pubkey::new_unique();
    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &incorrect_token_program,
        false,
    );

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        (incorrect_token_program, Account::default()),
    ];
    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidAccountData)],
    );
}

#[test]
fn test_successful_spl_token_to_token_2022() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token_2022::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let rent = &mollusk.sysvars.rent;
    let unwrapped_mint_account = setup_mint(spl_token::id(), rent);

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token2022::keyed_account(),
    ];
    let result = mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[
            Check::success(),
            // Ensure unwrapped_mint_account remains unchanged
            Check::account(&unwrapped_mint_address)
                .data(&unwrapped_mint_account.data)
                .build(),
        ],
    );

    // Assert state of resulting wrapped mint account

    let resulting_wrapped_mint_account = &result.resulting_accounts[0].1;
    assert_eq!(resulting_wrapped_mint_account.owner, spl_token_2022::id());

    let wrapped_mint_data = Mint::unpack(&resulting_wrapped_mint_account.data).unwrap();
    assert_eq!(wrapped_mint_data.decimals, MINT_DECIMALS);
    let expected_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    assert_eq!(
        wrapped_mint_data.mint_authority.unwrap(),
        expected_mint_authority,
    );
    assert_eq!(wrapped_mint_data.supply, 0);
    assert!(wrapped_mint_data.is_initialized);
    assert_eq!(
        wrapped_mint_data.freeze_authority.unwrap(),
        FREEZE_AUTHORITY
    );

    // Assert state of resulting backpointer account

    let resulting_backpointer_account = &result.resulting_accounts[1].1;
    assert_eq!(resulting_backpointer_account.owner, program_id);

    let backpointer = bytemuck::from_bytes::<Backpointer>(&resulting_backpointer_account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint_address);
}

#[test]
fn test_successful_token_2022_to_spl_token() {
    let program_id = spl_token_wrap::id();
    let mut mollusk = Mollusk::new(&program_id, "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);

    let unwrapped_mint_address = Pubkey::new_unique();
    let wrapped_token_program_id = spl_token::id();
    let wrapped_mint_address =
        get_wrapped_mint_address(&unwrapped_mint_address, &wrapped_token_program_id);
    let wrapped_backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    let instruction = create_mint(
        &program_id,
        &wrapped_mint_address,
        &wrapped_backpointer_address,
        &unwrapped_mint_address,
        &wrapped_token_program_id,
        false,
    );

    let rent = &mollusk.sysvars.rent;

    // Add extension to spl_token_2022

    let mint_size =
        ExtensionType::try_calculate_account_len::<PodMint>(&[ExtensionType::MintCloseAuthority])
            .unwrap();
    let mut buffer = vec![0; mint_size];
    let mut state =
        PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();
    state.base.decimals = MINT_DECIMALS;
    state.base.is_initialized = PodBool::from_bool(true);
    state.base.supply = PodU64::from(MINT_SUPPLY);
    state.base.freeze_authority = PodCOption::from(COption::Some(FREEZE_AUTHORITY));
    state.init_account_type().unwrap();

    let extension = state.init_extension::<MintCloseAuthority>(true).unwrap();
    let close_authority =
        OptionalNonZeroPubkey::try_from(Some(Pubkey::new_from_array([1; 32]))).unwrap();
    extension.close_authority = close_authority;

    let mut unwrapped_mint_account = setup_mint(spl_token_2022::id(), rent);
    unwrapped_mint_account.data = buffer;

    let accounts = &[
        (
            wrapped_mint_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (
            wrapped_backpointer_address,
            Account {
                lamports: 100_000_000,
                ..Default::default()
            },
        ),
        (unwrapped_mint_address, unwrapped_mint_account.clone()),
        (
            solana_system_interface::program::id(),
            Account {
                executable: true,
                ..Default::default()
            },
        ),
        mollusk_svm_programs_token::token::keyed_account(),
    ];
    let result = mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[
            Check::success(),
            // Ensure unwrapped_mint_account remains unchanged
            Check::account(&unwrapped_mint_address)
                .data(&unwrapped_mint_account.data)
                .build(),
        ],
    );

    // Assert state of resulting wrapped mint account

    let resulting_wrapped_mint_account = &result.resulting_accounts[0].1;
    assert_eq!(resulting_wrapped_mint_account.owner, spl_token::id());

    let wrapped_mint_data =
        PodStateWithExtensions::<PodMint>::unpack(&resulting_wrapped_mint_account.data)
            .unwrap()
            .base;

    assert_eq!(wrapped_mint_data.decimals, MINT_DECIMALS);
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
        wrapped_mint_data
            .freeze_authority
            .ok_or(ProgramError::InvalidAccountData)
            .unwrap(),
        FREEZE_AUTHORITY
    );

    // Assert state of resulting backpointer account

    let resulting_backpointer_account = &result.resulting_accounts[1].1;
    assert_eq!(resulting_backpointer_account.owner, program_id);

    let backpointer = bytemuck::from_bytes::<Backpointer>(&resulting_backpointer_account.data[..]);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint_address);
}
