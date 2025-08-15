use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::{MintExtension, MintExtension::MetadataPointer as MetadataPointerExt},
        mint_builder::MintBuilder,
        sync_metadata_builder::SyncMetadataBuilder,
    },
    borsh::BorshSerialize,
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    mpl_token_metadata::{
        accounts::Metadata as MetaplexMetadata,
        types::{Collection, CollectionDetails, Creator, Key, TokenStandard, UseMethod, Uses},
    },
    solana_account::Account,
    solana_instruction::AccountMeta,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{BaseStateWithExtensions, PodStateWithExtensions},
        pod::PodMint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::{
        error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority, id,
        instruction::sync_metadata_to_token_2022,
    },
    std::collections::HashMap,
};

pub mod helpers;

#[test]
fn test_fail_incorrect_token_program() {
    let mollusk = init_mollusk();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::TokenMetadata {
            name: "N".to_string(),
            symbol: "S".to_string(),
            uri: "U".to_string(),
            additional_metadata: vec![],
        })
        .build();
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .build();

    // Pass a fake program account instead of the real Token-2022 program
    let fake_program = KeyedAccount {
        key: Pubkey::new_unique(),
        account: create_program_account_loader_v3(&Pubkey::new_unique()),
    };

    let mut instruction = sync_metadata_to_token_2022(
        &id(),
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        None,
    );

    instruction.accounts[3] = AccountMeta::new_readonly(fake_program.key, false);

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        fake_program.pair(),
        TokenProgram::SplToken2022.keyed_account(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
}

#[test]
fn test_fail_wrapped_mint_not_token_2022() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::TokenMetadata {
            name: "N".to_string(),
            symbol: "S".to_string(),
            uri: "U".to_string(),
            additional_metadata: vec![],
        })
        .build();
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken) // Invalid program
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        ))
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_pda_mismatch() {
    let wrong_wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(Pubkey::new_unique()) // Not the derived PDA
        .build();

    SyncMetadataBuilder::new()
        .wrapped_mint(wrong_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_authority_pda_mismatch() {
    SyncMetadataBuilder::new()
        .wrapped_mint_authority(Pubkey::new_unique()) // Not the derived PDA
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}

#[test]
fn test_fail_unwrapped_mint_has_no_metadata() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No metadata extension

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(
            TokenWrapError::UnwrappedMintHasNoMetadata.into(),
        ))
        .execute();
}

#[test]
fn test_fail_spl_token_missing_metaplex_account() {
    let mollusk = init_mollusk();

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MetadataPointerExt)
        .lamports(1_000_000_000)
        .build();

    let instruction = sync_metadata_to_token_2022(
        &id(),
        &wrapped_mint.key,
        &wrapped_mint_authority,
        &unwrapped_mint.key,
        None, // Metaplex account is omitted
    );

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        TokenProgram::SplToken2022.keyed_account(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::NotEnoughAccountKeys)],
    );
}

#[test]
fn test_fail_sync_metadata_with_wrong_metaplex_owner() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);

    let malicious_metadata_account = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            lamports: 1_000_000_000,
            owner: Pubkey::new_unique(), // fake metadata account owner
            ..Default::default()
        },
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .metaplex_metadata(malicious_metadata_account)
        .check(Check::err(ProgramError::InvalidAccountOwner))
        .execute();
}

#[test]
fn test_fail_spl_token_with_invalid_metaplex_pda() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let invalid_metaplex_pda = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            owner: mpl_token_metadata::ID, // Correct owner
            ..Default::default()
        },
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .metaplex_metadata(invalid_metaplex_pda)
        .check(Check::err(TokenWrapError::MetaplexMetadataMismatch.into()))
        .execute();
}

#[test]
fn test_fail_spl_token_without_metaplex_metadata() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let missing_metaplex_account = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            owner: mpl_token_metadata::ID, // Correct owner, but no data
            ..Default::default()
        },
    };

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .metaplex_metadata(missing_metaplex_account)
        .check(Check::err(ProgramError::InvalidAccountData))
        .execute();
}

#[test]
fn test_success_initialize_from_token_2022() {
    let unwrapped_metadata = MintExtension::TokenMetadata {
        name: "Unwrapped Token".to_string(),
        symbol: "UWT".to_string(),
        uri: "https://unwrapped.dev/meta.json".to_string(),
        additional_metadata: vec![
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ],
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(unwrapped_metadata.clone())
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MetadataPointerExt)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = unwrapped_metadata
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
        panic!("destructure failed");
    }
}

#[test]
fn test_success_update_from_token_2022() {
    let old_metadata = MintExtension::TokenMetadata {
        name: "Old Name".to_string(),
        symbol: "SYM".to_string(),
        uri: "uri".to_string(),
        additional_metadata: vec![
            ("key1".to_string(), "old_value".to_string()),
            ("key2".to_string(), "value2".to_string()), // to be removed
        ],
    };

    let new_metadata = MintExtension::TokenMetadata {
        name: "Updated Name".to_string(), // updated
        symbol: "SYM".to_string(),        // same
        uri: "uri".to_string(),           // same
        additional_metadata: vec![
            ("key1".to_string(), "new_value".to_string()), // updated
            ("key3".to_string(), "value3".to_string()),    // new
        ],
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(new_metadata.clone())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        ))
        .with_extension(MetadataPointerExt)
        .with_extension(old_metadata)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = new_metadata
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            result.wrapped_mint_authority.key
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("destructure failed");
    }
}

fn assert_metaplex_fields_synced(
    wrapped_metadata: &TokenMetadata,
    metaplex_metadata: &MetaplexMetadata,
) {
    let additional_meta_map: HashMap<_, _> = wrapped_metadata
        .additional_metadata
        .iter()
        .cloned()
        .collect();

    assert_eq!(
        additional_meta_map.get("seller_fee_basis_points").unwrap(),
        &metaplex_metadata.seller_fee_basis_points.to_string()
    );
    assert_eq!(
        additional_meta_map.get("primary_sale_happened").unwrap(),
        &metaplex_metadata.primary_sale_happened.to_string()
    );
    assert_eq!(
        additional_meta_map.get("is_mutable").unwrap(),
        &metaplex_metadata.is_mutable.to_string()
    );
    if let Some(edition_nonce) = metaplex_metadata.edition_nonce {
        assert_eq!(
            additional_meta_map.get("edition_nonce").unwrap(),
            &edition_nonce.to_string()
        );
    }
    if let Some(token_standard) = &metaplex_metadata.token_standard {
        assert_eq!(
            additional_meta_map.get("token_standard").unwrap(),
            &serde_json::to_string(token_standard).unwrap()
        );
    }
    if let Some(collection) = &metaplex_metadata.collection {
        assert_eq!(
            additional_meta_map.get("collection").unwrap(),
            &serde_json::to_string(collection).unwrap()
        );
    }
    if let Some(uses) = &metaplex_metadata.uses {
        assert_eq!(
            additional_meta_map.get("uses").unwrap(),
            &serde_json::to_string(uses).unwrap()
        );
    }
    if let Some(collection_details) = &metaplex_metadata.collection_details {
        assert_eq!(
            additional_meta_map.get("collection_details").unwrap(),
            &serde_json::to_string(collection_details).unwrap()
        );
    }
    if let Some(creators) = &metaplex_metadata.creators {
        if !creators.is_empty() {
            assert_eq!(
                additional_meta_map.get("creators").unwrap(),
                &serde_json::to_string(creators).unwrap()
            );
        }
    }
    if let Some(config) = &metaplex_metadata.programmable_config {
        assert_eq!(
            additional_meta_map.get("config").unwrap(),
            &serde_json::to_string(config).unwrap()
        );
    }
}

#[test]
fn test_success_initialize_from_spl_token() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let metaplex_metadata_obj = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Spiderman Token".to_string(),
        symbol: "SPDR".to_string(),
        uri: "https://metaplex.dev/meta.json".to_string(),
        seller_fee_basis_points: 100,
        creators: Some(vec![Creator {
            address: Pubkey::new_unique(),
            verified: true,
            share: 100,
        }]),
        primary_sale_happened: true,
        is_mutable: false,
        edition_nonce: Some(1),
        token_standard: Some(TokenStandard::NonFungible),
        collection: Some(Collection {
            verified: false,
            key: Pubkey::new_unique(),
        }),
        uses: Some(Uses {
            use_method: UseMethod::Burn,
            remaining: 1,
            total: 1,
        }),
        collection_details: Some(CollectionDetails::V1 { size: 1 }),
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

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);

    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MetadataPointerExt)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .metaplex_metadata(metaplex_metadata)
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    // Assert base fields
    assert_eq!(wrapped_metadata.name, "Spiderman Token");
    assert_eq!(wrapped_metadata.symbol, "SPDR");
    assert_eq!(wrapped_metadata.uri, "https://metaplex.dev/meta.json");
    assert_eq!(
        Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
        result.wrapped_mint_authority.key
    );
    assert_eq!(wrapped_metadata.mint, wrapped_mint.key);

    // Assert additional metadata fields
    assert_metaplex_fields_synced(&wrapped_metadata, &metaplex_metadata_obj);
}

#[test]
fn test_success_update_from_spl_token() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let old_wrapped_metadata = MintExtension::TokenMetadata {
        name: "Old Wrapped Name".to_string(),
        symbol: "OLD".to_string(),
        uri: "https://old.uri/".to_string(),
        additional_metadata: vec![("seller_fee_basis_points".to_string(), "50".to_string())],
    };

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MintExtension::MetadataPointer)
        .with_extension(old_wrapped_metadata)
        .lamports(1_000_000_000)
        .build();

    let new_metaplex_metadata_obj = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "DocOct Token".to_string(),
        symbol: "OCT".to_string(),
        uri: "https://new.uri/".to_string(),
        seller_fee_basis_points: 200,
        creators: Some(vec![Creator {
            address: Pubkey::new_unique(),
            verified: false,
            share: 100,
        }]),
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: Some(2),
        token_standard: Some(TokenStandard::Fungible),
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    };

    let metaplex_metadata = KeyedAccount {
        key: MetaplexMetadata::find_pda(&unwrapped_mint.key).0,
        account: Account {
            lamports: 1_000_000_000,
            data: new_metaplex_metadata_obj.try_to_vec().unwrap(),
            owner: mpl_token_metadata::ID,
            ..Default::default()
        },
    };

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .metaplex_metadata(metaplex_metadata)
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    // Assert base fields
    assert_eq!(wrapped_metadata.name, "DocOct Token");
    assert_eq!(wrapped_metadata.symbol, "OCT");
    assert_eq!(wrapped_metadata.uri, "https://new.uri/");
    assert_eq!(
        Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
        result.wrapped_mint_authority.key
    );
    assert_eq!(wrapped_metadata.mint, wrapped_mint.key);

    // Assert additional metadata fields
    assert_metaplex_fields_synced(&wrapped_metadata, &new_metaplex_metadata_obj);
}
