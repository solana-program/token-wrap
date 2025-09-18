use {
    crate::helpers::{
        common::{KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
        sync_to_spl_token_builder::SyncToSplTokenBuilder,
        sync_to_token_2022_builder::SyncToToken2022Builder,
        token_account_builder::TokenAccountBuilder,
    },
    borsh::BorshSerialize,
    mollusk_svm::result::Check,
    mpl_token_metadata::{accounts::Metadata as MetaplexMetadata, types::Key},
    solana_account::Account,
    solana_account_info::AccountInfo,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_wrap::{error::TokenWrapError, metadata::extract_token_metadata},
};

pub mod helpers;

#[test]
fn test_fail_invalid_owner() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let wrong_owner = Pubkey::new_unique(); // Not a token program

    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &wrong_owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert_eq!(result.unwrap_err(), ProgramError::IncorrectProgramId);
}

// --- SPL Token Source Tests ---

#[test]
fn test_fail_spl_token_missing_metaplex_account() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert_eq!(result.unwrap_err(), ProgramError::NotEnoughAccountKeys);
}

#[test]
fn test_fail_spl_token_with_wrong_metaplex_owner() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let wrong_owner = Pubkey::new_unique();
    let source_metadata = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            owner: wrong_owner,
            ..Default::default()
        },
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountOwner);
}

#[test]
fn test_fail_spl_token_with_invalid_metaplex_pda() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();
    let invalid_pda_account = KeyedAccount {
        key: Pubkey::new_unique(), // Not the derived PDA
        account: Account {
            owner: mpl_token_metadata::ID,
            ..Default::default()
        },
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = invalid_pda_account.account.lamports;
    let mut source_data = invalid_pda_account.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &invalid_pda_account.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &invalid_pda_account.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(
        result.unwrap_err(),
        TokenWrapError::MetaplexMetadataMismatch.into()
    );
}

// --- Token-2022 Source Tests ---

#[test]
fn test_fail_no_fallback_account_provided() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No pointer extension

    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert_eq!(result.unwrap_err(), ProgramError::NotEnoughAccountKeys);
}

#[test]
fn test_success_no_pointer_fallback_to_metaplex() {
    // No pointer, but a valid Metaplex PDA is provided, should succeed.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No pointer

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let mut source_metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        lamports: 1_000_000_000,
        ..Default::default()
    };
    let metaplex_metadata_obj = MetaplexMetadata {
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
        name: "Metaplex Fallback".to_string(),
        symbol: "FALL".to_string(),
        uri: "uri.fallback".to_string(),
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
    source_metaplex_account.data = metaplex_metadata_obj.try_to_vec().unwrap();
    let source_metadata = KeyedAccount {
        key: metaplex_pda,
        account: source_metaplex_account,
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert!(result.is_ok());
    let token_metadata = result.unwrap();
    assert_eq!(token_metadata.name, "Metaplex Fallback");
}

#[test]
fn test_fail_no_pointer_fallback_wrong_owner() {
    // No pointer, fallback provided, but its owner is not Metaplex.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No pointer

    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let source_metadata = KeyedAccount {
        key: metaplex_pda,
        account: Account {
            owner: Pubkey::new_unique(), // Not Metaplex
            ..Default::default()
        },
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountOwner);
}

#[test]
fn test_fail_pointer_unset() {
    // Pointer extension exists, but address is None.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: None, // Unset pointer
        })
        .build();
    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert_eq!(
        result.unwrap_err(),
        TokenWrapError::MetadataPointerUnset.into()
    );
}

#[test]
fn test_success_self_referential_pointer() {
    // Token-2022 mint with a self-referential pointer and metadata should succeed.
    let unwrapped_mint_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_key)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(unwrapped_mint_key),
        })
        .with_extension(MintExtension::TokenMetadata {
            name: "Self Ref".to_string(),
            symbol: "SELF".to_string(),
            uri: "uri.self".to_string(),
            additional_metadata: vec![],
        })
        .build();

    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert!(result.is_ok());
    let token_metadata = result.unwrap();
    assert_eq!(token_metadata.name, "Self Ref");
    assert_eq!(token_metadata.symbol, "SELF");
}

#[test]
fn test_fail_self_referential_pointer_no_metadata() {
    // Self-referential pointer but no TokenMetadata extension should fail.
    let unwrapped_mint_key = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint_key)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(unwrapped_mint_key),
        })
        // No TokenMetadata extension
        .build();

    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    // The call to `get_variable_len_extension` should fail because the
    // TokenMetadata extension is not present on the mint.
    assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}

#[test]
fn test_fail_pointer_to_external_account_not_provided() {
    // Pointer points to an external account, but it's not provided.
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(Pubkey::new_unique()), // Points to external
        })
        .build();
    let mut lamports = unwrapped_mint.account.lamports;
    let mut data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, None, None);
    assert_eq!(result.unwrap_err(), ProgramError::NotEnoughAccountKeys);
}

#[test]
fn test_fail_pointer_mismatch() {
    // Pointer points to one account, but a different one is provided.
    let pointer_address = Pubkey::new_unique();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(pointer_address),
        })
        .build();

    let wrong_metadata_account = KeyedAccount {
        key: Pubkey::new_unique(), // Does not match pointer_address
        account: Account::default(),
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = wrong_metadata_account.account.lamports;
    let mut source_data = wrong_metadata_account.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &wrong_metadata_account.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &wrong_metadata_account.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(
        result.unwrap_err(),
        TokenWrapError::MetadataPointerMismatch.into()
    );
}

#[test]
fn test_fail_pointer_to_token_2022_account_is_unsupported() {
    let mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build();
    let source_metadata_account = TokenAccountBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint(mint)
        .build();

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(source_metadata_account.key),
        })
        .build();

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata_account.account.lamports;
    let mut source_data = source_metadata_account.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata_account.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata_account.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
}

#[test]
fn test_success_pointer_to_metaplex() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build();

    // Re-derive PDA with actual mint key
    let (metaplex_pda, _) = MetaplexMetadata::find_pda(&unwrapped_mint.key);
    let mut metaplex_account = Account {
        owner: mpl_token_metadata::ID,
        lamports: 1_000_000_000,
        ..Default::default()
    };
    let metaplex_metadata_obj = MetaplexMetadata {
        name: "Pointed-to Metaplex".to_string(),
        symbol: "PMP".to_string(),
        uri: "uri.pmp".to_string(),
        key: Key::MetadataV1,
        update_authority: Pubkey::new_unique(),
        mint: unwrapped_mint.key,
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
    metaplex_account.data = metaplex_metadata_obj.try_to_vec().unwrap();
    let source_metadata = KeyedAccount {
        key: metaplex_pda,
        account: metaplex_account,
    };

    // Add pointer to the mint
    let unwrapped_mint_with_pointer = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(unwrapped_mint.key)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(metaplex_pda),
        })
        .build();

    let mut mint_lamports = unwrapped_mint_with_pointer.account.lamports;
    let mut mint_data = unwrapped_mint_with_pointer.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint_with_pointer.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint_with_pointer.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert!(result.is_ok());
    let token_metadata = result.unwrap();
    assert_eq!(token_metadata.name, "Pointed-to Metaplex");
}

#[test]
fn test_fail_pointer_to_third_party_missing_owner_program() {
    // Pointer to third-party, but `owner_program_info` is missing.
    let source_metadata = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            owner: mock_metadata_owner::ID,
            ..Default::default()
        },
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(source_metadata.key),
        })
        .build();

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(&unwrapped_mint_info, Some(&source_metadata_info), None);
    assert_eq!(result.unwrap_err(), ProgramError::NotEnoughAccountKeys);
}

#[test]
fn test_fail_pointer_to_third_party_owner_program_mismatch() {
    // Pointer to third-party, but `owner_program_info` key mismatches.
    let source_metadata = KeyedAccount {
        key: Pubkey::new_unique(),
        account: Account {
            owner: mock_metadata_owner::ID, // Real owner
            ..Default::default()
        },
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(source_metadata.key),
        })
        .build();
    let wrong_owner_program = KeyedAccount {
        key: Pubkey::new_unique(), // Wrong program ID
        account: Account::default(),
    };

    let mut mint_lamports = unwrapped_mint.account.lamports;
    let mut mint_data = unwrapped_mint.account.data.clone();
    let unwrapped_mint_info = AccountInfo::new(
        &unwrapped_mint.key,
        false,
        false,
        &mut mint_lamports,
        &mut mint_data,
        &unwrapped_mint.account.owner,
        false,
        0,
    );
    let mut source_lamports = source_metadata.account.lamports;
    let mut source_data = source_metadata.account.data.clone();
    let source_metadata_info = AccountInfo::new(
        &source_metadata.key,
        false,
        false,
        &mut source_lamports,
        &mut source_data,
        &source_metadata.account.owner,
        false,
        0,
    );
    let mut owner_lamports = wrong_owner_program.account.lamports;
    let mut owner_data = wrong_owner_program.account.data.clone();
    let wrong_owner_program_info = AccountInfo::new(
        &wrong_owner_program.key,
        false,
        false,
        &mut owner_lamports,
        &mut owner_data,
        &wrong_owner_program.account.owner,
        false,
        0,
    );

    let result = extract_token_metadata(
        &unwrapped_mint_info,
        Some(&source_metadata_info),
        Some(&wrong_owner_program_info),
    );
    assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountOwner);
}

// ====== Integration tests for CPI logic ======
// These tests use Mollusk to test the `cpi_emit_and_decode` logic,
// which cannot be unit tested.

// Syncing to Token-2022

#[test]
fn test_integration_pointer_to_third_party_no_return_fails() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
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
    SyncToToken2022Builder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata)
        .check(Check::err(
            TokenWrapError::ExternalProgramReturnedNoData.into(),
        ))
        .execute();
}

#[test]
fn test_integration_pointer_to_third_party_success() {
    let source_metadata_extension = MintExtension::TokenMetadata {
        name: "Mock External".to_string(),
        symbol: "MEXT".to_string(),
        uri: "uri.mock".to_string(),
        additional_metadata: vec![],
    };
    let mock_metadata_key = Pubkey::new_unique();
    let mut source_metadata = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(mock_metadata_key)
        .with_extension(source_metadata_extension)
        .build();
    source_metadata.account.owner = mock_metadata_owner::ID;

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(mock_metadata_key),
        })
        .build();

    SyncToToken2022Builder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata)
        .execute();
}

// Syncing to Spl Token

#[test]
fn test_integration_spl_token_pointer_to_third_party_no_return_fails() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
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
    SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata)
        .check(Check::err(
            TokenWrapError::ExternalProgramReturnedNoData.into(),
        ))
        .execute();
}

#[test]
fn test_integration_spl_token_pointer_to_third_party_success() {
    let source_metadata_extension = MintExtension::TokenMetadata {
        name: "Mock External".to_string(),
        symbol: "MEXT".to_string(),
        uri: "uri.mock".to_string(),
        additional_metadata: vec![],
    };
    let mock_metadata_key = Pubkey::new_unique();
    let mut source_metadata = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(mock_metadata_key)
        .with_extension(source_metadata_extension)
        .build();
    source_metadata.account.owner = mock_metadata_owner::ID;

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::MetadataPointer {
            metadata_address: Some(mock_metadata_key),
        })
        .build();

    SyncToSplTokenBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .source_metadata(source_metadata)
        .execute();
}
