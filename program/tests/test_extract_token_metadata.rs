use {
    crate::helpers::{
        common::{KeyedAccount, TokenProgram},
        extensions::MintExtension,
        mint_builder::MintBuilder,
        sync_to_spl_token_builder::SyncToSplTokenBuilder,
        sync_to_token_2022_builder::SyncToToken2022Builder,
        token_account_builder::TokenAccountBuilder,
    },
    mollusk_svm::result::Check,
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
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
fn test_fail_pointer_missing() {
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
    assert_eq!(
        result.unwrap_err(),
        TokenWrapError::MetadataPointerMissing.into()
    );
}

#[test]
fn test_fail_pointer_unset() {
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
fn test_fail_pointer_to_external_account_not_provided() {
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
