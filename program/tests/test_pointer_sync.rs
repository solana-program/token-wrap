use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::{MintExtension, MintExtension::MetadataPointer},
        metadata::assert_metaplex_fields_synced,
        mint_builder::MintBuilder,
        sync_metadata_builder::SyncMetadataBuilder,
        token_account_builder::TokenAccountBuilder,
    },
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    mpl_token_metadata::{
        accounts::Metadata as MetaplexMetadata,
        types::{CollectionDetails, Creator, Key, TokenStandard, UseMethod, Uses},
    },
    solana_account::Account,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{BaseStateWithExtensions, PodStateWithExtensions, PodStateWithExtensionsMut},
        pod::PodMint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::{
        error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority, id,
        instruction::sync_metadata_to_token_2022,
    },
};

pub mod helpers;

#[test]
fn test_pointer_missing_fails() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No metadata extension

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(TokenWrapError::MetadataPointerMissing.into()))
        .execute();
}

#[test]
fn test_pointer_unset_fails() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: None,
        })
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(TokenWrapError::MetadataPointerUnset.into()))
        .execute();
}

#[test]
fn test_token_2022_self_pointer_success() {
    let mint_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(mint_key)
        .with_extension(MetadataPointer {
            metadata_address: Some(mint_key),
        })
        .with_extension(MintExtension::TokenMetadata {
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/test.json".to_string(),
            additional_metadata: vec![],
        })
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .execute();
}

#[test]
fn test_pointer_present_but_no_account_fails() {
    let external_address = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(external_address),
        })
        .build();

    // Don't provide the source_metadata account to the instruction
    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(ProgramError::NotEnoughAccountKeys))
        .execute();
}

#[test]
fn test_pointer_mismatch_fails() {
    let pointer_address = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(pointer_address),
        })
        .build();

    let wrong_metadata_account = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account::default(),
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(wrong_metadata_account)
        .check(Check::err(TokenWrapError::MetadataPointerMismatch.into()))
        .execute();
}

#[test]
fn test_fail_pointer_to_token_2022_account_metadata_unsupported() {
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build();
    let source_metadata_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(mint)
        .build();

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(source_metadata_account.key),
        })
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        // Source is not allowed to be a token account
        .source_metadata(source_metadata_account)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_fail_pointer_to_token_2022_mint() {
    let metadata_source_mint_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(metadata_source_mint_key),
        })
        .build();

    let source_metadata_extension = MintExtension::TokenMetadata {
        name: "Mock Token".to_string(),
        symbol: "MOCK".to_string(),
        uri: "https://example.com/mock.json".to_string(),
        additional_metadata: vec![],
    };

    let source_metadata_account = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(metadata_source_mint_key)
        .with_extension(source_metadata_extension.clone())
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata_account)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_pointer_to_metaplex_success() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build();

    let metaplex_metadata_obj = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Metaplex Token".to_string(),
        symbol: "MPL".to_string(),
        uri: "https://metaplex.dev/meta.json".to_string(),
        seller_fee_basis_points: 250,
        creators: Some(vec![Creator {
            address: Pubkey::new_unique(),
            verified: true,
            share: 100,
        }]),
        primary_sale_happened: true,
        is_mutable: true,
        edition_nonce: Some(1),
        token_standard: Some(TokenStandard::NonFungible),
        collection: Some(mpl_token_metadata::types::Collection {
            key: Pubkey::new_unique(),
            verified: false,
        }),
        uses: Some(Uses {
            use_method: UseMethod::Burn,
            remaining: 0,
            total: 0,
        }),
        collection_details: Some(CollectionDetails::V1 { size: 1 }),
        programmable_config: None,
    };

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let metaplex_account = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            lamports: 1_000_000_000,
            data: borsh::to_vec(&metaplex_metadata_obj).unwrap(),
            owner: mpl_token_metadata::ID,
            executable: false,
            rent_epoch: 0,
        },
    };

    // Point the Token-2022 mint's metadata pointer at the Metaplex PDA
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(metaplex_account.key),
        })
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint.clone())
        .source_metadata(metaplex_account)
        .execute();

    let mut binding = result.wrapped_mint.account.data.clone();
    let wrapped_state = PodStateWithExtensionsMut::<PodMint>::unpack(&mut binding).unwrap();
    let wrapped_tm = wrapped_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    assert_eq!(wrapped_tm.name, "Metaplex Token");
    assert_eq!(wrapped_tm.symbol, "MPL");
    assert_eq!(wrapped_tm.uri, "https://metaplex.dev/meta.json");
    assert_eq!(
        Option::<Pubkey>::from(wrapped_tm.update_authority).unwrap(),
        result.wrapped_mint_authority.key
    );
    assert_eq!(wrapped_tm.mint, result.wrapped_mint.key);
    assert_metaplex_fields_synced(&wrapped_tm, &metaplex_metadata_obj);
}

#[test]
fn test_pointer_to_metaplex_with_invalid_data_fails() {
    let unwrapped_mint_key = Pubkey::new_unique();
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint_key);

    // Point the unwrapped mint's metadata pointer to the Metaplex PDA
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_key)
        .with_extension(MetadataPointer {
            metadata_address: Some(metaplex_pda),
        })
        .build();

    // Create the Metaplex account with invalid data that cannot be deserialized
    let metaplex_account_invalid_data = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            lamports: 1_000_000_000,
            data: vec![1, 2, 3], // Invalid data
            owner: mpl_token_metadata::ID,
            ..Default::default()
        },
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(metaplex_account_invalid_data)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_third_party_missing_owner_program_fails() {
    let mollusk = init_mollusk();

    let mock_metadata_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(mock_metadata_key),
        })
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .with_extension(MetadataPointer {
            metadata_address: Some(wrapped_mint_address),
        })
        .build();

    let mock_metadata_account = KeyedAccount {
        key: mock_metadata_key,
        account: Account::default(),
    };

    let ix = sync_metadata_to_token_2022(
        &id(),
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        Some(&mock_metadata_key),
        None, // Missing owner program
    );

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        TokenProgram::SplToken2022.keyed_account(),
        mock_metadata_account.pair(),
    ];
    mollusk.process_and_validate_instruction(
        &ix,
        accounts,
        &[Check::err(ProgramError::NotEnoughAccountKeys)],
    );
}

#[test]
fn test_pointer_to_third_party_with_wrong_owner_program_fails() {
    let mollusk = init_mollusk();

    let mock_metadata_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(mock_metadata_key),
        })
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .with_extension(MetadataPointer {
            metadata_address: Some(wrapped_mint_address),
        })
        .build();

    // The metadata account is owned by the mock program
    let mock_metadata_account = KeyedAccount {
        key: mock_metadata_key,
        account: Account {
            owner: mock_metadata_owner::ID,
            ..Default::default()
        },
    };

    // But we provide a *different* program as the owner in the instruction
    let wrong_owner_program_key = Pubkey::new_unique();

    let ix = sync_metadata_to_token_2022(
        &id(),
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        Some(&mock_metadata_key),
        Some(&wrong_owner_program_key),
    );

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        TokenProgram::SplToken2022.keyed_account(),
        mock_metadata_account.pair(),
        (
            wrong_owner_program_key,
            create_program_account_loader_v3(&wrong_owner_program_key),
        ),
    ];

    mollusk.process_and_validate_instruction(
        &ix,
        accounts,
        &[Check::err(ProgramError::InvalidAccountOwner)],
    );
}

#[test]
fn test_pointer_to_third_party_success() {
    let mollusk = init_mollusk();

    let source_metadata_extension = MintExtension::TokenMetadata {
        name: "Mock External Token".to_string(),
        symbol: "MOCK".to_string(),
        uri: "https://example.com/mock.json".to_string(),
        additional_metadata: vec![("external_key".to_string(), "external_value".to_string())],
    };

    let mock_metadata_key = Pubkey::new_unique();

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(mock_metadata_key),
        })
        .build();

    let mut mock_metadata_account = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(mock_metadata_key)
        .with_extension(source_metadata_extension.clone())
        .build();
    mock_metadata_account.account.owner = mock_metadata_owner::ID;

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .with_extension(MetadataPointer {
            metadata_address: Some(wrapped_mint_address),
        })
        .with_extension(MintExtension::TokenMetadata {
            name: "".to_string(),
            symbol: "".to_string(),
            uri: "".to_string(),
            additional_metadata: vec![],
        })
        .build();

    let ix = sync_metadata_to_token_2022(
        &id(),
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        Some(&mock_metadata_key),
        Some(&mock_metadata_owner::ID),
    );

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        TokenProgram::SplToken2022.keyed_account(),
        mock_metadata_account.pair(),
        (
            mock_metadata_owner::ID,
            create_program_account_loader_v3(&mock_metadata_owner::ID),
        ),
    ];

    let result = mollusk.process_and_validate_instruction(&ix, accounts, &[Check::success()]);

    let final_wrapped_mint_account = result.get_account(&wrapped_mint.key).unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&final_wrapped_mint_account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = source_metadata_extension
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            wrapped_mint_authority
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("Unexpected extension type");
    }
}

#[test]
fn test_pointer_to_third_party_no_return_fails() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MetadataPointer {
            metadata_address: Some(mock_metadata_owner::NO_RETURN),
        })
        .build();

    let source_metadata = KeyedAccount {
        key: mock_metadata_owner::NO_RETURN,
        account: Account {
            owner: mock_metadata_owner::ID,
            ..Default::default()
        },
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata)
        .check(Check::err(
            TokenWrapError::ExternalProgramReturnedNoData.into(),
        ))
        .execute();
}
