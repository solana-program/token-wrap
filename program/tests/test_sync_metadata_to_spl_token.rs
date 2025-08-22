use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
        sync_to_spl_token_builder::SyncToSplTokenBuilder,
    },
    borsh::BorshSerialize,
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    mpl_token_metadata::{
        accounts::Metadata as MetaplexMetadata,
        types::{Collection, CollectionDetails, Creator, Key, TokenStandard, UseMethod, Uses},
        utils::clean,
    },
    solana_account::Account,
    solana_instruction::AccountMeta,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_wrap::{error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority},
};

pub mod helpers;

#[test]
fn test_fail_wrapped_mint_not_spl_token() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022) // Invalid program
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token::id(),
        ))
        .build();

    SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .check(Check::err(TokenWrapError::NoSyncingToToken2022.into()))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_pda_mismatch() {
    let wrong_wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_key(Pubkey::new_unique()) // Not the derived PDA
        .build();

    SyncToSplTokenBuilder::new()
        .wrapped_mint(wrong_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_authority_pda_mismatch() {
    SyncToSplTokenBuilder::new()
        .wrapped_mint_authority(Pubkey::new_unique()) // Not the derived PDA
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}

#[test]
fn test_fail_metaplex_pda_mismatch() {
    let wrong_metaplex_pda = KeyedAccount {
        key: Pubkey::new_unique(), // Not the derived PDA
        account: Account::default(),
    };

    SyncToSplTokenBuilder::new()
        .metaplex_metadata(wrong_metaplex_pda)
        .check(Check::err(TokenWrapError::MetaplexMetadataMismatch.into()))
        .execute();
}

#[test]
fn test_fail_incorrect_metaplex_program_id() {
    let mollusk = init_mollusk();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .build();
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint.key);
    let metaplex_metadata_account = KeyedAccount {
        key: metaplex_pda,
        account: Account::default(),
    };

    // The incorrect program account
    let fake_program = KeyedAccount {
        key: Pubkey::new_unique(),
        account: create_program_account_loader_v3(&Pubkey::new_unique()),
    };

    let mut instruction = spl_token_wrap::instruction::sync_metadata_to_spl_token(
        &spl_token_wrap::id(),
        &metaplex_metadata_account.key,
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        None,
        None,
    );

    // Swap out the correct program ID with the fake one
    instruction.accounts[4] = AccountMeta::new_readonly(fake_program.key, false);

    let accounts = &[
        metaplex_metadata_account.pair(),
        wrapped_mint.pair(),
        (
            wrapped_mint_authority,
            Account {
                lamports: 10_000_000_000,
                ..Default::default()
            },
        ),
        unwrapped_mint.pair(),
        fake_program.pair(),
        mollusk_svm::program::keyed_account_for_system_program(),
        mollusk.sysvars.keyed_account_for_rent_sysvar(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
}

#[test]
fn test_fail_owner_program_mismatch() {
    let mollusk = init_mollusk();

    let source_metadata = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            owner: mock_metadata_owner::ID, // The real owner
            ..Default::default()
        },
    };

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(source_metadata.key),
        })
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .build();
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint.key);
    let metaplex_metadata_account = KeyedAccount {
        key: metaplex_pda,
        account: Account::default(),
    };

    let wrong_owner_program = KeyedAccount {
        key: Pubkey::new_unique(), // An incorrect program ID
        account: create_program_account_loader_v3(&Pubkey::new_unique()),
    };

    let instruction = spl_token_wrap::instruction::sync_metadata_to_spl_token(
        &spl_token_wrap::id(),
        &metaplex_metadata_account.key,
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        Some(&source_metadata.key),
        Some(&wrong_owner_program.key),
    );

    let accounts = &[
        metaplex_metadata_account.pair(),
        wrapped_mint.pair(),
        (
            wrapped_mint_authority,
            Account {
                lamports: 10_000_000_000,
                ..Default::default()
            },
        ),
        unwrapped_mint.pair(),
        (
            mpl_token_metadata::ID,
            create_program_account_loader_v3(&mpl_token_metadata::ID),
        ),
        mollusk_svm::program::keyed_account_for_system_program(),
        mollusk.sysvars.keyed_account_for_rent_sysvar(),
        source_metadata.pair(),
        wrong_owner_program.pair(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::InvalidAccountOwner)],
    );
}

#[test]
fn test_fail_insufficient_payer_funds_for_cpi() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    // The destination metaplex metadata account is uninitialized
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint_address);
    let metaplex_keyed_account = KeyedAccount {
        key: metaplex_pda,
        account: Account::default(), // Belongs to system program
    };

    SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .metaplex_metadata(metaplex_keyed_account)
        .wrapped_mint_authority_lamports(100) // Not enough for rent
        .check(Check::err(ProgramError::Custom(1))) // Insufficient funds from system program CPI
        .execute();
}

#[test]
fn test_success_update_from_token2022_to_spl_token() {
    let unwrapped_mint_key = Pubkey::new_unique();
    let source_creators = vec![Creator {
        address: Pubkey::new_unique(),
        verified: true, // This will be set to false
        share: 100,
    }];
    let source_collection = Collection {
        verified: false,
        key: Pubkey::new_unique(),
    };
    let source_uses = Uses {
        use_method: UseMethod::Burn,
        remaining: 1,
        total: 1,
    };

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_key)
        .with_extension(MintExtension::TokenMetadata {
            name: "Unwrapped".to_string(),
            symbol: "UWT".to_string(),
            uri: "uri.unwrapped".to_string(),
            additional_metadata: vec![
                ("seller_fee_basis_points".to_string(), "150".to_string()),
                (
                    "creators".to_string(),
                    serde_json::to_string(&source_creators).unwrap(),
                ),
                (
                    "collection".to_string(),
                    serde_json::to_string(&source_collection).unwrap(),
                ),
                (
                    "uses".to_string(),
                    serde_json::to_string(&source_uses).unwrap(),
                ),
            ],
        })
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(unwrapped_mint_key),
        })
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint_address);
    let mut metaplex_metadata_account = Account {
        owner: mpl_token_metadata::ID,
        data: vec![0; 1024], // Allocate ample space for updates
        ..Default::default()
    };
    let initial_metaplex_metadata = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: wrapped_mint_authority,
        mint: wrapped_mint_address,
        name: "Old Name".to_string(),
        symbol: "OLD".to_string(),
        uri: "old.uri".to_string(),
        seller_fee_basis_points: 0,
        creators: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: None,
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };
    initial_metaplex_metadata
        .serialize(&mut &mut metaplex_metadata_account.data[..])
        .unwrap();

    let metaplex_keyed_account = KeyedAccount {
        key: metaplex_pda,
        account: metaplex_metadata_account,
    };

    let result = SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .metaplex_metadata(metaplex_keyed_account)
        .execute();

    let final_metaplex_metadata =
        MetaplexMetadata::from_bytes(&result.metaplex_metadata.account.data).unwrap();

    // Assertions
    assert_eq!(clean(final_metaplex_metadata.name), "Unwrapped");
    assert_eq!(clean(final_metaplex_metadata.symbol), "UWT");
    assert_eq!(clean(final_metaplex_metadata.uri), "uri.unwrapped");
    assert_eq!(final_metaplex_metadata.seller_fee_basis_points, 150);

    // Creators should be copied but unverified
    let mut expected_creators = source_creators;
    expected_creators[0].verified = false;
    assert_eq!(final_metaplex_metadata.creators, Some(expected_creators));

    assert_eq!(final_metaplex_metadata.collection, Some(source_collection));
    assert_eq!(final_metaplex_metadata.uses, Some(source_uses));

    // Ensure update authority is the PDA
    assert_eq!(
        final_metaplex_metadata.update_authority,
        wrapped_mint_authority
    );
    assert!(final_metaplex_metadata.is_mutable);
}

#[test]
fn test_success_update_from_spl_token_to_spl_token() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let (source_metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let mut source_metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        ..Default::default()
    };
    // Rich source metadata
    let source_metaplex_metadata = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Source".to_string(),
        symbol: "SRC".to_string(),
        uri: "uri.source".to_string(),
        seller_fee_basis_points: 500,
        creators: Some(vec![Creator {
            address: Pubkey::new_unique(),
            verified: true, // Will become unverified
            share: 100,
        }]),
        primary_sale_happened: true,
        is_mutable: true,
        edition_nonce: Some(0),
        token_standard: Some(TokenStandard::Fungible),
        collection: Some(Collection {
            verified: false,
            key: Pubkey::new_unique(),
        }),
        uses: Some(Uses {
            use_method: UseMethod::Single,
            remaining: 1,
            total: 1,
        }),
        collection_details: Some(CollectionDetails::V1 { size: 100 }),
        programmable_config: None,
    };
    source_metaplex_account.data = source_metaplex_metadata.try_to_vec().unwrap();

    let source_keyed_account = KeyedAccount {
        key: source_metaplex_pda,
        account: source_metaplex_account,
    };

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let (dest_metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint_address);
    let mut dest_metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        data: vec![0; 1024], // Allocate ample space for updates
        ..Default::default()
    };
    // Empty initial destination metadata
    let dest_metaplex_metadata = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: wrapped_mint_authority,
        mint: wrapped_mint_address,
        name: String::new(),
        symbol: String::new(),
        uri: String::new(),
        seller_fee_basis_points: 0,
        creators: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: None,
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };
    dest_metaplex_metadata
        .serialize(&mut &mut dest_metaplex_account.data[..])
        .unwrap();

    let dest_keyed_account = KeyedAccount {
        key: dest_metaplex_pda,
        account: dest_metaplex_account,
    };

    let result = SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_keyed_account)
        .metaplex_metadata(dest_keyed_account)
        .execute();

    let final_metaplex_metadata =
        MetaplexMetadata::from_bytes(&result.metaplex_metadata.account.data).unwrap();

    // Assertions
    assert_eq!(clean(final_metaplex_metadata.name), "Source");
    assert_eq!(clean(final_metaplex_metadata.symbol), "SRC");
    assert_eq!(clean(final_metaplex_metadata.uri), "uri.source");
    assert_eq!(final_metaplex_metadata.seller_fee_basis_points, 500);

    // We expect the creator to be unverified after the sync
    let mut expected_creators = source_metaplex_metadata.creators.clone();
    if let Some(creators) = &mut expected_creators {
        for creator in creators.iter_mut() {
            creator.verified = false;
        }
    }
    assert_eq!(final_metaplex_metadata.creators, expected_creators);
    assert_eq!(final_metaplex_metadata.uses, source_metaplex_metadata.uses);
    assert!(!final_metaplex_metadata.primary_sale_happened);
    assert_eq!(
        final_metaplex_metadata.is_mutable,
        source_metaplex_metadata.is_mutable
    );
    assert_eq!(final_metaplex_metadata.edition_nonce, None);
    assert_eq!(final_metaplex_metadata.token_standard, None);
    assert_eq!(
        final_metaplex_metadata.collection,
        source_metaplex_metadata.collection
    );
    assert_eq!(final_metaplex_metadata.collection_details, None);
    assert_eq!(
        final_metaplex_metadata.update_authority,
        wrapped_mint_authority
    );
}

#[test]
fn test_success_initialize_from_token2022_to_spl_token() {
    let unwrapped_mint_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_key)
        .with_extension(MintExtension::TokenMetadata {
            name: "Unwrapped".to_string(),
            symbol: "UWT".to_string(),
            uri: "uri.unwrapped".to_string(),
            additional_metadata: vec![],
        })
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(unwrapped_mint_key),
        })
        .build();

    let result = SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .execute();

    let final_metaplex_metadata =
        MetaplexMetadata::from_bytes(&result.metaplex_metadata.account.data).unwrap();

    assert_eq!(clean(final_metaplex_metadata.name), "Unwrapped");
    assert_eq!(clean(final_metaplex_metadata.symbol), "UWT");
    assert_eq!(clean(final_metaplex_metadata.uri), "uri.unwrapped");
}

#[test]
fn test_success_initialize_from_spl_token_to_spl_token() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let metaplex_metadata_obj = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Test Token".to_string(),
        symbol: "TEST".to_string(),
        uri: "uri".to_string(),
        seller_fee_basis_points: 0,
        creators: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: None,
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };

    let metaplex_metadata = KeyedAccount {
        key: MetaplexMetadata::find_pda(&unwrapped_mint.key).0,
        account: Account {
            lamports: 1_000_000_000,
            data: metaplex_metadata_obj.try_to_vec().unwrap(),
            owner: mpl_token_metadata::ID,
            ..Default::default()
        },
    };

    // The destination metaplex metadata account is uninitialized
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint_address);
    let metaplex_keyed_account = KeyedAccount {
        key: metaplex_pda,
        account: Account::default(),
    };

    let result = SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(metaplex_metadata)
        .metaplex_metadata(metaplex_keyed_account)
        .execute();

    let final_metaplex_metadata =
        MetaplexMetadata::from_bytes(&result.metaplex_metadata.account.data).unwrap();

    assert_eq!(clean(final_metaplex_metadata.name), "Test Token");
    assert_eq!(clean(final_metaplex_metadata.symbol), "TEST");
    assert_eq!(clean(final_metaplex_metadata.uri), "uri");
    assert_eq!(
        final_metaplex_metadata.update_authority,
        result.wrapped_mint_authority.key
    );
    assert!(final_metaplex_metadata.is_mutable);
    assert!(!final_metaplex_metadata.primary_sale_happened);
    // Calculate the expected edition_nonce (bump seed) for this specific test run.
    let (_, expected_bump) = Pubkey::find_program_address(
        &[
            "metadata".as_bytes(),
            mpl_token_metadata::ID.as_ref(),
            result.wrapped_mint.key.as_ref(),
            "edition".as_bytes(),
        ],
        &mpl_token_metadata::ID,
    );
    assert_eq!(final_metaplex_metadata.edition_nonce, Some(expected_bump));
    assert_eq!(
        final_metaplex_metadata.token_standard,
        Some(TokenStandard::Fungible)
    );
    assert_eq!(final_metaplex_metadata.collection, None);
    assert_eq!(final_metaplex_metadata.collection_details, None);
}

#[test]
fn test_success_nulls_fields_on_update() {
    // This test verifies that when updating an existing Metaplex metadata
    // account from a sparse source, the fields that are absent in the source
    // are correctly nulled out in the destination.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let (source_metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let mut source_metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        lamports: 1_000_000_000,
        ..Default::default()
    };
    let source_metaplex_metadata = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Sparse Source".to_string(),
        symbol: "SPARSE".to_string(),
        uri: "uri.sparse".to_string(),
        seller_fee_basis_points: 100,
        creators: None,
        collection: None,
        uses: None,
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: None,
        token_standard: None,
        collection_details: None,
        programmable_config: None,
    };
    source_metaplex_account.data = source_metaplex_metadata.try_to_vec().unwrap();

    let source_keyed_account = KeyedAccount {
        key: source_metaplex_pda,
        account: source_metaplex_account,
    };

    // This metadata has optional fields populated, which should be cleared.
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let (dest_metaplex_pda, _) = MetaplexMetadata::find_pda(&wrapped_mint_address);
    let mut dest_metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        data: vec![0; 1024],
        lamports: 1_000_000_000,
        ..Default::default()
    };
    let initial_dest_metaplex_metadata = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: wrapped_mint_authority,
        mint: wrapped_mint_address,
        name: "Old Rich Name".to_string(),
        symbol: "RICH".to_string(),
        uri: "uri.rich".to_string(),
        seller_fee_basis_points: 999,
        creators: Some(vec![Creator {
            address: Pubkey::new_unique(),
            verified: false,
            share: 100,
        }]),
        collection: Some(Collection {
            verified: false,
            key: Pubkey::new_unique(),
        }),
        uses: Some(Uses {
            use_method: UseMethod::Burn,
            remaining: 5,
            total: 5,
        }),
        primary_sale_happened: true,
        is_mutable: true,
        edition_nonce: Some(1),
        token_standard: None,
        collection_details: None,
        programmable_config: None,
    };
    initial_dest_metaplex_metadata
        .serialize(&mut &mut dest_metaplex_account.data[..])
        .unwrap();

    let dest_keyed_account = KeyedAccount {
        key: dest_metaplex_pda,
        account: dest_metaplex_account,
    };

    let result = SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_keyed_account)
        .metaplex_metadata(dest_keyed_account)
        .execute();

    let final_metaplex_metadata =
        MetaplexMetadata::from_bytes(&result.metaplex_metadata.account.data).unwrap();

    // Assert basic fields were updated from the source.
    assert_eq!(clean(final_metaplex_metadata.name), "Sparse Source");
    assert_eq!(clean(final_metaplex_metadata.symbol), "SPARSE");
    assert_eq!(clean(final_metaplex_metadata.uri), "uri.sparse");
    assert_eq!(final_metaplex_metadata.seller_fee_basis_points, 100);

    // Assert that fields present in the destination but not the source were nulled.
    assert_eq!(final_metaplex_metadata.creators, None);
    assert_eq!(final_metaplex_metadata.collection, None);
    assert_eq!(final_metaplex_metadata.uses, None);

    // Assert not changed
    assert!(final_metaplex_metadata.primary_sale_happened);
    assert!(final_metaplex_metadata.is_mutable);
    assert_eq!(
        final_metaplex_metadata.update_authority,
        wrapped_mint_authority
    );
}
